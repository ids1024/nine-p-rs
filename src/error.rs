#[cfg(not(feature = "std"))]
use alloc::string::String;
use core::{fmt, str};

#[derive(Debug)]
pub enum Error {
    #[cfg(feature = "std")]
    Io(std::io::Error),
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

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(feature = "std")]
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<str::Utf8Error> for Error {
    fn from(error: str::Utf8Error) -> Self {
        Self::Utf8(error)
    }
}
