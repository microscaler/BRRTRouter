//! Bounded HTTP fetches through `may_minihttp::client` for both HTTP and rustls-backed HTTPS.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use http_legacy::{Method, Uri};
use may_minihttp::client::HttpClient;
use rustls::{ClientConfig, RootCertStore};
use rustls_platform_verifier::BuilderVerifierExt;
use tracing::{error, warn};
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
/// Used by JWKS refresh paths.
///
/// # Diagnostics
///
/// Every failed attempt is logged at `WARN` and exhaustion of all attempts at `ERROR`, each with
/// the URL, the attempt number, the configured timeout, the elapsed time and the underlying error.
///
/// **Why this is deliberate.** This function used to log every failure at `debug!` and simply
/// return `None`. At default log levels a JWKS endpoint that had never once answered was
/// indistinguishable from a healthy one: the refresh silently never succeeded, every token
/// validation failed with a bare 401, and nothing in the logs named DNS, TLS, the edge or the
/// upstream. Operators had to eliminate each of those by hand. The caller's `Option` contract is
/// unchanged — the difference is that the failure is now *visible*.
pub fn fetch_get_text_with_retry(
    url: &str,
    options: &HttpFetchOptions,
    attempts: u32,
) -> Option<String> {
    let started = Instant::now();
    let timeout_ms = options.timeout.as_millis() as u64;
    let mut last_failure: Option<String> = None;

    for attempt in 0..attempts {
        let attempt_started = Instant::now();
        match fetch_get(url, options) {
            Ok((status, body)) if (200..300).contains(&status) => match String::from_utf8(body) {
                Ok(text) => return Some(text),
                Err(error) => {
                    // A 2xx carrying a non-UTF-8 body is a peer misconfiguration, not a transient
                    // fault, so it is not retried — but it must never be silent either.
                    error!(
                        url = %url,
                        status,
                        attempt = attempt + 1,
                        attempts,
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        error = %error,
                        "HTTP fetch failed: 2xx response body is not valid UTF-8"
                    );
                    return None;
                }
            },
            Ok((status, _)) => {
                last_failure = Some(format!("HTTP status {status}"));
                warn!(
                    url = %url,
                    status,
                    attempt = attempt + 1,
                    attempts,
                    timeout_ms,
                    attempt_ms = attempt_started.elapsed().as_millis() as u64,
                    "HTTP fetch attempt failed: unexpected status"
                );
            }
            Err(error) => {
                last_failure = Some(error.to_string());
                warn!(
                    url = %url,
                    attempt = attempt + 1,
                    attempts,
                    timeout_ms,
                    attempt_ms = attempt_started.elapsed().as_millis() as u64,
                    error = %error,
                    "HTTP fetch attempt failed"
                );
            }
        }
    }

    error!(
        url = %url,
        attempts,
        timeout_ms,
        elapsed_ms = started.elapsed().as_millis() as u64,
        error = last_failure.as_deref().unwrap_or("no attempts were made"),
        "HTTP fetch failed after all attempts; no body was retrieved"
    );
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
        "http" | "https" => fetch_method_via_client(&parsed, Method::POST, Some(body), options),
        other => Err(HttpFetchError::InvalidUrl(format!(
            "unsupported scheme: {other}"
        ))),
    }
}

/// Perform a bounded HTTP QUERY (RFC 10008) and return `(status_code, body)`.
///
/// Bridges to `http` 0.2 [`Method::from_bytes`]`("QUERY")` for may_minihttp
/// (Story 11.3). QUERY is safe/idempotent — see [`crate::http::method_allows_automatic_retry`].
///
/// # Errors
///
/// Returns [`HttpFetchError`] on URL parse failure, network/TLS errors, or oversize body.
pub fn fetch_query(
    url: &str,
    body: &[u8],
    options: &HttpFetchOptions,
) -> Result<(u16, Vec<u8>), HttpFetchError> {
    let parsed = Url::parse(url).map_err(|e| HttpFetchError::InvalidUrl(e.to_string()))?;
    let method = Method::from_bytes(b"QUERY").map_err(|e| {
        HttpFetchError::InvalidUrl(format!("QUERY method unsupported by http client: {e}"))
    })?;
    match parsed.scheme() {
        "http" | "https" => fetch_method_via_client(&parsed, method, Some(body), options),
        other => Err(HttpFetchError::InvalidUrl(format!(
            "unsupported scheme: {other}"
        ))),
    }
}

/// Perform a bounded HTTP PATCH and return `(status_code, body)`.
///
/// # Errors
///
/// Returns [`HttpFetchError`] on URL parse failure, network/TLS errors, or oversize body.
pub fn fetch_patch(
    url: &str,
    body: &[u8],
    options: &HttpFetchOptions,
) -> Result<(u16, Vec<u8>), HttpFetchError> {
    let parsed = Url::parse(url).map_err(|e| HttpFetchError::InvalidUrl(e.to_string()))?;
    match parsed.scheme() {
        "http" | "https" => fetch_method_via_client(&parsed, Method::PATCH, Some(body), options),
        other => Err(HttpFetchError::InvalidUrl(format!(
            "unsupported scheme: {other}"
        ))),
    }
}

