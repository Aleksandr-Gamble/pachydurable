use std::{error::Error, fmt};
use mobc;
use redis;
use serde_json;
pub type GenericError = Box<dyn std::error::Error + Send + Sync>;


/// This captures non-tokio_postgres error variants from MOBC 
#[derive(Debug)]
pub enum MobcErr {
    Timeout,
    BadConn,
    PoolClosed,
}

/// This error captures problems reading/writing to disk, as well as errors accessing Redis and
/// http errors 
#[derive(Debug)]
pub enum PachyDarn {
    Postgres(tokio_postgres::Error),
    MobcPG(MobcErr),
    MobcRedis(MobcErr),
    MissingRow(MissingRowError),
    Redis(redis::RedisError),
    SerdeJSON(serde_json::Error),
}

impl Error for PachyDarn {}

impl fmt::Display for PachyDarn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}


impl From<redis::RedisError> for PachyDarn {
    fn from(err: redis::RedisError) -> Self {
        PachyDarn::Redis(err)
    }
}

impl From<serde_json::Error> for PachyDarn {
    fn from(err: serde_json::Error) -> Self {
        PachyDarn::SerdeJSON(err)
    }
}


impl From<tokio_postgres::Error> for PachyDarn {
    fn from(err: tokio_postgres::Error) -> Self {
        PachyDarn::Postgres(err)
    }
}


impl From<mobc::Error<tokio_postgres::Error>> for PachyDarn {
    fn from(err: mobc::Error<tokio_postgres::Error>) -> Self {
        match err {
            mobc::Error::Inner(tpg) => PachyDarn::Postgres(tpg),
            mobc::Error::Timeout => PachyDarn::MobcPG(MobcErr::Timeout),
            mobc::Error::BadConn => PachyDarn::MobcPG(MobcErr::BadConn),
            mobc::Error::PoolClosed => PachyDarn::MobcPG(MobcErr::PoolClosed),
        }
    }
}


impl From<mobc::Error<redis::RedisError>> for PachyDarn {
    fn from(err: mobc::Error<redis::RedisError>) -> Self {
        match err {
            mobc::Error::Inner(rerr) => PachyDarn::Redis(rerr),
            mobc::Error::Timeout => PachyDarn::MobcRedis(MobcErr::Timeout),
            mobc::Error::BadConn => PachyDarn::MobcRedis(MobcErr::BadConn),
            mobc::Error::PoolClosed => PachyDarn::MobcRedis(MobcErr::PoolClosed),
        }
    }
}

impl From<MissingRowError> for PachyDarn {
    fn from(err: MissingRowError) -> Self {
        PachyDarn::MissingRow(err)
    }
}



/// Use this struct when you expect a row but there is none
#[derive(Debug)]
pub struct MissingRowError {
    pub message: String,
}

impl Error for MissingRowError {}

impl fmt::Display for MissingRowError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MissingRowError: {}", self.message)
    }
}

impl MissingRowError {
    pub fn from_str(message: &str) -> Self {
        MissingRowError{
            message: message.to_string()
        }
    }
}

