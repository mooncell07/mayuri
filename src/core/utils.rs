use uris::Uri;

use super::errors::URIError;

pub const CRLF: &str = "\r\n";
pub const DEFAULT_PORT_SECURE: u16 = 443;
pub const DEFAULT_PORT_INSECURE: u16 = 80;

pub fn get_uri(uri_string: &str) -> Result<Uri, URIError> {
    Ok(Uri::parse(uri_string)?)
}

pub fn get_host(uri: &Uri) -> Result<String, URIError> {
    uri.host_to_string()?
        .ok_or_else(|| URIError::IncompleteURIError("Host Not Present".into()))
}

pub fn is_secured(uri: &Uri) -> Result<bool, URIError> {
    match uri.scheme() {
        Some(s) => {
            if s == "wss" {
                Ok(true)
            } else {
                Ok(false)
            }
        }
        None => Err(URIError::IncompleteURIError(
            "Please pass valid scheme `ws` or `wss`".into(),
        )),
    }
}

pub fn get_port(uri: &Uri) -> Result<u16, URIError> {
    if let Some(port) = uri.port() {
        Ok(port)
    } else {
        if is_secured(uri)? {
            Ok(DEFAULT_PORT_SECURE)
        } else {
            Ok(DEFAULT_PORT_INSECURE)
        }
    }
}

pub fn get_socket_address(uri: &Uri) -> Result<String, URIError> {
    let host = get_host(uri)?;
    let port = get_port(uri)?;

    Ok(format!("{}:{}", host, port))
}

pub fn get_resource_target(uri: &Uri) -> Result<String, URIError> {
    let path = uri.path_to_string().unwrap_or(String::from("/"));
    let query = uri.query_to_string()?.unwrap_or(String::from(""));

    Ok(path + &query)
}
