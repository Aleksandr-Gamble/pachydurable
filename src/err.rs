use std::{error::Error, fmt};

use tokio_postgres::Error as TokioPgError;

pub type GenericError = Box<dyn std::error::Error + Send + Sync>;



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

