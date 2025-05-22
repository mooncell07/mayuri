use super::errors::URIError;
use fluent_uri::{Uri, component::Authority, encoding::EStr};
pub const CRLF: &str = "\r\n";
pub const DEFAULT_PORT_SECURE: u16 = 443;
pub const DEFAULT_PORT_INSECURE: u16 = 80;

pub const ACCEPT_KEY_NAME: &str = "sec-websocket-accept";

pub fn get_uri(uri_string: String) -> Result<Uri<String>, URIError> {
    Uri::parse(uri_string).map_err(|e| URIError::MalformedURIError(e.to_string()))
}

#[must_use]
pub fn get_host(auth: &Authority<'_>) -> String {
    auth.host().to_owned()
}

#[must_use]
pub fn is_secured(uri: &Uri<String>) -> bool {
    let scheme = uri.scheme().to_string();
    scheme == "wss"
}

pub fn get_port(uri: &Uri<String>) -> Result<u16, URIError> {
    let auth = uri.authority();
    match auth {
        Some(a) => a.port_to_u16()?.map_or_else(
            || {
                if is_secured(uri) {
                    Ok(DEFAULT_PORT_SECURE)
                } else {
                    Ok(DEFAULT_PORT_INSECURE)
                }
            },
            Ok,
        ),
        None => Err(URIError::IncompleteURIError(
            "Authority for the URI is not found".into(),
        )),
    }
}

pub fn get_socket_address(uri: &Uri<String>) -> Result<String, URIError> {
    let host = uri.authority().map_or_else(
        || {
            Err(URIError::IncompleteURIError(
                "Authority for the URI is not found".into(),
            ))
        },
        |auth| Ok(get_host(&auth)),
    )?;
    let port = get_port(uri)?;

    Ok(format!("{host}:{port}"))
}

pub fn get_resource_target(uri: &Uri<String>) -> Result<String, URIError> {
    let mut path = uri.path().to_string();
    if path.is_empty() {
        path = String::from("/");
    }

    let query = uri.query().unwrap_or(EStr::EMPTY).to_string();

    Ok(path + &query)
}

#[macro_export]
macro_rules! safe_get_handshake_item {
    ($v:expr, $i:expr, $t:expr) => {
        $v.get($i).map_or_else(
            || {
                Err(WebSocketError::Handshake(
                    HandshakeFailureError::HeaderError(
                        "`$t` header not found in handshake response".into(),
                    ),
                ))
            },
            Ok,
        )
    };
}
