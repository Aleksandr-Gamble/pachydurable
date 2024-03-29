//! This module defines ergonomic ways to create and use a Redis connection pool.
//! The mobc crate is used to create an async pool. This was chosen for two reasons,
//! 1) familiar paralellism with the mobc-postgres crate used extensively elewhere
//! 2) [This page](https://blog.logrocket.com/using-redis-in-a-rust-web-service/) reports successful deployment in production using mobc:
//! 
//! The new_client_from_env() and new_pool_from_env() methods maket it easy to connect using these environment variables:
//! REDIS_HOST: The IP where the Redis server is running. Defauls to "127.0.0.1"
//! REDIS_PORT: The port on which the server is listening. Defaults to 6379
//! REDIS_PW: The authentication password for Redis
//! IS_TSL: If set to anything, rediss will be used instead of redis

use std::env;
use serde::{Serialize, de::DeserializeOwned};
use async_trait::async_trait;
use mobc::Pool;
use mobc_redis::{RedisConnectionManager, redis::{AsyncCommands, RedisResult, Client, aio::Connection}};
use tokio_postgres::{row::Row, types::ToSql};
use crate::err::{PachyDarn, MissingRowError};
use crate::connect::ClientNoTLS;
use crate::autocomplete::{AutoComp, WhoWhatWhere};

// constants for mobc redis connection pools
// see https://blog.logrocket.com/using-redis-in-a-rust-web-service/
const CACHE_POOL_MAX_OPEN: u64 = 16;
const _CACHE_POOL_MAX_IDLE: u64 = 8;
const _CACHE_POOL_TIMEOUT_SECONDS: u64 = 20;
const _CACHE_POOL_EXPIRE_SECONDS: u64 = 60;
const OBSCURE_TEST_KEY: &'static str = "_OBSCURE_TEST_KEY_0";

pub type RedisConn = Connection<RedisConnectionManager>;
pub type RedisPool = Pool<RedisConnectionManager>;


/// The cacheable trait lets you lookup an instance of a struct from some parameters using the cached_or_cache function.
/// It will first check to see if a value has been cached in Redis
/// If not, it will next check in postgres.
/// If a value is found, it will be cahced and returned 
/// If nothing is found in Postgres either, the None variant will be returned
#[async_trait]
pub trait Cacheable: Serialize + DeserializeOwned {

    /// Redis keys caching instances of this type will be prefixed with this prefix
    fn key_prefix() -> &'static str;

    /// When a value is cached to redis, set the expiry in seconds until it is removed auomatically.
    fn seconds_expiry() -> usize;

    /// This method generates a key showing where to cache an instance of a struct in Redis
    fn redis_key(params:&[&(dyn ToSql + Sync)]) -> String {
        let mut key = format!("cacheable_{}", Self::key_prefix());
        for param in params {
            let delta = format!("_{:?}", param).replace("\"","");
            key.push_str(&delta);
        }
        key
    }

    /// Define the query that should be used with the assocaited parameters (i.e. those used in redis_key()) 
    /// to return an instance of the struct 
    fn query() -> &'static str;

    /// Define how to convert a postgres row to as instance of the struct 
    fn from_row(row: &Row) -> Self;

}

/// The cacheable trait lets you lookup an instance of a struct from some parameters using the cached_or_cache function.
/// It will first check to see if a value has been cached in Redis
/// If not, it will next check in postgres.
/// If a value is found, it will be cahced and returned 
/// If nothing is found in Postgres either, the None variant will be returned
pub async fn cached_or_cache<T: Cacheable>(c: &ClientNoTLS, pool: &RedisPool, params: &[&(dyn ToSql + Sync)]) -> Result<Option<T>, PachyDarn> {
    let key = T::redis_key(params);
    let cached: Option<T> = rediserde::get(pool, &key).await?;
    match cached {
        Some(val) => Ok(Some(val)),
        None => {
            let query = T::query();
            let rows = c.query(query, params).await?;
            match rows.get(0) {
                None => Ok(None),
                Some(row) => {
                    let val = T::from_row(row);
                    let _x = rediserde::set_ex(pool, &key, &val, T::seconds_expiry()).await?;
                    Ok(Some(val))
                }
            }
        }
    }
}


/// the cached_or_cache function returns Result<Option<T>, PachyDarn>
/// The "_f" in cached_or_cache_f indicates that it forces the code to look for the Some variant,
/// returning the MissingRow variant of a PachyDarn error if it was not found 
pub async fn cached_or_cache_f<T: Cacheable>(c: &ClientNoTLS, pool: &RedisPool, params: &[&(dyn ToSql + Sync)]) -> Result<T, PachyDarn> {
    let opt: Option<T> = cached_or_cache(c, pool, params).await?;
    match opt {
        Some(val) => Ok(val),
        None => Err(PachyDarn::from(MissingRowError::from_str("cached_or_cache_f found a None variant"))),
    }
}


