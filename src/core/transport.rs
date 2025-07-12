use std::sync::Arc;
use std::sync::atomic::AtomicU8;

use crate::WebSocketError;

use super::enums::Opcode;
use super::enums::State;
use super::errors::ConnectionError;
use super::frame::Frame;
use super::utils::get_connection_state;

use log::debug;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Transport {
    writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>>,
    pub state: Arc<AtomicU8>,
}

impl Transport {
    pub fn new(
        writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>>,
        state: Arc<AtomicU8>,
    ) -> Self {
        Self { writer, state }
    }

    pub async fn write(&mut self, frame: &mut Frame) -> Result<(), WebSocketError> {
        let data = frame.encode()?;
        let state = get_connection_state(&self.state);
        match state {
            State::OPEN | State::CLOSING => match self.writer.lock().await.write_all(&data).await {
                Ok(_n) => {
                    debug!("Sent {} bytes of data", data.len());
                    Ok(())
                }
                Err(err) => Err(WebSocketError::Stream(ConnectionError::WriteError(
                    format!("Couldn't Write to the Stream: {err}"),
                ))),
            },

            State::CLOSED => Err(WebSocketError::Stream(ConnectionError::WriteError(
                String::from("Connection is Closed"),
            ))),

            _ => Err(WebSocketError::Stream(ConnectionError::WriteError(
                format!("Unknown State {:?}", self.state),
            ))),
        }
    }

    pub async fn write_text(&mut self, msg: &[u8]) -> Result<(), WebSocketError> {
        let mut frame = Frame::set_defaults(Opcode::Text, msg);
        self.write(&mut frame).await?;
        Ok(())
    }
}
