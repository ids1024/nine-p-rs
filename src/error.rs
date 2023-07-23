use std::{fmt, io, str};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Utf8(str::Utf8Error),
    MessageLength,
    UnrecognizedTag(u16),
    UnexpectedType(u8),
    Protocol(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self) // XXX
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<str::Utf8Error> for Error {
    fn from(error: str::Utf8Error) -> Self {
        Self::Utf8(error)
    }
}
