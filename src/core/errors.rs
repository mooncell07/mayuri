use std::io;
use thiserror::Error;
use uris;

use std::string::FromUtf8Error;
use strum;

#[derive(Error, Debug)]
pub enum HandshakeFailureError {
    #[error("Invalid Handshake Header")]
    HeaderError,

    #[error("Handshake Validation Failed")]
    ValidationError,
}

#[derive(Error, Debug)]
pub enum URIError {
    #[error("Incomplete URI: {0}")]
    IncompleteURIError(String),

    #[error("Malformed URI: {0}")]
    MalformedURIError(#[from] uris::Error),
}

#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("Read Error: {0}")]
    ReadError(String),

    #[error("Write Error: {0}")]
    WriteError(String),
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Error converting payload to UTF-8: {0}")]
    Utf8Error(#[from] FromUtf8Error),

    #[error("Frame Error: {0}")]
    FrameError(String),

    #[error("Invalid Event Error: {source} for `{error_event}`")]
    InvalidEventError {
        error_event: String,

        #[source]
        source: strum::ParseError,
    },
}

#[derive(Error, Debug)]
pub enum WebSocketError {
    #[error("[Handshake Failure] {0}")]
    Handshake(#[from] HandshakeFailureError),

    #[error("[URI Error] {0}")]
    Uri(#[from] URIError),

    #[error("[Connection Error] {0}")]
    Stream(#[from] ConnectionError),

    #[error("[Parse Error] {0}")]
    Parse(#[from] ParseError),

    #[error("[IO Error] {0}")]
    Io(#[from] io::Error),
}
