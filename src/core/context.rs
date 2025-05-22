use super::{
    enums::{Event, Opcode, State},
    errors::{ConnectionError, ParseError, WebSocketError},
    frame::Frame,
    stream::write_stream,
};
use std::sync::Arc;
use tokio::{io::AsyncWrite, sync::Mutex};

#[derive(Clone)]
pub struct Context {
    pub state: State,
    pub belongs_to: Event,
    frame: Option<Arc<Frame>>,
    tcp_writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>>,
}

impl Context {
    pub fn new(
        state: State,
        event: Event,
        frame: Option<Arc<Frame>>,
        tcp_writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>>,
    ) -> Self {
        Self {
            state,
            belongs_to: event,
            frame,
            tcp_writer,
        }
    }

    pub fn read_text(&self) -> Result<String, WebSocketError> {
        match self.belongs_to {
            Event::OnMESSAGE => self.frame.as_ref().map_or_else(
                || {
                    Err(WebSocketError::Parse(ParseError::FrameError(String::from(
                        "Frame not available",
                    ))))
                },
                |f| Ok(String::from_utf8_lossy(&f.payload_data).to_string()),
            ),
            _ => Err(WebSocketError::Stream(ConnectionError::ReadError(format!(
                "Attempted to read TEXT data from a Context that belongs to {:?} Event",
                self.belongs_to
            )))),
        }
    }

    pub async fn write_text(&self, msg: &str) -> Result<(), WebSocketError> {
        let mut frame = Frame::set_defaults(Opcode::Text, msg.as_bytes());
        let data = frame.encode()?;
        let mut writer = self.tcp_writer.lock().await;
        write_stream(&mut writer, &self.state, &data).await?;

        Ok(())
    }
}
