
use std::{error::Error};
use std::string::{FromUtf8Error, FromUtf16Error};
use std::fmt;
use std::io;

use fmt::Formatter;

use super::MinecraftPacketReader;

#[derive(Debug)]
pub enum MinecraftStringError {
    FromUtf8Error(FromUtf8Error),
    FromUtf16Error(FromUtf16Error),
    IoError(io::Error)
}

impl From<FromUtf8Error> for MinecraftStringError {
    fn from(err: FromUtf8Error) -> MinecraftStringError {
        MinecraftStringError::FromUtf8Error(err)
    }
}

impl From<FromUtf16Error> for MinecraftStringError {
    fn from(err: FromUtf16Error) -> MinecraftStringError {
        MinecraftStringError::FromUtf16Error(err)
    }
}

impl From<io::Error> for MinecraftStringError {
    fn from(err: io::Error) -> MinecraftStringError {
        MinecraftStringError::IoError(err)
    }
}

impl fmt::Display for MinecraftStringError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            MinecraftStringError::FromUtf8Error(ref err) => fmt::Display::fmt(err, f),
            MinecraftStringError::FromUtf16Error(ref err) => fmt::Display::fmt(err, f),
            MinecraftStringError::IoError(ref err) => fmt::Display::fmt(err, f),
        }
    }
}

impl Error for MinecraftStringError {
    fn description(&self) -> &str {
        match &self {
            MinecraftStringError::FromUtf8Error(ref err) => err.description(),
            MinecraftStringError::FromUtf16Error(ref err) => err.description(),
            MinecraftStringError::IoError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&dyn Error> {
        Some (
            match &self {
                MinecraftStringError::FromUtf8Error(ref err) => err as &dyn Error,
                MinecraftStringError::FromUtf16Error(ref err) => err as &dyn Error,
                MinecraftStringError::IoError(ref err) => err as &dyn Error,
            }
        )
    }
}