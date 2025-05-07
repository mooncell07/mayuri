use super::enums::Opcode;
use super::errors::WebsocketError;
use super::frame::Frame;
use super::stream::Stream;
use super::utils::get_uri;

use std::str;

pub struct Websocket {
    stream: Stream,
}

impl Websocket {
    pub async fn connect(uri_string: &str) -> Result<Websocket, WebsocketError> {
        let uri = get_uri(uri_string)?;
        let stream = Stream::new(&uri).await?;

        Ok(Self { stream })
    }

    pub async fn send(&mut self, data: &str) -> Result<(), WebsocketError> {
        let mut frame = Frame::set_defaults(Opcode::Text, data.as_bytes());
        let mut data = frame.encode()?;
        self.stream.write(&mut data).await?;

        Ok(())
    }

    pub async fn listen(&mut self) -> Result<(), WebsocketError> {
        self.stream.read().await?;

        Ok(())
    }
}
