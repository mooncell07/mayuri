use std::io;
use thiserror::Error;
use uris;

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
pub enum WebsocketError {
    #[error("[Handshake Failure] {0}")]
    Handshake(#[from] HandshakeFailureError),

    #[error("[URI Error] {0}")]
    Uri(#[from] URIError),

    #[error("[Connection Error] {0}")]
    Stream(#[from] ConnectionError),

    #[error("[IO Error] {0}")]
    Io(#[from] io::Error),
}
