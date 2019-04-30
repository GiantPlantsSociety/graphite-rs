use std::convert::From;
use std::error::Error;
use std::fmt;
use std::num::ParseIntError;
use std::time::SystemTimeError;

#[derive(Debug, PartialEq)]
pub enum ParseError {
    RenderFormat,
    SystemTimeError(String),
    ParseIntError(ParseIntError),
    EmptyString,
    Time,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::RenderFormat => write!(f, "Format cannot be parsed"),
            ParseError::Time => write!(f, "Time cannot be parsed"),
            ParseError::SystemTimeError(s) => write!(f, "System time error: {}", s),
            ParseError::ParseIntError(s) => write!(f, "{}", s),
            ParseError::EmptyString => write!(f, "Cannot parse empty string"),
        }
    }
}

impl Error for ParseError {}

impl From<SystemTimeError> for ParseError {
    fn from(error: SystemTimeError) -> Self {
        ParseError::SystemTimeError(error.to_string())
    }
}

impl From<ParseIntError> for ParseError {
    fn from(error: ParseIntError) -> Self {
        ParseError::ParseIntError(error)
    }
}
