use uris::Uri;

use super::errors::URIError;

pub const CRLF: &str = "\r\n";

pub fn get_uri(uri_string: &str) -> Result<Uri, URIError> {
    Ok(Uri::parse(uri_string)?)
}

pub fn get_host(uri: &Uri) -> Result<String, URIError> {
    uri.host_to_string()?
        .ok_or_else(|| URIError::IncompleteURIError("Host Not Present".into()))
}

pub fn get_socket_address(uri: &Uri) -> Result<String, URIError> {
    let host = get_host(uri)?;
    let port = uri
        .port()
        .ok_or_else(|| URIError::IncompleteURIError("Port Not Present".into()))?;

    Ok(format!("{}:{}", host, port))
}

pub fn get_resource_target(uri: &Uri) -> Result<String, URIError> {
    let path = uri.path_to_string().unwrap_or(String::from("/"));
    let query = uri.query_to_string()?.unwrap_or(String::from(""));

    Ok(path + &query)
}
