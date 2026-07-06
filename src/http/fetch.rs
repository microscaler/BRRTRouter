//! Bounded GET fetch over HTTP/1.1 using `may_http` (plain) or rustls (HTTPS).

use std::io::{Read, Write};
use std::sync::Arc;
use std::time::Duration;

use http_legacy::{Method, Uri};
use may_http::client::HttpClient;
use rustls::pki_types::ServerName;
use url::Url;

/// Options for outbound GET requests from security providers.
#[derive(Debug, Clone)]
pub struct HttpFetchOptions {
    /// Total read/write timeout per request.
    pub timeout: Duration,
    /// Maximum response body bytes to read.
    pub max_body_bytes: usize,
    /// Extra request headers (name, value).
    pub extra_headers: Vec<(String, String)>,
}

impl Default for HttpFetchOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(500),
            max_body_bytes: 256 * 1024,
            extra_headers: Vec::new(),
        }
    }
}

/// Errors from the coroutine HTTP fetch layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpFetchError {
    InvalidUrl(String),
    Connect(String),
    Request(String),
    Response(String),
    Read(String),
    Tls(String),
    BodyTooLarge,
    InvalidStatus(u16),
}

impl std::fmt::Display for HttpFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl(msg) => write!(f, "invalid URL: {msg}"),
            Self::Connect(msg) => write!(f, "connect: {msg}"),
            Self::Request(msg) => write!(f, "request: {msg}"),
            Self::Response(msg) => write!(f, "response: {msg}"),
            Self::Read(msg) => write!(f, "read: {msg}"),
            Self::Tls(msg) => write!(f, "tls: {msg}"),
            Self::BodyTooLarge => write!(f, "response body exceeds limit"),
            Self::InvalidStatus(code) => write!(f, "HTTP status {code}"),
        }
    }
}

impl std::error::Error for HttpFetchError {}

/// Perform a bounded HTTP GET and return `(status_code, body)`.
///
/// Supports `http://` via `may_http` and `https://` via rustls on `may::net::TcpStream`.
///
/// # Errors
///
/// Returns [`HttpFetchError`] on URL parse failure, network/TLS errors, or oversize body.
pub fn fetch_get(url: &str, options: &HttpFetchOptions) -> Result<(u16, Vec<u8>), HttpFetchError> {
    let parsed = Url::parse(url).map_err(|e| HttpFetchError::InvalidUrl(e.to_string()))?;
    match parsed.scheme() {
        "http" => fetch_get_http(&parsed, options),
        "https" => fetch_get_https(&parsed, options),
        other => Err(HttpFetchError::InvalidUrl(format!("unsupported scheme: {other}"))),
    }
}

/// GET with retries; returns body text only on 2xx responses.
///
/// Used by JWKS refresh paths (two attempts, short timeout).
pub fn fetch_get_text_with_retry(
    url: &str,
    options: &HttpFetchOptions,
    attempts: u32,
) -> Option<String> {
    for attempt in 0..attempts {
        match fetch_get(url, options) {
            Ok((status, body)) if (200..300).contains(&status) => {
                return String::from_utf8(body).ok();
            }
            Ok((status, _)) => {
                tracing::debug!(
                    "HTTP fetch attempt {}: status {} for {}",
                    attempt + 1,
                    status,
                    url
                );
            }
            Err(e) => {
                tracing::debug!(
                    "HTTP fetch attempt {}: {} for {}",
                    attempt + 1,
                    e,
                    url
                );
            }
        }
    }
    None
}

fn fetch_get_http(url: &Url, options: &HttpFetchOptions) -> Result<(u16, Vec<u8>), HttpFetchError> {
    let host = url
        .host_str()
        .ok_or_else(|| HttpFetchError::InvalidUrl("missing host".to_string()))?;
    let port = url.port().unwrap_or(80);

    let mut client = HttpClient::connect((host, port))
        .map_err(|e| HttpFetchError::Connect(format!("{host}:{port}: {e}")))?;
    client.set_timeout(Some(options.timeout));

    let uri: Uri = url
        .as_str()
        .parse()
        .map_err(|e| HttpFetchError::InvalidUrl(format!("uri: {e}")))?;

    let mut req = client.new_request(Method::GET, uri);
    for (name, value) in &options.extra_headers {
        if let Ok(header_value) = http_legacy::HeaderValue::from_str(value) {
            req.headers_mut().insert(name.as_str(), header_value);
        }
    }

    let mut response = client
        .send_request(req)
        .map_err(|e| HttpFetchError::Response(e.to_string()))?;

    let status = response.status().as_u16();
    read_bounded_body(&mut response, options.max_body_bytes).map(|body| (status, body))
}

