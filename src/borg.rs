//! This module introduces the Borg trait and associated borg() function.
//! As a nmeumonic, Borg was chosen as the name of the trait because it creates one type by                              
//! 'assimilating' or owning two other types. It uses for generic types:                                                 
//! B: an input taken By reference                                                
//! O: an input taken by Ownership                                                
//! R: an intermedite value that will be cached to / deserialized from Redis
//! G: A generated term that consumes R and is consumed itself    
//!
//! And additional fifth generic E must also be provided as the error type to return when something
//! goes wrong.
//!
//! This approach has a few benefits
//! 1) Building up types that (can) consume bulky types can be done easily, removing unnecesasry
//!    .clone() operations.
//! 2) When a new instance of a type is needed, and you want to ensure a record exists for that
//!    instance in a database, the required .redis_pk_member() String will be used to first check
//!    Redis if a matching instance has already been instantiated, avoiding unnecessarly, slow disk
//!    operations.
//! 3) The .on_invocation(), .on_pk_sadd(), and .on_instantiation() optional methods make it
//!    ergonomic to emit events (presumably via http call) at various point in instantiation.

use std::convert::From;
use async_recursion::async_recursion;
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use tokio_postgres::types::FromSqlOwned;
use crate::{connect::ClientNoTLS, err::{PachyDarn, MissingRowError}, redis::{rediserde, RedisPool}};


/// The Borg trait is intended as a fast, ergonomic way to build up complex types
/// while (1) minimizing disk io via caching, and 
/// (2) Minimizing memory copy/clone operations via consuming one struct and an intermediate (cached struct)
/// 
/// As a nmeumonic, Borg was chosen as the name of the trait because it creates one type by
/// 'assimilating' or owning two other types. It uses for generic types:
/// B: an input taken By reference
/// O: an input taken by Ownership 
/// R: an intermedite value that will be cached to / deserialized from Redis
/// G: A generated term that consumes R and is consumed itself 
/// 
/// On additional consideration used in making this trait is that a trait method that returns
/// Result<Self, error> is not allowed since Sized is not implemented for an unknown Self, but
/// Result<type, error> is okay since the type is known. 
/// 

/// Once this trait is implemented, you can can call t: T = borg(c, rpool, &B, O) to instantiate
/// 
/// Note that the redis_key_r may not be unique to a struct
/// For instance, if you cache the subdoman_prefix id for a SubDomain,
/// That prefix will be used by many Subdomains
/// In contrast, the REDIS_PK_MEMBER of the REDIS_PK_SET should be unique
/// if it is not found, ON_SADD_PK will be called.
/// This allows a mechanism to write new things to disk if they have not been cached previously 
/// define a key that will be used for the SET containing PK values for instantiations

#[async_trait]
pub trait Borg<B, O, R: Serialize + DeserializeOwned, G, E: std::error::Error + From<PachyDarn>>: std::marker::Sync {

    /// the redis prefix will be used in two contexts:
    /// borg_r_PREFIX_SUFFIX is the key that will be used to cache the R value
    /// borg_pks_PREFIX will be used for the set of PKs to determine when on_pk_sadd needs to be called
    fn redis_prefix() -> &'static str;

    /// define a key that will be used to cache a value for R in Redis
    fn redis_suffix_r(b: &B, o: &O) -> String;

    /// How long should a cached value for R in redis persist 
    fn redis_expiry_r() -> usize {
        60*60*2 as usize // 2 hours 
    }

    /// Define a string unique to a given to a fully-specified innstance
    fn redis_pk_member(&self) -> String;

    /// to avoid accumulation of excessively large sets, clear the set if it gets larger than this 
    fn redis_pk_max_ct() -> usize {
        1_000_000 as usize
    }

    /// This method generates the value R to be cached to redis if not previously set 
    /// Notice the 'a lifetime signature- you have to adhere to this as you will see
    /// if you [read the docs](https://docs.rs/async-trait/latest/async_trait/#elided-lifetimes)
    async fn redis_value<'a>(c: &'a ClientNoTLS, rpool: &'a RedisPool, b: &'a B, o: &'a O) -> Result<R, E>;

    /// This method takes the value R used/taken as a Redis value and the owned type O
    /// and returns a generated 'G' type 
    async fn generate<'a>(c: &'a ClientNoTLS, rpool: &'a RedisPool, b: &'a B, o: O, r: R) -> Result<G, E>;

    /// Define a method that returns self based on &B and the generated struct G
    fn instantiate(b: &B, g: G) -> Self;

    /// borg(...) calls this method first thing on invocation:
    /// there may be reason to emit an event etc. before there is any other chance for error 
    async fn on_invocation(_b: &B, _o: &O) -> Result<(), E> {
        Ok(())
    }

    /// borg(...) will call on_pk_sadd AFTER instantiate(...) but BEFORE on_instantiation(...)
    /// IF the string returned by redis_pk_member was not present 
    /// This is typically done to ensure a record exists in Postgres reflecting the new item
    async fn on_pk_sadd<'a>(&'a self, _c: &'a ClientNoTLS, _rpool: &'a RedisPool, _b: &'a B) -> Result<(), E> {
        Ok(())
    }
    
    /// borg(...) calls this method last thing, just after constructing self 
    /// and just before returning it. method is called last thing- just as instantiation finishes.
    async fn on_instantiation(&self) -> Result<(), E> {
        Ok(())
    }
}


