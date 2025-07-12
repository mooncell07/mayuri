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
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::struct_excessive_bools)]
pub mod core;
pub use async_trait;
pub use core::{
    context::Context, errors::WebSocketError, protocol::WebSocketProtocol, transport::Transport,
};

use core::{
    stream::{StreamBuilder, StreamType},
    utils::{get_socket_address, get_uri},
};

use log::{debug, info};
use std::str;

pub struct WebSocket<P: WebSocketProtocol> {
    stream: StreamType<P>,
}

impl<P: WebSocketProtocol + Send + Sync + 'static> WebSocket<P> {
    pub async fn connect(uri: &str, protocol: P) -> Result<Self, WebSocketError> {
        env_logger::init();
        let uri_obj = get_uri(String::from(uri))?;
        info!(
            "Attempting to create connection with {}",
            get_socket_address(&uri_obj)?
        );

        let stream = StreamBuilder::new(uri_obj, None)?
            .build_stream(protocol)
            .await?;

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