fn fetch_get_https(url: &Url, options: &HttpFetchOptions) -> Result<(u16, Vec<u8>), HttpFetchError> {
    use may::net::TcpStream;

    let host = url
        .host_str()
        .ok_or_else(|| HttpFetchError::InvalidUrl("missing host".to_string()))?;
    let port = url.port().unwrap_or(443);

    let mut tcp = TcpStream::connect((host, port))
        .map_err(|e| HttpFetchError::Connect(format!("{host}:{port}: {e}")))?;
    tcp.set_read_timeout(Some(options.timeout))
        .map_err(|e| HttpFetchError::Connect(e.to_string()))?;
    tcp.set_write_timeout(Some(options.timeout))
        .map_err(|e| HttpFetchError::Connect(e.to_string()))?;

    let config = rustls::ClientConfig::builder()
        .with_platform_verifier()
        .map_err(|e| HttpFetchError::Tls(e.to_string()))?
        .with_no_client_auth();

    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| HttpFetchError::Tls(format!("server name: {e}")))?;

    let mut tls =
        rustls::ClientConnection::new(Arc::new(config), server_name)
            .map_err(|e| HttpFetchError::Tls(e.to_string()))?;
    let mut tls_stream = rustls::Stream::new(&mut tls, &mut tcp);

    let path = if url.query().is_some() {
        format!(
            "{}?{}",
            url.path(),
            url.query().unwrap_or_default()
        )
    } else {
        url.path().to_string()
    };

    let host_header = if port == 443 {
        host.to_string()
    } else {
        format!("{host}:{port}")
    };

    let mut request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host_header}\r\nConnection: close\r\nUser-Agent: brrtrouter/0.1\r\n"
    );
    for (name, value) in &options.extra_headers {
        request.push_str(name);
        request.push_str(": ");
        request.push_str(value);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");

    tls_stream
        .write_all(request.as_bytes())
        .map_err(|e| HttpFetchError::Request(e.to_string()))?;

    let mut raw = Vec::new();
    tls_stream
        .take(options.max_body_bytes as u64 + 8192)
        .read_to_end(&mut raw)
        .map_err(|e| HttpFetchError::Read(e.to_string()))?;

    parse_http_response(&raw, options.max_body_bytes)
}

fn parse_http_response(raw: &[u8], max_body: usize) -> Result<(u16, Vec<u8>), HttpFetchError> {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut response = httparse::Response::new(&mut headers);
    let status = response
        .parse(raw)
        .map_err(|e| HttpFetchError::Response(format!("parse: {e:?}")))?;

    let header_len = match status {
        httparse::Status::Complete(len) => len,
        httparse::Status::Partial => {
            return Err(HttpFetchError::Response("incomplete headers".to_string()));
        }
    };

    let code = response
        .code
        .ok_or_else(|| HttpFetchError::Response("missing status code".to_string()))?;

    let body = raw[header_len..].to_vec();
    if body.len() > max_body {
        return Err(HttpFetchError::BodyTooLarge);
    }

    Ok((code, body))
}

fn read_bounded_body(
    reader: &mut impl Read,
    max_body: usize,
) -> Result<Vec<u8>, HttpFetchError> {
    let mut buf = Vec::new();
    reader
        .by_ref()
        .take(max_body as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|e| HttpFetchError::Read(e.to_string()))?;
    if buf.len() > max_body {
        return Err(HttpFetchError::BodyTooLarge);
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_http_response_extracts_status_and_body() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
        let (status, body) = parse_http_response(raw, 1024).unwrap();
        assert_eq!(status, 200);
        assert_eq!(body, b"hello");
    }

    #[test]
    fn parse_http_response_handles_404() {
        let raw = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
        let (status, body) = parse_http_response(raw, 1024).unwrap();
        assert_eq!(status, 404);
        assert!(body.is_empty());
    }

    #[test]
    fn parse_http_response_rejects_oversize_body() {
        let raw = b"HTTP/1.1 200 OK\r\n\r\nhello";
        assert_eq!(
            parse_http_response(raw, 2),
            Err(HttpFetchError::BodyTooLarge)
        );
    }

    #[test]
    fn parse_http_response_rejects_partial_headers() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Leng";
        assert!(parse_http_response(raw, 1024).is_err());
    }

    #[test]
    fn read_bounded_body_respects_limit() {
        let data = b"12345";
        let err = read_bounded_body(&mut &data[..], 3).unwrap_err();
        assert_eq!(err, HttpFetchError::BodyTooLarge);
    }

    #[test]
    fn fetch_get_rejects_unsupported_scheme() {
        let err = fetch_get("ftp://example.com/x", &HttpFetchOptions::default()).unwrap_err();
        assert!(matches!(err, HttpFetchError::InvalidUrl(_)));
    }

    #[test]
    fn fetch_get_rejects_malformed_url() {
        let err = fetch_get("not-a-url", &HttpFetchOptions::default()).unwrap_err();
        assert!(matches!(err, HttpFetchError::InvalidUrl(_)));
    }

    #[test]
    fn http_fetch_error_display_includes_context() {
        let err = HttpFetchError::Connect("refused".to_string());
        assert!(err.to_string().contains("refused"));
    }
}