/// The PreWarmDepth indicates how many characters (1,2, or 3) should be used for pre-caching autocomplete results
pub enum PreWarmDepth {
    /// pre-warm the cache with 1-character deep results: i.e. 36 values
    Char1,
    /// pre-warm the cache with 1+2-character deep results: i.e. 36*(1+42) = 1,548 values
    Char2,
    /// pre-warm the cache with 1+2+3-character deep results: i.e. 36*(1+42)*(1+42) = 66,564 values
    Char3,
}


/// The autocomplete introduces the AutoComp trait, which allows a vector of <WhoWhatWhere<PK>>
/// to be returned by querying Postgres for a given phrase.   
/// This CachedAutoComp trait is related (in fact, it requires for AutoComp to also be implemented):
/// By defining a dtype() classmethod (which is needed so different WhoWhatWhere types don't share the same
/// key in redis) and a seconds to expiry, the cached_autocomp function will first look in redis 
/// for a cached value. If one cannot be found, the (non-cached) AutoComp trait is used to find 
/// resulting hits, which are then cached and returned. 
pub trait CachedAutoComp<PKC: Serialize+DeserializeOwned+std::marker::Send>: AutoComp<PKC> {
    /// The data type is used in prefixing the redis key.
    fn dtype() -> &'static str;
    /// The cahced value in redis will expire after this many seconds.
    fn seconds_expiry() -> usize;
    /// This sets the depth (number of characters) to which a value will be cached in Redis. 
    fn prewarm_depth() -> PreWarmDepth;
}




// generate the Redis key to use for cached autocomplete results for a given <T> and phrase
fn autocomp_key<PKC: Serialize+DeserializeOwned+std::marker::Send, T: CachedAutoComp<PKC>>(phrase: &str) -> String {
    let lphrase = phrase.to_lowercase(); // Postgres tsquery is case insensitive by Redis keys are not
    let key = format!("autocomp_{}_{}", T::dtype(), &lphrase );
    key
}



/// as the name implies, recache will redo the postgres query for autocomplete results for a given phrase and cache the value,
/// overwiting any previous result. 
pub async fn recache<PKC: Serialize+DeserializeOwned+std::marker::Send, T: CachedAutoComp<PKC>>(pool: &RedisPool, c: &ClientNoTLS, phrase: &str) -> Result<Vec<WhoWhatWhere<PKC>>, PachyDarn> {
    let key = autocomp_key::<PKC, T>(&phrase);
    let hits: Vec<WhoWhatWhere<PKC>> = <T as AutoComp<PKC>>::exec_autocomp(c, &phrase).await?;
    let _x = rediserde::set_ex(pool, &key, &hits, T::seconds_expiry()).await?;
    Ok(hits)
}


/// the cached_autocomp function will first look in Redis for cached autocomplete results before looking in Postgres.  
/// See more detail under the CachedAutoComp trait. 
pub async fn cached_autocomp<PKC: Serialize+DeserializeOwned+std::marker::Send, T: CachedAutoComp<PKC>>(pool: &RedisPool, c: &ClientNoTLS, phrase: &str) -> Result<Vec<WhoWhatWhere<PKC>>, PachyDarn> {
    let key = autocomp_key::<PKC, T>(phrase);
    let cached: Option<Vec<WhoWhatWhere<PKC>>> = rediserde::get(pool, &key).await?;
    match cached {
        Some(hits) => Ok(hits),
        None => { recache::<PKC, T>(pool, c, phrase).await }
    }
}


/// The AutoComp trait queries postgres for matching WhoWhatWhere<PKC> structs.  This is typically slowest for the first few
/// characters (i.e. very short strings) because they will generate the most matches. It is helpful to therefore
/// defind a method that will iterate over many short strings and pre-query the database and cache the results to Redis. 
pub async fn warm_the_cache<PKC: Serialize+DeserializeOwned+std::marker::Send, T: CachedAutoComp<PKC>>(pool: &RedisPool, c: &ClientNoTLS) -> Result<(), PachyDarn> {
    let chars1 =  "abcdefghijklmnopqrstuvwxyz0123456789";
    let chars23 = "abcdefghijklmnopqrstuvwxyz_.!?-0123456789 "; // note the space at the end
    for c1 in chars1.chars() {
        let mut phrase = c1.to_string();
        let _hits = recache::<PKC, T>(pool, c, &phrase).await?;
        match T::prewarm_depth() {
            PreWarmDepth::Char1 => continue,
            _ => {}
        }
        for c2 in chars23.chars() {
            phrase.push(c2);
            let _hits = recache::<PKC, T>(pool, c, &phrase).await?;
            match T::prewarm_depth() {
                PreWarmDepth::Char3 => {},
                _ => continue
            }
            for c3 in chars23.chars() {
                phrase.push(c3);
                let _hits = recache::<PKC, T>(pool, c, &phrase).await?;
            }
        }
    }
    Ok(())
}


