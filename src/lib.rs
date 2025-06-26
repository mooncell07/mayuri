#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![deny(clippy::nursery)]
#![deny(clippy::cargo)]
#![deny(clippy::pedantic)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::indexing_slicing)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_safety_doc)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::struct_excessive_bools)]
pub mod core;

pub use core::{context::Context, enums::Event, listener};
use core::{
    errors::WebSocketError,
    listener::LISTENER_FUTURE_INFO_SLICE,
    stream::{StreamBuilder, StreamType},
    utils::{get_socket_address, get_uri},
};
use log::{debug, info};
pub use registry::bind;
use std::str;

pub struct WebSocket {
    stream: StreamType,
}

impl WebSocket {
    pub async fn connect(uri_string: &str) -> Result<Self, WebSocketError> {
        env_logger::init();
        info!(
            "Registered `{}` Listener(s)",
            LISTENER_FUTURE_INFO_SLICE.len()
        );
        let uri = get_uri(String::from(uri_string))?;
        info!(
            "Attempting to create connection with {}",
            get_socket_address(&uri)?
        );

        let stream = StreamBuilder::new(uri, None)?.build_stream().await?;
        Ok(Self { stream })
    }

    pub async fn run(&mut self) -> Result<(), WebSocketError> {
        debug!("Starting Event Loop");
        match &mut self.stream {
            StreamType::Plain(plain_stream) => loop {
                plain_stream.read().await?;
            },
            StreamType::Secured(secured_stream) => loop {
                secured_stream.read().await?;
            },
        };
    }
}
