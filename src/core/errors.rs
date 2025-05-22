use fluent_uri::error;
use rustls_pki_types::InvalidDnsNameError;
use std::{io, num::ParseIntError, string::FromUtf8Error};
use strum;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HandshakeFailureError {
    #[error("Invalid Handshake Header: {0}")]
    HeaderError(String),

    #[error("Handshake Validation Failed")]
    ValidationError,
}

#[derive(Error, Debug)]
pub enum URIError {
    #[error("Incomplete URI: {0}")]
    IncompleteURIError(String),

    #[error("Malformed URI {0}")]
    MalformedURIError(String),

    #[error("URI couldnt be resolved")]
    ResolveError(#[from] error::ResolveError),

    #[error("DNS Error: {0}")]
    DNSError(#[from] InvalidDnsNameError),

    #[error("Bad Port Error: {0}")]
    BadPortError(#[from] ParseIntError),
}

#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("Read Error: {0}")]
    ReadError(String),

    #[error("Write Error: {0}")]
    WriteError(String),

    #[error("Bad Connector: {0}")]
    ConnectorError(String),
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
