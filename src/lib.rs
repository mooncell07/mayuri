pub mod core;

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
    pub async fn connect(uri_string: &str) -> Result<WebSocket, WebSocketError> {
        let uri = get_uri(uri_string)?;
        let _stream = Stream::new(&uri).await?;

        Ok(Self { _stream })
    }

    pub async fn send(&mut self, data: &str) -> Result<(), WebSocketError> {
        let mut frame = Frame::set_defaults(Opcode::Text, data.as_bytes());
        let mut data = frame.encode()?;
        self._stream.write(&mut data).await?;

        Ok(())
    }

    pub async fn listen(&mut self) -> Result<(), WebSocketError> {
        self._stream.read().await?;

        Ok(())
    }
}
