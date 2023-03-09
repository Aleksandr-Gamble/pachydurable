use std::{error::Error, fmt};
use serde::{Serialize, Deserialize};

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