/// Perform a bounded HTTP DELETE and return `(status_code, body)`.
///
/// # Errors
///
/// Returns [`HttpFetchError`] on URL parse failure, network/TLS errors, or oversize body.
pub fn fetch_delete(
    url: &str,
    body: Option<&[u8]>,
    options: &HttpFetchOptions,
) -> Result<(u16, Vec<u8>), HttpFetchError> {
    let parsed = Url::parse(url).map_err(|e| HttpFetchError::InvalidUrl(e.to_string()))?;
    match parsed.scheme() {
        "http" | "https" => fetch_method_via_client(&parsed, Method::DELETE, body, options),
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

/// Process-wide rustls client configuration for outbound HTTPS, built exactly once.
///
/// # Why this is cached
///
/// `may_minihttp::client::HttpClient::from_url` calls its internal `platform_tls_config()` on
/// **every** HTTPS connect, and that call builds a fresh `rustls::ClientConfig` backed by the
/// platform verifier — which reads and parses the whole system CA bundle off disk each time.
/// On the JWKS refresh path that cost was paid per fetch *and* per retry, out of the same budget
/// as the request itself. It is a large part of why the old hard-coded 200ms JWKS timeout was
/// simply unachievable once the URL moved from plaintext in-cluster HTTP to HTTPS on a real
/// hostname behind a TLS-terminating edge: the deadline was partly consumed before a single byte
/// left the process.
///
/// The trust store does not change between fetches, so we build it once and share the `Arc` for
/// the life of the process. `Err` is cached too — a machine with an unreadable trust store will
/// not become readable by re-parsing it on every request, and re-trying would reintroduce exactly
/// the per-fetch cost this exists to remove.
static PLATFORM_TLS_CONFIG: OnceLock<Result<Arc<ClientConfig>, String>> = OnceLock::new();

/// How many times the shared TLS config was actually constructed.
///
/// The entire point of the cache is that this stays at 1 no matter how many HTTPS fetches run,
/// so it is the thing the test asserts on.
static TLS_CONFIG_BUILDS: AtomicUsize = AtomicUsize::new(0);

/// Return the shared rustls client configuration, building it on first use.
fn platform_tls_config() -> Result<Arc<ClientConfig>, HttpFetchError> {
    PLATFORM_TLS_CONFIG
        .get_or_init(|| {
            TLS_CONFIG_BUILDS.fetch_add(1, Ordering::Relaxed);
            build_platform_tls_config()
        })
        .clone()
        .map_err(HttpFetchError::Tls)
}

/// Mirrors `may_minihttp`'s platform TLS setup unless `SSL_CERT_FILE` names an
/// explicit PEM bundle. Containers use that standard variable to extend their
/// system roots with deployment-local CAs.
fn build_platform_tls_config() -> Result<Arc<ClientConfig>, String> {
    if let Some(path) = std::env::var_os("SSL_CERT_FILE") {
        let file = File::open(&path)
            .map_err(|error| format!("cannot open SSL_CERT_FILE {}: {error}", path.display()))?;
        return build_tls_config_from_pem(BufReader::new(file));
    }
    if let Some(path) = std::env::var_os("EXTRA_CA_CERT_FILE") {
        let file = File::open(&path).map_err(|error| {
            format!("cannot open EXTRA_CA_CERT_FILE {}: {error}", path.display())
        })?;
        let mut roots = RootCertStore::empty();
        let native = rustls_native_certs::load_native_certs();
        roots.add_parsable_certificates(native.certs);
        add_pem_roots(&mut roots, BufReader::new(file), "EXTRA_CA_CERT_FILE")?;
        return build_tls_config_from_roots(roots);
    }

    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|error| format!("TLS protocol setup failed: {error}"))?;
    let config = builder
        .with_platform_verifier()
        .map_err(|error| format!("platform verifier failed: {error}"))?
        .with_no_client_auth();
    Ok(Arc::new(config))
}

fn build_tls_config_from_pem(mut reader: impl BufRead) -> Result<Arc<ClientConfig>, String> {
    let mut roots = RootCertStore::empty();
    add_pem_roots(&mut roots, &mut reader, "SSL_CERT_FILE")?;
    build_tls_config_from_roots(roots)
}

fn add_pem_roots(
    roots: &mut RootCertStore,
    mut reader: impl BufRead,
    source: &str,
) -> Result<(), String> {
    let mut loaded = 0usize;
    for certificate in rustls_pemfile::certs(&mut reader) {
        let certificate =
            certificate.map_err(|error| format!("invalid certificate in {source}: {error}"))?;
        roots
            .add(certificate)
            .map_err(|error| format!("invalid certificate in {source}: {error}"))?;
        loaded += 1;
    }
    if loaded == 0 {
        return Err(format!("{source} contains no certificates"));
    }
    Ok(())
}

fn build_tls_config_from_roots(roots: RootCertStore) -> Result<Arc<ClientConfig>, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|error| format!("TLS protocol setup failed: {error}"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

fn connect_client(url: &Url, options: &HttpFetchOptions) -> Result<HttpClient, HttpFetchError> {
    // Plaintext HTTP needs no TLS material at all, so the trust store is never touched for
    // in-cluster `http://...svc.cluster.local` JWKS URLs; HTTPS reuses the cached config above
    // instead of letting `from_url` rebuild one per connect.
    let mut client = if url.scheme().eq_ignore_ascii_case("https") {
        HttpClient::from_url_with_tls_config(url.as_str(), platform_tls_config()?)
            .map_err(|error| HttpFetchError::Connect(error.to_string()))?
    } else {
        HttpClient::from_url(url.as_str())
            .map_err(|error| HttpFetchError::Connect(error.to_string()))?
    };
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

fn fetch_method_via_client(
    url: &Url,
    method: Method,
    body: Option<&[u8]>,
    options: &HttpFetchOptions,
) -> Result<(u16, Vec<u8>), HttpFetchError> {
    let mut client = connect_client(url, options)?;
    let uri: Uri = request_uri_for_may_minihttp(url)?;
    let mut req = client.new_request(method, uri);
    apply_extra_headers(&mut req, options);
    if let Some(bytes) = body {
        req.send(bytes)
            .map_err(|e| HttpFetchError::Request(e.to_string()))?;
    }
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
    fn fetch_query_positive_p2_method_from_bytes() {
        let m = Method::from_bytes(b"QUERY").expect("QUERY token for may_minihttp");
        assert_eq!(m.as_str(), "QUERY");
    }

    #[test]
    fn fetch_query_rejects_unsupported_scheme() {
        let err =
            fetch_query("ftp://example.com/q", b"{}", &HttpFetchOptions::default()).unwrap_err();
        assert!(matches!(err, HttpFetchError::InvalidUrl(_)));
    }

    #[test]
    fn http_fetch_error_display_includes_context() {
        let err = HttpFetchError::Connect("refused".to_string());
        assert!(err.to_string().contains("refused"));
    }

    /// The system CA bundle used to be re-parsed from disk on every HTTPS connect. It must now be
    /// built once and shared: repeated calls hand back the *same* `Arc` and never re-run the
    /// builder, no matter how many fetches happen.
    #[test]
    fn platform_tls_config_is_built_once_and_shared() {
        let first = platform_tls_config();
        let builds_after_first = TLS_CONFIG_BUILDS.load(Ordering::Relaxed);

        // Stand in for "many fetches": each of these is what one HTTPS connect would ask for.
        for _ in 0..8 {
            let next = platform_tls_config();
            match (&first, &next) {
                (Ok(a), Ok(b)) => assert!(
                    Arc::ptr_eq(a, b),
                    "each fetch must reuse the same TLS config allocation"
                ),
                (Err(a), Err(b)) => assert_eq!(a, b, "cached failure must be stable"),
                _ => panic!("cached TLS config result changed between calls"),
            }
        }

        assert_eq!(
            TLS_CONFIG_BUILDS.load(Ordering::Relaxed),
            builds_after_first,
            "TLS config was rebuilt after the first call"
        );
        assert_eq!(
            builds_after_first, 1,
            "TLS config must be constructed exactly once per process"
        );
    }

    #[test]
    fn explicit_pem_bundle_builds_tls_config() {
        let rcgen::CertifiedKey { cert, .. } =
            rcgen::generate_simple_self_signed(vec!["provider.test".to_string()])
                .expect("certificate");
        let bundle = cert.pem();
        assert!(build_tls_config_from_pem(std::io::Cursor::new(bundle)).is_ok());
    }

    #[test]
    fn explicit_empty_pem_bundle_fails_closed() {
        let error = build_tls_config_from_pem(std::io::Cursor::new(Vec::<u8>::new()))
            .expect_err("empty bundle must fail");
        assert!(error.contains("contains no certificates"));
    }

    /// `connect_client` must not touch the trust store for plaintext URLs at all — an in-cluster
    /// `http://` JWKS URL should never pay for certificate parsing.
    #[test]
    fn plaintext_scheme_does_not_require_tls_config() {
        let url = Url::parse("http://127.0.0.1:1/jwks.json").unwrap();
        let err = connect_client(&url, &HttpFetchOptions::default()).unwrap_err();
        assert!(
            matches!(err, HttpFetchError::Connect(_)),
            "expected a connect failure on a dead port, got {err:?}"
        );
    }
}
