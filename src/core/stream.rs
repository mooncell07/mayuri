use crate::core::utils::set_connection_state;

use super::{
    context::Context,
    enums::{Opcode, State},
    errors::{
        ConnectionError::{self, ReadError},
        ParseError, URIError, WebSocketError,
    },
    frame::{Frame, Headers},
    handshake::Handshake,
    protocol::WebSocketProtocol,
    transport::Transport,
    utils::{get_connection_state, get_host, get_socket_address, is_secured},
};
use fluent_uri::Uri;
use log::{debug, info};
use rustls_pki_types::{CertificateDer, ServerName, pem::PemObject};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU8;
use tokio::sync::Mutex;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadHalf, split},
    net::TcpStream,
    spawn,
};
use tokio_rustls::{
    TlsConnector,
    client::TlsStream,
    rustls::{ClientConfig, RootCertStore},
};
use webpki_roots;

pub struct Stream<P: WebSocketProtocol, R> {
    user_protocol: Arc<Mutex<P>>,
    reader: R,
    transport: Transport,
    pub state: Arc<AtomicU8>,
}

impl<P: WebSocketProtocol + Send + Sync + 'static, R: AsyncRead + Unpin> Stream<P, R> {
    pub async fn new<W: AsyncWrite + Unpin + Send + 'static>(
        user_protocol: P,
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

        let state = Arc::new(AtomicU8::new(State::OPEN.as_u8()));
        let transport = Self::get_transport(writer, Arc::clone(&state))?;
        let user_protocol = Arc::new(Mutex::new(user_protocol));
        let mut stream = Self {
            user_protocol,
            reader,
            transport,
            state,
        };

        stream.post_init();

        info!("Connection established with {}", get_socket_address(uri)?);

        Ok(stream)
    }

    pub fn get_transport<W: AsyncWrite + Unpin + Send + 'static>(
        writer: W,
        state: Arc<AtomicU8>,
    ) -> Result<Transport, ParseError> {
        let transport = Transport::new(Arc::new(Mutex::new(Box::new(writer))), state);

        Ok(transport)
    }

    pub fn post_init(&mut self) {
        let proto = Arc::clone(&self.user_protocol);
        let transport = self.transport.clone();
        spawn(async move { proto.lock().await.on_connect(transport).await });
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
        let state = get_connection_state(&self.state);
        match state {
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
                        set_connection_state(State::CLOSED, &self.state);
                        Err(WebSocketError::Stream(ReadError("Unexpected EOF".into())))
                    }

                    Ok(n) => {
                        debug!("Received {n} bytes of data");

                        let frame = Frame::decode(&buf, headers)?;
                        let opcode = frame.headers.opcode;
                        let ctx = Context::new(frame)?;

                        let proto = Arc::clone(&self.user_protocol);
                        if opcode == Opcode::Close {
                            debug!("WebSocket Connection is closing now");

                            set_connection_state(State::CLOSING, &self.state);
                            let mut frame = Frame::set_defaults(Opcode::Close, b"1000");
                            self.transport.write(&mut frame).await?;
                            tokio::spawn(async move { proto.lock().await.on_close(ctx).await });
                        } else {
                            tokio::spawn(async move { proto.lock().await.on_message(ctx).await });
                        }

                        Ok(())
                    }
                    Err(err) => {
                        set_connection_state(State::CLOSED, &self.state);
                        Err(WebSocketError::Stream(ReadError(format!(
                            "Unexpected EOF: {err}"
                        ))))
                    }
                }
            }

            State::CLOSED => Err(WebSocketError::Stream(ConnectionError::ReadError(
                String::from("Connection is Closed"),
            ))),
            _ => Err(WebSocketError::Stream(ReadError(format!(
                "Unknown State {:?}",
                self.state
            )))),
        }
    }
}

pub enum StreamType<P: WebSocketProtocol> {
    Plain(Stream<P, ReadHalf<TcpStream>>),
    Secured(Stream<P, ReadHalf<TlsStream<TcpStream>>>),
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

    async fn create_secured_stream<P: WebSocketProtocol + Send + Sync + 'static>(
        &self,
        user_protocol: P,
        uri: &Uri<String>,
    ) -> Result<Stream<P, ReadHalf<TlsStream<TcpStream>>>, WebSocketError> {
        let addr = get_socket_address(uri)?;
        let tcp_stream = TcpStream::connect(addr).await?;
        let tls_stream = self.wrap_tls(tcp_stream, uri).await?;

        let (tls_reader, tls_writer) = split(tls_stream);
        Stream::new(user_protocol, tls_reader, tls_writer, uri).await
    }

    async fn create_plain_stream<P: WebSocketProtocol + Send + Sync + 'static>(
        &self,
        user_protocol: P,
        uri: &Uri<String>,
    ) -> Result<Stream<P, ReadHalf<TcpStream>>, WebSocketError> {
        let addr = get_socket_address(uri)?;
        let tcp_stream = TcpStream::connect(addr).await?;

        let (tcp_reader, tcp_writer) = split(tcp_stream);
        Stream::new(user_protocol, tcp_reader, tcp_writer, uri).await
    }

    pub async fn build_stream<P: WebSocketProtocol + Send + Sync + 'static>(
        &self,
        user_protocol: P,
    ) -> Result<StreamType<P>, WebSocketError> {
        let stream_type = if is_secured(&self.uri) {
            StreamType::Secured(self.create_secured_stream(user_protocol, &self.uri).await?)
        } else {
            StreamType::Plain(self.create_plain_stream(user_protocol, &self.uri).await?)
        };
        Ok(stream_type)
    }
}
