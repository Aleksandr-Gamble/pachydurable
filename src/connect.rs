use std::{env, vec::Vec, error::Error, fmt, marker::Sync};
pub use tokio_postgres::{Config, NoTls, row::Row, Error as ErrorTKPG};
use tokio_postgres::{types::ToSql}; // can't pub use ToSql as it is private
pub use tokio_postgres::GenericClient;
pub use mobc::{self, Pool};
pub use mobc_postgres::PgConnectionManager;
use crate::err::{GenericError, MissingRowError};


/// The ConnPoolNoTLS a common connector used for various applications
/// It can be cloned for thread-safe http servers etc.
/// In the future, this should probably switch to Tls
pub type ConnPoolNoTLS = Pool<PgConnectionManager<NoTls>>;
/// The client is also notls and should be changed in the future
pub type ClientNoTLS = mobc::Connection<PgConnectionManager<NoTls>>;


/// return an option<T>
pub async fn get_opt<'a, T>(client: &'a ClientNoTLS, query: &'static str, rowfunc: &'a dyn Fn(&Row) -> T, params: &'a [&'a (dyn ToSql + Sync)]) -> Result<Option<T>, GenericError> {
    let rows = client.query(query, params).await?;
    match rows.get(0) {
        None => Ok(None),
        Some(row) => Ok(Some(rowfunc(row))) // see https://users.rust-lang.org/t/how-to-store-function-pointers-in-struct-and-call-them/51348
    }
}

/// return T
pub async fn get_one<'a, T>(client: &'a ClientNoTLS, query: &'static str, rowfunc: &'a dyn Fn(&Row) -> T, params:&'a [&'a (dyn ToSql + Sync)]) -> Result<T, GenericError> {
    let t: T = match get_opt(client, query, rowfunc, params).await? {
        Some(t) => t,
        None => return Err(MissingRowError{message: format!("No row found for query \"{}\"", query)}.into())
    };
    Ok(t)
}


/// This cool function takes a references to a pool and a query and returns a vec of results
pub async fn get_vec<'a, T>(client: &'a ClientNoTLS, query: &'static str, rowfunc: &'a dyn Fn(&Row) -> T, params:&'a[&'a(dyn ToSql + Sync)]) -> Result<Vec<T>, GenericError> {
    let rows = client.query(query, params).await?;
    let mut vt = Vec::new();
    for row in rows {
        let t = rowfunc(&row);
        vt.push(t);
    }
    Ok(vt)
}


/// create a new Pool from environment variables
pub async fn pool_no_tls_from_env() -> Result<ConnPoolNoTLS, GenericError> {
    let config = SimpleConfig::new_from_env();
    pool_no_tls_from_config(&config).await
}

/// create a new Pool from a SimpleConfig
pub async fn pool_no_tls_from_config(config: &SimpleConfig) -> Result<ConnPoolNoTLS, GenericError> {
    let mut pg_config = Config::new();
    pg_config.user(&config.user);
    pg_config.password(&config.password);
    pg_config.dbname(&config.database);
    pg_config.host(&config.host);
    pg_config.port(config.port);
    // instantiate a manager and a pool
    let manager = PgConnectionManager::new(pg_config, NoTls);
    let pool = Pool::builder().max_open(20).max_idle(5).build(manager);
    // ensure you can connect now instead of throwing an 
    let _client: ClientNoTLS = pool.get().await?; // No ensure you can connect
    Ok(pool)
}

/// This struct describes how to connect to an instance using host/port/passwords etc.
pub struct SimpleConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
}

impl SimpleConfig {

    /// Instantiate a new SimpleConfig from a provided database and user name,
    /// Sourcing other parameters from environment variables
    pub fn new_from_db_user_env(database: &str, user: &str) -> Self {
        let host = match env::var("PSQL_HOST") {
            Ok(var) => var,
            Err(_) => "127.0.0.1".to_string(),
        };
        let port = match env::var("PSQL_PORT") {
            Ok(var) => var,
            Err(_) => "5432".to_string(),
        };
        let password = match env::var("PSQL_PW") {
            Ok(var) => var,
            Err(_) => "".to_string(),
        };
        SimpleConfig {
            host: host,
            port: port.parse::<u16>().unwrap(),
            user: user.to_string(),
            password: password,
            database: database.to_string(),
        }
    }


    /// Instantiate a new SimpleConfig purely from environment variables
    pub fn new_from_env() -> Self {
        let user = match env::var("PSQL_USER") {
            Ok(var) => var,
            Err(_) => "postgres".to_string(),
        };
        let database = match env::var("PSQL_DB") {
            Ok(var) => var,
            Err(_) => "postgres".to_string(),
        };
        SimpleConfig::new_from_db_user_env(&database, &user)
    }
}


pub fn ts_expression(phrase: &str) -> String {
    // Given a phrase like "crimson thread", convert it to a TS expression
    let mut prefixes = Vec::new();
    for word in phrase.to_lowercase().split_whitespace() {
        let mut prefix = word.to_string();
        prefix.push_str(":*");
        prefixes.push(prefix);
    }
    let ts_expression = prefixes.join(" & ");
    ts_expression
}



