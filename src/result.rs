use std::io;

#[derive(Debug)]
pub enum Error {
    EOF,
    IOError(io::Error),
    EncodingError,
    InvalidFormat(&'static str),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IOError(error)
    }
}

pub type Result<T> = core::result::Result<T, Error>;
