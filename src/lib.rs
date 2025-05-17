pub mod core;

pub use core::context::Context;
pub use core::enums::Event;
pub use core::listener;
pub use core::stream;
pub use registry::bind;

use core::errors::WebSocketError;
use core::stream::Stream;
use core::utils::{get_socket_address, get_uri};
use listener::LISTENER_FUTURE_INFO_SLICE;
use log::{debug, info};
use std::str;

pub struct WebSocket {
    pub _stream: Stream,
}

impl WebSocket {
    pub async fn connect(uri_string: &str) -> Result<WebSocket, WebSocketError> {
        env_logger::init();
        info!(
            "Registered {} Listener(s)",
            LISTENER_FUTURE_INFO_SLICE.len()
        );
        let uri = get_uri(uri_string)?;
        info!(
            "Attempting to create connection with {}",
            get_socket_address(&uri)?
        );
        let _stream = Stream::new(&uri).await?;

        Ok(Self { _stream })
    }

    pub async fn run(&mut self) -> Result<(), WebSocketError> {
        debug!("Starting Event Loop");
        loop {
            self._stream.read().await?
        }
    }
}