/// Instantiate a type that implements the Borg trait by taking ownership of TC and referencing
/// TR. 
/// The Borg::on_instantiation() method will be called automatically 
pub async fn borg<B, O, R: Serialize + DeserializeOwned, G, E: std::error::Error + From<PachyDarn>, T: Borg<B, O, R, G, E>>(c: &ClientNoTLS, rpool: &RedisPool, b: &B, o: O) -> Result<T, E> {
    // call on_invocation first- before any (other) error can be thrown 
    let _x = <T as Borg<B, O, R, G, E>>::on_invocation(b, &o).await?;
    // determine which Redis key should be used to SET/GET values for R
    let prefix = <T as Borg<B, O, R, G, E>>::redis_prefix();
    let suffix: String = <T as Borg<B, O, R, G, E>>::redis_suffix_r(&b, &o);
    let key_r = format!("borg_r_{}_{}", prefix, &suffix);
    let key_set_pks = format!("borg_pks_{}", prefix);
    // check to see if that key is set in Redis
    let cached: Option<R> = rediserde::get(rpool, &key_r).await?;
    let r: R = match cached {
        Some(val) => val,
        None => {
            // If the value has not been set in redis, generate it by calling redis_value(...)
            let val: R = <T as Borg<B, O, R, G, E>>::redis_value(c, rpool, &b, &o).await?;
            let _x = rediserde::set_ex(rpool, &key_r, &val, <T as Borg<B, O, R, G, E>>::redis_expiry_r()).await?;
            val
        }
    };
    // Consume the owned type O and the Redis type R to return a generated type G
    let g: G = <T as Borg<B, O, R, G, E>>::generate(c, rpool, &b, o, r).await?;
    // instantiate the thing you want to return
    let inst = T::instantiate(&b, g);
    // if the PK for inst is not a member of the associated set in redis, call on_pk_sadd
    let member = inst.redis_pk_member();
    if ! rediserde::sismember_str(rpool, &key_set_pks, &member).await? {
        let _x = inst.on_pk_sadd(c, rpool, &b).await?;
        if <T as Borg<B, O, R, G, E>>::redis_pk_max_ct() < rediserde::scard(rpool, &key_set_pks).await? {
            // too many old keys are cached! delete the set and start over 
            let _x = rediserde::del(rpool, &key_set_pks).await?;
        }
        let _x = rediserde::sadd_str(rpool, &key_set_pks, &member).await?;
    }
    // finally, call on_instantiation if you want to emit an event or whatever
    let _x = inst.on_instantiation().await?;
    Ok(inst)
}



/// The WritePG trait makes it easy to write things to Postgres
/// The the type T that is returned can be set to the product PK or whatever else you prefer
#[async_trait]
pub trait WritePG<T: Send + Sync> {
    async fn write_pg(&self, c: &ClientNoTLS) -> Result<T, PachyDarn>;
}


/// Several tables have an (integer) PK with a unique constraint on a VARCHAR value
/// This function lets you provide the QUERY and INSERT statements to allow querying/insereting into those tables
/// NOTE: This function is recursive becuae it contains logic to retry upon duplicate insert attempts
/// This is only expected to occur if many inserts are being done at once 
#[async_recursion]
pub async fn get_string_id<'a, T: FromSqlOwned>(c: &'a ClientNoTLS, name: &'a str, query: &'a str, insert: &'a str) -> Result<T, PachyDarn> {
    let rows = c.query(query, &[&name]).await?;
    match rows.get(0) {
        Some(row) => {Ok(row.get(0))},
        None => {
            // if you reach this point, a record needs to be insertred
            match c.query(insert, &[&name] ).await {
                Ok(rows) => {
                    match rows.get(0) {
                        Some(row) => {
                            let id: T= row.get(0);
                            Ok(id)
                        },
                        // IDK how you would ever reach the code below, but it sounds bad
                        None => Err(MissingRowError{message: "How on earth do you insert a row but not get it back?".to_string()}.into())
                    }
                },
                Err(e) => {
                    let errtext = e.to_string();
                    if errtext.contains("duplicate key value violates unique constraint") {
                        // When many inserts are happening concurrently, this error can occur on occasion
                        // When two processes try to inset the same record at once.
                        // just pause for a few milliseconds and recurse
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        println!("   Warning - get_string_id is recursing- suspect concurrent inserts for '{}'", name);
                        get_string_id(c, name, query, insert).await
                    } else {
                        Err(e.into())
                    }
                },
            }
        },
    }
}


#[cfg(test)]
mod tests {
    use tokio::runtime::Runtime;
    use pachydurable::{connect::pool_no_tls_from_env, err::PachyDarn, redis};
    use super::*;
}


