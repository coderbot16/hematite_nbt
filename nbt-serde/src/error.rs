use std::error;
use std::fmt;
use std::io;
use std::result;
use std::string;

use serde;

pub type Result<T> = result::Result<T, Error>;

// TODO: HeterogenousList
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Serde(String),
    NoRootCompound,
    UnknownTag(u8),
    NonBooleanByte(i8),
    UnexpectedTag(u8, u8),
    UnrepresentableType(&'static str),
    InvalidUtf8,
    IncompleteNbtValue
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            Error::Io(ref err) => fmt::Display::fmt(err, f),
            Error::Serde(ref msg) => f.write_str(msg),
            Error::NoRootCompound => {
                f.write_str("all values must have a root compound")
            },
            Error::UnknownTag(t) => {
                write!(f, "unknown tag: {}", t)
            },
            Error::NonBooleanByte(b) => {
                write!(f, "boolean bytes must be 0 or 1, found {}", b)
            },
            Error::UnexpectedTag(a, b) => {
                write!(f, "unexpected tag: {}, expecting: {}", a, b)
            },
            Error::UnrepresentableType(t) => {
                write!(f, "cannot represent {} in NBT format", t)
            },
            Error::InvalidUtf8 => write!(f, "a string is not valid UTF-8"),
            Error::IncompleteNbtValue => write!(f, "data does not represent a complete NbtValue")
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
    	if err.kind() == io::ErrorKind::UnexpectedEof {
    		Error::IncompleteNbtValue
    	} else {
    		Error::Io(err)
    	}
    }
}

impl From<string::FromUtf8Error> for Error {
    fn from(_: string::FromUtf8Error) -> Error {
        Error::InvalidUtf8
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Io(_) => "IO error",
            Error::Serde(ref msg) => &msg[..],
            Error::NoRootCompound => "all values must have a root compound",
            Error::UnknownTag(_) => "unknown tag",
            Error::NonBooleanByte(_) =>
                "encountered a non-0 or 1 byte for a boolean",
            Error::UnexpectedTag(_, _) => "unexpected tag",
            Error::UnrepresentableType(_) => "unrepresentable type",
            Error::InvalidUtf8 => "a string is not valid UTF-8",
            Error::IncompleteNbtValue => "data does not represent a complete NbtValue"
        }
    }
}

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Error {
        Error::Serde(msg.to_string())
    }
}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Error {
        Error::Serde(msg.to_string())
    }
}
