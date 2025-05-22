use super::{
    context::Context,
    enums::{Event, Opcode, State},
    errors::{
        ConnectionError::{self, ReadError, WriteError},
        ParseError, URIError, WebSocketError,
    },
    frame::{Frame, Headers},
    handshake::Handshake,
    listener::LISTENER_FUTURE_INFO_SLICE,
    utils::{get_host, get_socket_address, is_secured},
};
use fluent_uri::Uri;
use log::{debug, info};
use rustls_pki_types::{CertificateDer, ServerName, pem::PemObject};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, split},
    net::TcpStream,
};
use tokio_rustls::{
    TlsConnector,
    client::TlsStream,
    rustls::{ClientConfig, RootCertStore},
};
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
        uri: &Uri<String>,
    ) -> Result<Self, WebSocketError> {
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

        stream.post_new()?;

        Ok(stream)
    }

    pub fn post_new(&self) -> Result<(), ParseError> {
        let ctx = Context::new(self.state, Event::OnCONNECT, None, Arc::clone(&self.writer));
        self.broadcast(&ctx)?;
        Ok(())
    }

    pub async fn fetch_headers(&mut self) -> Result<Headers, WebSocketError> {
        let mut buf: [u8; 2] = [0; 2];
        match self.reader.read_exact(&mut buf).await {
            Ok(0) => Err(WebSocketError::Stream(ReadError(
                "Couldn't read Frame Headers".into(),
            ))),
            Ok(n) => {
                debug!("Received Frame Headers of length {n}");

                let mut headers = Headers::decode(&buf)?;

                let payload_len_ext = if headers.extend_by == 16 {
                    u64::from(self.reader.read_u16().await?)
                } else if headers.extend_by == 64 {
                    self.reader.read_u64().await?
                } else {
                    0u64
                };
                headers.payload_len_ext = payload_len_ext;
                Ok(headers)
            }
            Err(err) => Err(WebSocketError::Stream(ReadError(format!(
                "Unexpected EOF while reading Frame Headers: {err}"
            )))),
        }
    }

    pub async fn read(&mut self) -> Result<(), WebSocketError> {
        match self.state {
            State::OPEN => {
                let headers = self.fetch_headers().await?;

                let final_payload_len = {
                    if headers.extend_by > 0 {
                        headers.payload_len_ext
                    } else {
                        u64::from(headers.payload_len)
                    }
                };

                debug!("Frame Headers: {headers:?}");

                let mut buf = vec![0u8; final_payload_len as usize];

                match self.reader.read_exact(&mut buf).await {
                    Ok(0) => {
                        self.state = State::CLOSED;
                        Err(WebSocketError::Stream(ReadError("Unexpected EOF".into())))
                    }

                    Ok(n) => {
                        debug!("Received {n} bytes of data");

                        let frame = Arc::new(Frame::decode(&buf, headers)?);
                        let writer = Arc::clone(&self.writer);
                        let ctx = Context::new(
                            self.state,
                            Event::OnMESSAGE,
                            Some(Arc::clone(&frame)),
                            writer,
                        );
                        self.broadcast(&ctx)?;

                        if frame.headers.opcode == Opcode::Close {
                            self.state = State::CLOSING;
                        }

                        Ok(())
                    }
                    Err(err) => {
                        self.state = State::CLOSED;
                        Err(WebSocketError::Stream(ReadError(format!(
                            "Unexpected EOF: {err}"
                        ))))
                    }
                }
            }
            _ => Err(WebSocketError::Stream(ReadError(format!(
                "Unknown State {:?}",
                self.state
            )))),
        }
    }

    pub fn broadcast(&self, context: &Context) -> Result<(), ParseError> {
        info!("Broadcasting Event: `{:?}`", context.belongs_to);
        for listener_future_info in LISTENER_FUTURE_INFO_SLICE {
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
        _ => Err(WriteError(format!("Unknown State {state:?}"))),
    }
}

pub enum StreamType {
    Plain(Stream<ReadHalf<TcpStream>>),
    Secured(Stream<ReadHalf<TlsStream<TcpStream>>>),
}

pub struct StreamBuilder {
    uri: Uri<String>,
    cafile: Option<PathBuf>,
}

impl StreamBuilder {
    pub fn new(uri: Uri<String>, cert_path: Option<String>) -> Result<Self, WebSocketError> {
        let cafile = cert_path.map(PathBuf::from);

        Ok(Self { uri, cafile })
    }

    fn get_tls_config(&self) -> Result<ClientConfig, ConnectionError> {
        let mut root_cert_store = RootCertStore::empty();
        match &self.cafile {
            Some(n) => {
                for cert in CertificateDer::pem_file_iter(n).into_iter().flatten() {
                    let _ = match cert {
                        Ok(n) => Ok(root_cert_store.add(n)),
                        Err(e) => Err(ConnectionError::ConnectorError(e.to_string())),
                    }?;
                }
            }
            None => root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned()),
        }
        Ok(ClientConfig::builder()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth())
    }

    async fn wrap_tls(
        &self,
        tcp_stream: TcpStream,
        uri: &Uri<String>,
    ) -> Result<TlsStream<TcpStream>, WebSocketError> {
        let tls_config = self.get_tls_config()?;
        let tls_connector = TlsConnector::from(Arc::new(tls_config));
        let maybe_auth = uri.authority();
        let auth = maybe_auth.map_or_else(
            || {
                Err(URIError::IncompleteURIError(
                    "Authority for the URI is not found".into(),
                ))
            },
            Ok,
        )?;
        let dnsname = ServerName::try_from(get_host(&auth))
            .map_err(|e| WebSocketError::Uri(super::errors::URIError::DNSError(e)))?;
        let tls_stream = tls_connector.connect(dnsname, tcp_stream).await?;
        Ok(tls_stream)
    }

    async fn create_secured_stream(
        &self,
        uri: &Uri<String>,
    ) -> Result<Stream<ReadHalf<TlsStream<TcpStream>>>, WebSocketError> {
        let addr = get_socket_address(uri)?;
        let tcp_stream = TcpStream::connect(addr).await?;
        let tls_stream = self.wrap_tls(tcp_stream, uri).await?;

        let (tls_reader, tls_writer) = split(tls_stream);
        Stream::new(tls_reader, tls_writer, uri).await
    }

    async fn create_plain_stream(
        &self,
        uri: &Uri<String>,
    ) -> Result<Stream<ReadHalf<TcpStream>>, WebSocketError> {
        let addr = get_socket_address(uri)?;
        let tcp_stream = TcpStream::connect(addr).await?;

        let (tcp_reader, tcp_writer) = split(tcp_stream);
        Stream::new(tcp_reader, tcp_writer, uri).await
    }

    pub async fn build_stream(&self) -> Result<StreamType, WebSocketError> {
        let stream_type = if is_secured(&self.uri) {
            StreamType::Secured(self.create_secured_stream(&self.uri).await?)
        } else {
            StreamType::Plain(self.create_plain_stream(&self.uri).await?)
        };
        Ok(stream_type)
    }
}
