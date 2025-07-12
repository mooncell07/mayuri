use super::{
    errors::{HandshakeFailureError, URIError, WebSocketError},
    frame::HandshakeHeaders,
    utils::{ACCEPT_KEY_NAME, CRLF, get_host, get_resource_target},
};
use crate::safe_get_handshake_item;
use base64::{Engine, engine::general_purpose::STANDARD};
use fluent_uri::Uri;
use log::debug;
use rand::RngCore;
use sha1::{Digest, Sha1};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const __GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

pub struct Handshake<'a, R, W>
where
    W: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    reader: &'a mut R,
    pub writer: &'a mut W,
    uri: &'a Uri<String>,
}

impl<'a, R: AsyncRead + Unpin, W: AsyncWrite + Unpin> Handshake<'a, R, W> {
    pub const fn new(reader: &'a mut R, writer: &'a mut W, uri: &'a Uri<String>) -> Self {
        Self {
            reader,
            writer,
            uri,
        }
    }
    pub async fn run(&mut self) -> Result<(), WebSocketError> {
        let security_key = Self::generate_security_key();
        let handshake_payload = Self::get_handshake_payload(self.uri, security_key.as_str())?;
        self.writer.write_all(handshake_payload.as_bytes()).await?;

        debug!("Handshake Bytes sent to the server");

        let mut buf: [u8; 4096] = [0; 4096];

        self.reader.read(&mut buf).await?;
        let resp = String::from_utf8_lossy(&buf).to_string();

        debug!("Handshake Response received from the server");
        let handshake_headers = HandshakeHeaders::new(&resp)?;

        debug!(
            "Handshake Status: Version: {} | Status Code: {} | {}",
            handshake_headers.http_version,
            handshake_headers.http_status_code,
            handshake_headers.http_status_text
        );

        let accept =
            safe_get_handshake_item!(handshake_headers.headers, ACCEPT_KEY_NAME, ACCEPT_KEY_NAME)?;

        Self::validate_accept(accept.as_str(), security_key)?;
        Ok(())
    }

    fn generate_security_key() -> String {
        let mut bytes = vec![0u8; 16];
        rand::rng().fill_bytes(&mut bytes);
        STANDARD.encode(&bytes)
    }

    fn generate_valid_accept(security_key: String) -> String {
        let accept = security_key + __GUID;
        let mut hasher = Sha1::new();
        hasher.update(accept.as_bytes());
        let result = hasher.finalize();
        STANDARD.encode(result)
    }

    pub fn validate_accept(
        accept_key: &str,
        security_key: String,
    ) -> Result<(), HandshakeFailureError> {
        let valid_accept_key = Self::generate_valid_accept(security_key);
        if accept_key == valid_accept_key {
            debug!("{ACCEPT_KEY_NAME} from Server's Handshake Bytes has been validated");
            Ok(())
        } else {
            Err(HandshakeFailureError::ValidationError)
        }
    }
    fn get_handshake_payload(uri: &Uri<String>, security_key: &str) -> Result<String, URIError> {
        let maybe_auth = uri.authority();
        let auth = maybe_auth.map_or_else(
            || {
                Err(URIError::IncompleteURIError(
                    "Authority for the URI is not found".into(),
                ))
            },
            Ok,
        )?;

        let host = get_host(&auth);
        let target = get_resource_target(uri)?;

        Ok(format!(
            "GET {target} HTTP/1.1{CRLF}\
        Host: {host}{CRLF}\
        Connection: Upgrade{CRLF}\
        Upgrade: websocket{CRLF}\
        Sec-WebSocket-Key: {security_key}{CRLF}\
        Sec-WebSocket-Version: 13{CRLF}{CRLF}"
        ))
    }
}
