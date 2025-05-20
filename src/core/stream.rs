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
use super::utils::{get_host, get_socket_address, is_secured};
use log::{debug, info};
use rustls_pki_types::ServerName;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite, split};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use uris::Uri;
use webpki_roots;

pub struct Stream<R> {
    reader: R,
    pub writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send + 'static>>>,
    pub state: State,
}

impl<R: AsyncRead + Unpin> Stream<R> {
    pub async fn new<W: AsyncWrite + Unpin + Send + 'static>(
        mut reader: R,
        mut writer: W,
        uri: &Uri,
    ) -> Result<Stream<R>, WebSocketError> {
        debug!("Running handshake");
        {
            let mut handshake = Handshake::new(&mut reader, &mut writer, uri);
            handshake.run().await?;
        }
        debug!("Handshake complete");

        let stream = Self {
            writer: Arc::new(Mutex::new(Box::new(writer))),
            reader,
            state: State::OPEN,
        };

        info!("Connection established with {}", get_socket_address(uri)?);

        stream.post_new().await?;

        Ok(stream)
    }

    pub async fn post_new(&self) -> Result<(), ParseError> {
        let ctx = Context::new(self.state, Event::OnCONNECT, None, Arc::clone(&self.writer));
        self.broadcast(ctx).await?;
        Ok(())
    }

    pub async fn read(&mut self) -> Result<(), WebSocketError> {
        let mut buf: [u8; 4096] = [0; 4096];
        match self.state {
            State::OPEN => match self.reader.read(&mut buf).await {
                Ok(0) => {
                    self.state = State::CLOSED;
                    Err(WebSocketError::Stream(ReadError("Unexpected EOF".into())))
                }

                Ok(_n) => {
                    let frame = Arc::new(Frame::decode(&buf)?);
                    let writer = Arc::clone(&self.writer);
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
        info!("Broadcasting Event: `{:?}`", context.belongs_to);
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

pub enum StreamType {
    Plain(Stream<ReadHalf<TcpStream>>),
    Secured(Stream<ReadHalf<TlsStream<TcpStream>>>),
}

pub async fn write_stream(
    tcp_writer: &mut Box<(dyn AsyncWrite + Send + Unpin + 'static)>,
    state: &State,
    data: &[u8],
) -> Result<(), ConnectionError> {
    match state {
        State::OPEN => match tcp_writer.write_all(data).await {
            Ok(_n) => {
                debug!("Sent {} bytes of data", data.len());
                Ok(())
            }
            Err(err) => Err(WriteError(format!("Couldn't Write to the Stream: {err}"))),
        },
        _ => Err(WriteError(format!("Unknown State {:?}", state))),
    }
}

pub struct StreamBuilder {
    uri: Uri,
    certs: Option<PathBuf>,
}

impl StreamBuilder {
    pub fn new(uri: Uri, certs: Option<PathBuf>) -> StreamBuilder {
        Self { uri, certs }
    }

    fn _get_tls_config(&self) -> ClientConfig {
        let mut root_cert_store = RootCertStore::empty();
        root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = ClientConfig::builder()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth();
        config
    }

    async fn _wrap_tls(
        &self,
        tcp_stream: TcpStream,
        uri: &Uri,
    ) -> Result<TlsStream<TcpStream>, WebSocketError> {
        let tls_config = self._get_tls_config();
        let tls_connector = TlsConnector::from(Arc::new(tls_config));
        let dnsname = ServerName::try_from(get_host(uri)?)
            .map_err(|e| WebSocketError::Uri(super::errors::URIError::DNSError(e)))?;
        let tls_stream = tls_connector.connect(dnsname, tcp_stream).await?;
        Ok(tls_stream)
    }

    async fn _create_secured_stream(
        &self,
        uri: &Uri,
    ) -> Result<Stream<ReadHalf<TlsStream<TcpStream>>>, WebSocketError> {
        let addr = get_socket_address(uri)?;
        let tcp_stream = TcpStream::connect(addr).await?;
        let tls_stream = self._wrap_tls(tcp_stream, uri).await?;

        let (tls_reader, tls_writer) = split(tls_stream);
        Ok(Stream::new(tls_reader, tls_writer, uri).await?)
    }

    async fn _create_plain_stream(
        &self,
        uri: &Uri,
    ) -> Result<Stream<ReadHalf<TcpStream>>, WebSocketError> {
        let addr = get_socket_address(uri)?;
        let tcp_stream = TcpStream::connect(addr).await?;

        let (tcp_reader, tcp_writer) = split(tcp_stream);
        Ok(Stream::new(tcp_reader, tcp_writer, uri).await?)
    }

    pub async fn build_stream(&self) -> Result<StreamType, WebSocketError> {
        let stream_type = if is_secured(&self.uri)? {
            StreamType::Secured(self._create_secured_stream(&self.uri).await?)
        } else {
            StreamType::Plain(self._create_plain_stream(&self.uri).await?)
        };
        Ok(stream_type)
    }
}
