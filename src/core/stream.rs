use super::enums::State;
use super::errors::ConnectionError::{ReadError, WriteError};
use super::errors::{ConnectionError, WebsocketError};
use super::handshake::Handshake;
use super::utils::get_socket_address;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uris::Uri;

pub struct Stream {
    tcp_stream: TcpStream,
    pub state: State,
}

impl Stream {
    pub async fn new(uri: &Uri) -> Result<Stream, WebsocketError> {
        let addr = get_socket_address(uri)?;
        let mut tcp_stream = TcpStream::connect(addr).await?;

        {
            let mut handshake = Handshake::new(&mut tcp_stream, uri);
            handshake.run().await?;
        }

        Ok(Self {
            tcp_stream,
            state: State::OPEN,
        })
    }

    pub async fn read(&mut self) -> Result<(), ConnectionError> {
        let mut buf: [u8; 128] = [0; 128];
        match self.state {
            State::OPEN => match self.tcp_stream.read(&mut buf).await {
                Ok(0) => {
                    self.state = State::CLOSED;
                    Err(ReadError("Unexpected EoF".into()))
                }

                Ok(_n) => Ok(()),
                Err(err) => {
                    self.state = State::CLOSED;
                    Err(ReadError(format!("Unexpected EoF: {err}")))
                }
            },
            _ => Err(ReadError(format!("Unknown State {:?}", self.state))),
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