/// Return a new connection pool from the mobc_redis::Client struct
pub async fn new_pool_from_client(client: Client) -> Result<RedisPool, PachyDarn> {
    let manager = RedisConnectionManager::new(client);
    let pool = Pool::builder()
        //.get_timeout(Some(Duration::from_secs(CACHE_POOL_TIMEOUT_SECONDS)))
        .max_open(CACHE_POOL_MAX_OPEN)
        //.max_idle(CACHE_POOL_MAX_IDLE)
        //.max_lifetime(Some(Duration::from_secs(CACHE_POOL_EXPIRE_SECONDS)))
        //.max_lifetime(None)
        .build(manager);
    // try to connect now so you fail early
    let mut conn = pool.get().await?;
    let _x: Option<String> = conn.get(OBSCURE_TEST_KEY).await?;
    Ok(pool)
}

/// Create a new pool from a client generated with these environment variables:
pub async fn new_pool_from_env() -> Result<RedisPool, PachyDarn> {
    let client = new_client_from_env()?;
    new_pool_from_client(client).await
}


/// Generate a new client based on a uri scheme, a host, and a password
pub fn new_client(uri_scheme: &str, redis_host: &str, redis_pw: &str) -> RedisResult<Client> {
    let redis_conn_url = format!("{}://:{}@{}", uri_scheme, redis_pw, redis_host);
    Client::open(redis_conn_url)
}

/// Generate a new client from environment variables
pub fn new_client_from_env() -> RedisResult<Client>  {
    let uri_scheme = match env::var("IS_TLS") {
        Ok(_) => "rediss",
        Err(_) => "redis",
    };

    let redis_host: String = match env::var("REDIS_HOST") {
        Ok(val) => val,
        Err(_) => {
            match env::var("REDIS_PORT")  {
                Ok(port) => format!("127.0.0.1:{}", port),
                Err(_) => "127.0.0.1:6379".to_string(),
            }
        },
    };
    let redis_pw: String = match env::var("REDIS_PW") {
        Ok(val) => val,
        Err(_) => "".to_string(),
    };
    new_client(&uri_scheme, &redis_host, &redis_pw)
}



pub mod rediserde {
    use super::{RedisPool};
    use mobc_redis::redis::AsyncCommands;
    use crate::err::PachyDarn;
    use serde::{Serialize, de::DeserializeOwned};
    use serde_json;


    /// Delete a key 
    pub async fn del(pool: &RedisPool, key: &str) -> Result<(), PachyDarn> {
        let mut rconn = pool.get().await?;
        let _ : () = rconn.del(key).await?;
        Ok(())
    }

    /// For a struct that can be deserialized,
    /// This helpful method gets a connection, gets the value stored at the key,
    /// deserializes it, and returns the desired struct
    pub async fn get<T: DeserializeOwned>(pool: &RedisPool, key: &str) -> Result<Option<T>, PachyDarn> {
        let mut rconn = pool.get().await?;
        let jz: String = match rconn.get(key).await {
            Ok(val) => val,
            Err(e) => {
                if e.to_string().contains("response was nil") {
                    return Ok(None)
                }
                return Err(e.into())
            }  
        };
        let t: T = serde_json::from_str(&jz)?;
        Ok(Some(t))
    }

    /// For a struct that can be serialized,
    /// This helpful method gets a connection, gets teh value stored at the key,
    /// deserializes it, and returns the desired struct 
    pub async fn set<T: Serialize>(pool: &RedisPool, key: &str, value: &T) -> Result<(), PachyDarn> {
        let mut rconn = pool.get().await?;
        let jz: String = serde_json::to_string(value)?;
        let _ : () = rconn.set(key, jz).await?;
        Ok(())
    }

    /// This is like set but with an expiry 
    pub async fn set_ex<T: Serialize>(pool: &RedisPool, key: &str, value: &T, seconds_expiry: usize) -> Result<(), PachyDarn> {
        let mut rconn = pool.get().await?;
        let jz: String = serde_json::to_string(value)?;
        let _ : () = rconn.set_ex(key, jz, seconds_expiry).await?;
        Ok(())
    }

