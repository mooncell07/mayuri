use super::context::Context;
use super::enums::{Event, State};
use super::errors::ConnectionError::{ReadError, WriteError};
use super::errors::{ConnectionError, WebSocketError};
use super::frame::Frame;
use super::handshake::Handshake;
use super::utils::get_socket_address;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uris::Uri;

pub struct Stream {
    tcp_stream: TcpStream,
    pub state: State,
    callbacks: Vec<fn(&Context)>,
}

impl Stream {
    pub async fn new(uri: &Uri, callbacks: Vec<fn(&Context)>) -> Result<Stream, WebSocketError> {
        let addr = get_socket_address(uri)?;
        let mut tcp_stream = TcpStream::connect(addr).await?;

        {
            let mut handshake = Handshake::new(&mut tcp_stream, uri);
            handshake.run().await?;
        }

        Ok(Self {
            tcp_stream,
            state: State::OPEN,
            callbacks,
        })
    }

    pub async fn read(&mut self) -> Result<(), WebSocketError> {
        let mut buf: [u8; 4096] = [0; 4096];
        match self.state {
            State::OPEN => match self.tcp_stream.read(&mut buf).await {
                Ok(0) => {
                    self.state = State::CLOSED;
                    Err(WebSocketError::Stream(ReadError("Unexpected EOF".into())))
                }

                Ok(_n) => {
                    let frame = Frame::decode(&buf)?;
                    let ctx = Context::new(Event::OnMESSAGE, frame);
                    for func in self.callbacks.iter() {
                        func(&ctx);
                    }
                    Ok(())
                }
                Err(err) => {
                    self.state = State::CLOSED;
                    Err(WebSocketError::Stream(ReadError(format!(
                        "Unexpected EOF: {err}"
                    ))))
                }
            },
            _ => Err(WebSocketError::Stream(ReadError(format!(
                "Unknown State {:?}",
                self.state
            )))),
        }
    }

    pub async fn write(&mut self, data: &mut [u8]) -> Result<(), ConnectionError> {
        match self.state {
            State::OPEN => match self.tcp_stream.write_all(data).await {
                Ok(_n) => Ok(()),
                Err(err) => Err(WriteError(format!("Couldn't Write to the Stream: {err}"))),
            },
            _ => Err(WriteError(format!("Unknown State {:?}", self.state))),
        }
    }
}
