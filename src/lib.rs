pub mod core;

pub use core::context::Context;
pub use core::enums::Event;
pub use core::listener;
pub use core::stream;
pub use registry::bind;

use core::enums::Opcode;
use core::errors::WebSocketError;
use core::frame::Frame;
use core::stream::{Stream, write_stream};
use core::utils::get_uri;
use std::str;

pub struct WebSocket {
    pub _stream: Stream,
}

impl WebSocket {
    pub async fn connect(uri_string: &str) -> Result<WebSocket, WebSocketError> {
        let uri = get_uri(uri_string)?;
        let _stream = Stream::new(&uri).await?;

        Ok(Self { _stream })
    }

    pub async fn run(&mut self) -> Result<(), WebSocketError> {
        loop {
            self._stream.read().await?
        }
    }
}
