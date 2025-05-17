use super::context::Context;
use super::enums::{Event, State};
use super::errors::{ConnectionError, WebSocketError};
use super::errors::{
    ConnectionError::{ReadError, WriteError},
    ParseError,
};
use super::frame::Frame;
use super::handshake::Handshake;
use super::listener::LISTENER_FUTURE_INFO_SLICE;
use super::utils::get_socket_address;
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::split;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use uris::Uri;

pub struct Stream {
    pub tcp_writer: Arc<Mutex<WriteHalf<TcpStream>>>,
    tcp_reader: ReadHalf<TcpStream>,
    pub state: State,
}

impl Stream {
    pub async fn new(uri: &Uri) -> Result<Stream, WebSocketError> {
        let addr = get_socket_address(uri)?;
        let (mut tcp_reader, mut tcp_writer) = split(TcpStream::connect(addr).await?);

        {
            let mut handshake = Handshake::new(&mut tcp_reader, &mut tcp_writer, uri);
            handshake.run().await?;
        }

        let state = State::OPEN;
        let shareable_tcp_writer = Arc::new(Mutex::new(tcp_writer));

        let stream = Self {
            tcp_writer: shareable_tcp_writer,
            tcp_reader,
            state,
        };

        stream.post_new().await?;

        Ok(stream)
    }

    pub async fn post_new(&self) -> Result<(), ParseError> {
        let ctx = Context::new(
            self.state,
            Event::OnCONNECT,
            None,
            Arc::clone(&self.tcp_writer),
        );
        self.broadcast(ctx).await?;
        Ok(())
    }

    pub async fn read(&mut self) -> Result<(), WebSocketError> {
        let mut buf: [u8; 4096] = [0; 4096];
        match self.state {
            State::OPEN => match self.tcp_reader.read(&mut buf).await {
                Ok(0) => {
                    self.state = State::CLOSED;
                    Err(WebSocketError::Stream(ReadError("Unexpected EOF".into())))
                }

                Ok(_n) => {
                    let frame = Arc::new(Frame::decode(&buf)?);
                    let writer = Arc::clone(&self.tcp_writer);
                    let ctx = Context::new(
                        self.state,
                        Event::OnMESSAGE,
                        Some(Arc::clone(&frame)),
                        writer,
                    );

                    self.broadcast(ctx).await?;

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

    pub async fn broadcast(&self, context: Context) -> Result<(), ParseError> {
        for listener_future_info in LISTENER_FUTURE_INFO_SLICE.iter() {
            let user_event = listener_future_info.belongs_to;
            let mapped_event =
                Event::from_str(user_event).map_err(|e| ParseError::InvalidEventError {
                    error_event: user_event.to_string(),
                    source: e,
                })?;
            if mapped_event == context.belongs_to {
                let ctx = context.clone();
                tokio::spawn(async move {
                    (listener_future_info.listener_future_callback)(ctx).await;
                });
            };
        }

        Ok(())
    }
}

pub async fn write_stream(
    tcp_writer: &mut WriteHalf<TcpStream>,
    state: &State,
    data: &[u8],
) -> Result<(), ConnectionError> {
    match state {
        State::OPEN => match tcp_writer.write_all(data).await {
            Ok(_n) => Ok(()),
            Err(err) => Err(WriteError(format!("Couldn't Write to the Stream: {err}"))),
        },
        _ => Err(WriteError(format!("Unknown State {:?}", state))),
    }
}
