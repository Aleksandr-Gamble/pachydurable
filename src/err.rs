use std::{error::Error, fmt};

use tokio_postgres::Error as TokioPgError;
use hyper;
use mobc;
use redis;
use serde_json;
use hyperactive::server::ServerError;
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
    Hyperactive(ServerError),
}

impl Error for PachyDarn {}

impl fmt::Display for PachyDarn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<ServerError> for PachyDarn {
    fn from(err: ServerError) -> Self {
        PachyDarn::Hyperactive(err)
    }
}

impl From<hyper::Error> for PachyDarn {
    fn from(err: hyper::Error) -> Self {
        let srverr = ServerError::from(err);
        PachyDarn::Hyperactive(srverr)
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


/// The DiskError indicates something went wrong reading or writing to disk 
#[derive(Debug)]
pub enum DiskError {
    PG(TokioPgError),
    MissingRow,
}


impl Error for DiskError{}

impl fmt::Display for DiskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DiskError: {:?}", &self)
    }
}

impl From<TokioPgError> for DiskError {
    fn from(e: TokioPgError) -> Self {
        DiskError::PG(e)
    }
}

impl DiskError {
    pub fn missing_row() -> Self {
        DiskError::MissingRow
    }
}


/*
#[derive(Debug)]
pub struct PachyErr {
    // A very generic error. This is a bit of an antipattern,
    // but it is easier than creating a new error types for a hundred misc things
    pub message: String,
}

impl Error for PachyErr {}

impl fmt::Display for PachyErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PachyErr: {}", self.message)
    }
}

impl PachyErr {
    pub fn from_str(message: &str) -> Self {
        PachyErr{
            message: message.to_string()
        }
    }
}
*/
