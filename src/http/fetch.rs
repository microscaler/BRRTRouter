//! Bounded HTTP fetches through `may_minihttp::client` for both HTTP and rustls-backed HTTPS.

use std::io::Read;
use std::time::Duration;

use http_legacy::{Method, Uri};
use may_minihttp::client::HttpClient;
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

/// Full HTTP GET metadata (status, optional `Location`, body).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpGetResponse {
    pub status: u16,
    pub location: Option<String>,
    pub body: Vec<u8>,
}

/// Perform a bounded HTTP GET and return `(status_code, body)`.
///
/// Supports `http://` and rustls-backed `https://` through `may_minihttp::client`.
///
/// # Errors
///
/// Returns [`HttpFetchError`] on URL parse failure, network/TLS errors, or oversize body.
pub fn fetch_get(url: &str, options: &HttpFetchOptions) -> Result<(u16, Vec<u8>), HttpFetchError> {
    fetch_get_full(url, options).map(|r| (r.status, r.body))
}

/// Perform a bounded HTTP GET and return status, optional redirect `Location`, and body.
///
/// Does not follow redirects — callers use this for OAuth authorize hops (302 + `Location`).
///
/// # Errors
///
/// Returns [`HttpFetchError`] on URL parse failure, network/TLS errors, or oversize body.
pub fn fetch_get_full(
    url: &str,
    options: &HttpFetchOptions,
) -> Result<HttpGetResponse, HttpFetchError> {
    let parsed = Url::parse(url).map_err(|e| HttpFetchError::InvalidUrl(e.to_string()))?;
    match parsed.scheme() {
        "http" | "https" => fetch_get_full_via_client(&parsed, options),
        other => Err(HttpFetchError::InvalidUrl(format!(
            "unsupported scheme: {other}"
        ))),
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
                tracing::debug!("HTTP fetch attempt {}: {} for {}", attempt + 1, e, url);
            }
        }
    }
    None
}

/// Perform a bounded HTTP POST and return `(status_code, body)`.
///
/// Supports `http://` and rustls-backed `https://` through `may_minihttp::client`.
///
/// # Errors
///
/// Returns [`HttpFetchError`] on URL parse failure, network/TLS errors, or oversize body.
pub fn fetch_post(
    url: &str,
    body: &[u8],
    options: &HttpFetchOptions,
) -> Result<(u16, Vec<u8>), HttpFetchError> {
    let parsed = Url::parse(url).map_err(|e| HttpFetchError::InvalidUrl(e.to_string()))?;
    match parsed.scheme() {
        "http" | "https" => fetch_post_via_client(&parsed, body, options),
        other => Err(HttpFetchError::InvalidUrl(format!(
            "unsupported scheme: {other}"
        ))),
    }
}

fn request_path_and_query(url: &Url) -> String {
    if url.query().is_some() {
        format!("{}?{}", url.path(), url.query().unwrap_or_default())
    } else {
        url.path().to_string()
    }
}

/// Path (+ query) URI for `may_minihttp::client` — must not include scheme/host (unlike HTTPS raw socket).
fn request_uri_for_may_minihttp(url: &Url) -> Result<Uri, HttpFetchError> {
    let mut path = request_path_and_query(url);
    if path.is_empty() {
        path = "/".to_string();
    }
    path.parse()
        .map_err(|e| HttpFetchError::InvalidUrl(format!("path uri: {e}")))
}

fn connect_client(url: &Url, options: &HttpFetchOptions) -> Result<HttpClient, HttpFetchError> {
    let mut client = HttpClient::from_url(url.as_str())
        .map_err(|error| HttpFetchError::Connect(error.to_string()))?;
    client.set_timeout(Some(options.timeout));
    Ok(client)
}

fn apply_extra_headers(request: &mut may_minihttp::client::Request, options: &HttpFetchOptions) {
    for (name, value) in &options.extra_headers {
        if let (Ok(header_name), Ok(header_value)) = (
            http_legacy::HeaderName::try_from(name.as_str()),
            http_legacy::HeaderValue::from_str(value),
        ) {
            request.headers_mut().insert(header_name, header_value);
        }
    }
}

fn fetch_post_via_client(
    url: &Url,
    body: &[u8],
    options: &HttpFetchOptions,
) -> Result<(u16, Vec<u8>), HttpFetchError> {
    let mut client = connect_client(url, options)?;
    let uri: Uri = request_uri_for_may_minihttp(url)?;
    let mut req = client.new_request(Method::POST, uri);
    apply_extra_headers(&mut req, options);
    req.send(body)
        .map_err(|e| HttpFetchError::Request(e.to_string()))?;
    let mut response = client
        .send_request(req)
        .map_err(|e| HttpFetchError::Response(e.to_string()))?;
    let status = response.status().as_u16();
    read_bounded_body(&mut response, options.max_body_bytes).map(|b| (status, b))
}

fn fetch_get_full_via_client(
    url: &Url,
    options: &HttpFetchOptions,
) -> Result<HttpGetResponse, HttpFetchError> {
    let mut client = connect_client(url, options)?;
    let uri: Uri = request_uri_for_may_minihttp(url)?;
    let mut req = client.new_request(Method::GET, uri);
    apply_extra_headers(&mut req, options);
    let mut response = client
        .send_request(req)
        .map_err(|e| HttpFetchError::Response(e.to_string()))?;
    let status = response.status().as_u16();
    let location = response
        .headers()
        .get(http_legacy::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = read_bounded_body(&mut response, options.max_body_bytes)?;
    Ok(HttpGetResponse {
        status,
        location,
        body,
    })
}

fn read_bounded_body(reader: &mut impl Read, max_body: usize) -> Result<Vec<u8>, HttpFetchError> {
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
    fn read_bounded_body_respects_limit() {
        let data = b"12345";
        let err = read_bounded_body(&mut &data[..], 3).unwrap_err();
        assert_eq!(err, HttpFetchError::BodyTooLarge);
    }

    #[test]
    fn request_uri_for_may_minihttp_uses_path_not_full_url() {
        let url = Url::parse(
            "http://auth-service.identity.svc.cluster.local:8080/auth/v1/.well-known/jwks.json",
        )
        .unwrap();
        let uri = request_uri_for_may_minihttp(&url).unwrap();
        assert_eq!(uri.to_string(), "/auth/v1/.well-known/jwks.json");
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
