use std::convert::From;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    // IO.
    IoError(String),

    // Dumps.
    DumpParseError(String),
    
    // Int.
    IntParseError(String),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        let msg = err.to_string();
        match err.kind() {
            _ => Error::IoError(msg),
        }
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(err: std::num::ParseIntError) -> Self {
        let msg = err.to_string();
        match err.kind() {
            _ => Error::IntParseError(msg),
        }
    }
}

impl From<std::num::TryFromIntError> for Error {
    fn from(err: std::num::TryFromIntError) -> Self {
        Error::IntParseError(err.to_string())
    }
}