    /// add a struct to a set
    pub async fn sadd<T: Serialize>(pool: &RedisPool, key: &str, value: &T) -> Result<(), PachyDarn> {
        let mut rconn = pool.get().await?;
        let jz: String = serde_json::to_string(value)?;
        let _ : () = rconn.sadd(key, jz).await?;
        Ok(())
    }

    /// add a string to a set
    pub async fn sadd_str(pool: &RedisPool, key: &str, val: &str) -> Result<(), PachyDarn> {
        let mut rconn = pool.get().await?;
        let _ : () = rconn.sadd(key, val).await?;
        Ok(())
    }

    /// report if a string is a member of a set 
    pub async fn sismember_str(pool: &RedisPool, key: &str, val: &str) -> Result<bool, PachyDarn> {
        let mut rconn = pool.get().await?;
        let ismember: bool = rconn.sismember(key, val).await?;
        Ok(ismember)
    }

    pub async fn spop_str(pool: &RedisPool, key: &str) -> Result<Option<String>, PachyDarn> {
        // This pool.get() hangs sometimes with the error "Timed out in mobc". What to do?  
        let mut rconn = pool.get().await?;
        let jz: String = match rconn.spop(key).await {
            Ok(val) => val,
            Err(e) => {
                if e.to_string().contains("response was nil") {
                    return Ok(None)
                }
                return Err(e.into())
            }  
        };
        Ok(Some(jz))
    }


    pub async fn spop<T: DeserializeOwned>(pool: &RedisPool, key: &str) -> Result<Option<T>, PachyDarn> {
        let jz = match spop_str(pool, key).await? {
            Some(val) => val,
            None => return Ok(None),
        };
        let t: T = serde_json::from_str(&jz)?;
        Ok(Some(t))
    }

    pub async fn scard(pool: &RedisPool, key: &str) -> Result<usize, PachyDarn> {
        let mut rconn = pool.get().await?;
        let cardinality = rconn.scard(key).await?;
        Ok(cardinality)
    }

}





#[cfg(test)]
mod tests {
    use mobc_redis;
    use rand::{Rng, distributions::Alphanumeric}; 
    use tokio::runtime::Runtime;
    use serde::{Serialize, Deserialize};
    use super::*;

    // use different keys for different tests-
    // remember they all get executed at once asynchronously 
    const OBSCURE_TEST_KEY_1: &'static str = "_OBSCURE_TEST_KEY_1";
    const OBSCURE_TEST_KEY_2: &'static str = "_OBSCURE_TEST_KEY_2";

    fn gen_rand_int() -> i32 {
        rand::thread_rng().gen_range(1..1000)
    }

    #[derive(Serialize, Deserialize)]
    struct DemoStruct {
        id: i32,
        name: String,
    }

    impl mobc_redis::redis::ToRedisArgs for DemoStruct {
        fn write_redis_args<W>(&self, out: &mut W)
            where
                W: ?Sized + mobc_redis::redis::RedisWrite {
            out.write_arg_fmt(serde_json::to_string(self).expect("Can't serialize DemoStruct"))
        }
    }

    #[test]
    fn get_set_int() {
        // ensure you can set and get a value 
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let rpool = new_pool_from_env().await.unwrap();
            let mut rconn = rpool.get().await.unwrap();
            let rand_int = gen_rand_int();
            let _ : () = rconn.set(OBSCURE_TEST_KEY_1, rand_int).await.unwrap();
            let ox: Option<i32> = rconn.get(OBSCURE_TEST_KEY_1).await.unwrap();
            assert_eq!(ox.unwrap(), rand_int);
            println!("redis::get_set_int passed: {} == {}", ox.unwrap(), rand_int);

        })
    }

    #[test]
    fn get_set_struct() {
        // ensure you save and load an instance of a struct 
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let rpool = new_pool_from_env().await.unwrap();
            // ensure you get delete a key and then get the None variant back 
            let _x = rediserde::del(&rpool, OBSCURE_TEST_KEY_2).await.unwrap();
            let ods2: Option<DemoStruct> = rediserde::get(&rpool, OBSCURE_TEST_KEY_2).await.unwrap();
            assert!(ods2.is_none());
            // Then set it and ensure you can get the Some() variant back
            let id = gen_rand_int();
            let name: String = rand::thread_rng().sample_iter(&Alphanumeric).take(7).map(char::from).collect();
            let ds = DemoStruct{id, name};
            let _x = rediserde::set(&rpool, OBSCURE_TEST_KEY_2, &ds).await.unwrap();
            let ods2: Option<DemoStruct> = rediserde::get(&rpool, OBSCURE_TEST_KEY_2).await.unwrap();
            let ds2 = ods2.unwrap();
            assert_eq!(&ds.id, &ds2.id);
            assert_eq!(&ds.name, &ds2.name);
        })
    }
}

