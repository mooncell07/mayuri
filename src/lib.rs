pub mod core;
pub use core::context::Context;
use core::enums::Opcode;
use core::errors::WebSocketError;
use core::frame::Frame;
use core::stream::Stream;
use core::utils::get_uri;

use std::str;

pub struct WebSocket {
    pub _stream: Stream,
}

impl WebSocket {
    pub async fn connect(
        uri_string: &str,
        callbacks: Vec<fn(&Context)>,
    ) -> Result<WebSocket, WebSocketError> {
        let uri = get_uri(uri_string)?;
        let _stream = Stream::new(&uri, callbacks).await?;

        Ok(Self { _stream })
    }

    pub async fn send(&mut self, data: &str) -> Result<(), WebSocketError> {
        let mut frame = Frame::set_defaults(Opcode::Text, data.as_bytes());
        let mut data = frame.encode()?;
        self._stream.write(&mut data).await?;

        Ok(())
    }

    pub async fn run(&mut self) -> Result<(), WebSocketError> {
        loop {
            self._stream.read().await?
        }
    }
}
