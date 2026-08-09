//! In-process HTTP test client for BRRTRouter apps (Epic 13.10).
//!
//! Enable with Cargo feature `testing`. Intended for product-crate integration
//! tests (Sesame, pet_store, …) so they do not copy private `tests/common`
//! TCP helpers.
//!
//! # Example
//!
//! ```rust,ignore
//! use brrtrouter::test_support::TestApp;
//! use brrtrouter::dispatcher::Dispatcher;
//!
//! let app = TestApp::from_spec("examples/pet_store/doc/openapi.yaml", |dispatcher, routes| {
//!     // SAFETY: test registration of typed handlers
//!     unsafe { pet_store::registry::register_from_spec(dispatcher, routes) }
//! })?;
//!
//! let res = app
//!     .get("/pets")
//!     .header("X-API-Key", "test123")
//!     .send()?;
//! assert_eq!(res.status, 200);
//! ```

use crate::dispatcher::Dispatcher;
use crate::router::Router;
use crate::server::{AppService, HttpServer, ServerHandle};
use crate::spec::RouteMeta;
use arc_swap::ArcSwap;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

static MAY_INIT: Once = Once::new();

fn ensure_may_runtime() {
    MAY_INIT.call_once(|| {
        may::config().set_stack_size(0x8000);
    });
}

/// Errors from constructing a [`TestApp`] or sending a request.
#[derive(Debug)]
pub enum TestAppError {
    /// OpenAPI load failed.
    Spec(anyhow::Error),
    /// Local bind failed.
    Bind(std::io::Error),
    /// `HttpServer::start` failed.
    Start(std::io::Error),
    /// Listener never accepted connections.
    ServerNotReady,
    /// Builder path was empty.
    EmptyPath,
    /// TCP connect to the test server failed.
    Connect(std::io::Error),
    /// Read/write against the test server failed.
    Io(std::io::Error),
    /// JSON request body serialization failed.
    Serialize(serde_json::Error),
    /// JSON response body deserialization failed.
    Deserialize(serde_json::Error),
    /// Response body was not UTF-8.
    Utf8(std::str::Utf8Error),
}

impl fmt::Display for TestAppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spec(e) => write!(f, "failed to load OpenAPI spec: {e}"),
            Self::Bind(e) => write!(f, "failed to bind local listener: {e}"),
            Self::Start(e) => write!(f, "failed to start HTTP server: {e}"),
            Self::ServerNotReady => write!(f, "HTTP server did not become ready in time"),
            Self::EmptyPath => write!(f, "request path must be non-empty"),
            Self::Connect(e) => write!(f, "failed to connect to test server: {e}"),
            Self::Io(e) => write!(f, "I/O error talking to test server: {e}"),
            Self::Serialize(e) => write!(f, "failed to serialize JSON body: {e}"),
            Self::Deserialize(e) => write!(f, "failed to parse JSON response: {e}"),
            Self::Utf8(e) => write!(f, "response body is not valid UTF-8: {e}"),
        }
    }
}

impl StdError for TestAppError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Spec(e) => Some(e.as_ref()),
            Self::Bind(e) | Self::Start(e) | Self::Connect(e) | Self::Io(e) => Some(e),
            Self::Serialize(e) | Self::Deserialize(e) => Some(e),
            Self::Utf8(e) => Some(e),
            Self::ServerNotReady | Self::EmptyPath => None,
        }
    }
}

/// Options for [`TestApp::from_spec_with_options`].
#[derive(Debug, Clone, Default)]
pub struct TestAppOptions {
    /// Static site directory passed to [`AppService::new`].
    pub static_dir: Option<PathBuf>,
    /// Doc directory passed to [`AppService::new`].
    pub doc_dir: Option<PathBuf>,
}

/// Running in-process BRRTRouter HTTP server for tests.
pub struct TestApp {
    addr: SocketAddr,
    handle: Option<ServerHandle>,
}

impl TestApp {
    /// Start a server from an already-built [`AppService`].
    ///
    /// Binds `127.0.0.1:0`, starts [`HttpServer`], and waits until the port accepts
    /// connections. Returns [`TestAppError::ServerNotReady`] instead of succeeding
    /// silently when the listener never comes up.
    pub fn from_service(service: AppService) -> Result<Self, TestAppError> {
        ensure_may_runtime();
        let listener = TcpListener::bind("127.0.0.1:0").map_err(TestAppError::Bind)?;
        let addr = listener.local_addr().map_err(TestAppError::Bind)?;
        drop(listener);
        let handle = HttpServer(service)
            .start(addr)
            .map_err(TestAppError::Start)?;
        wait_ready(addr)?;
        Ok(Self {
            addr,
            handle: Some(handle),
        })
    }

    /// Load `spec_path`, register handlers via `register`, then start a server.
    ///
    /// `register` typically calls a generated `register_from_spec` (e.g. pet_store).
    pub fn from_spec<F>(spec_path: impl AsRef<Path>, register: F) -> Result<Self, TestAppError>
    where
        F: FnOnce(&mut Dispatcher, &[RouteMeta]),
    {
        Self::from_spec_with_options(spec_path, TestAppOptions::default(), register)
    }

    /// Like [`Self::from_spec`] with static/doc directory overrides.
    pub fn from_spec_with_options<F>(
        spec_path: impl AsRef<Path>,
        options: TestAppOptions,
        register: F,
    ) -> Result<Self, TestAppError>
    where
        F: FnOnce(&mut Dispatcher, &[RouteMeta]),
    {
        ensure_may_runtime();
        let spec_path = spec_path.as_ref();
        let (routes, schemes, _slug) =
            crate::load_spec_full(spec_path.to_str().unwrap_or_default())
                .map_err(TestAppError::Spec)?;
        let router = Arc::new(ArcSwap::from_pointee(Router::new(routes.clone())));
        let mut dispatcher = Dispatcher::new();
        register(&mut dispatcher, &routes);
        let service = AppService::new(
            router,
            Arc::new(ArcSwap::from_pointee(dispatcher)),
            schemes,
            spec_path.to_path_buf(),
            options.static_dir,
            options.doc_dir,
        );
        Self::from_service(service)
    }

    /// Local address of the test server.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Begin a custom request (`method` is uppercased on the wire).
    #[must_use]
    pub fn request(
        &self,
        method: impl Into<String>,
        path: impl Into<String>,
    ) -> RequestBuilder<'_> {
        RequestBuilder {
            app: self,
            method: method.into(),
            path: path.into(),
            headers: Vec::new(),
            cookies: Vec::new(),
            body: None,
            content_type: None,
        }
    }

    /// `GET` helper.
    #[must_use]
    pub fn get(&self, path: impl Into<String>) -> RequestBuilder<'_> {
        self.request("GET", path)
    }

    /// `POST` helper.
    #[must_use]
    pub fn post(&self, path: impl Into<String>) -> RequestBuilder<'_> {
        self.request("POST", path)
    }

    /// `PUT` helper.
    #[must_use]
    pub fn put(&self, path: impl Into<String>) -> RequestBuilder<'_> {
        self.request("PUT", path)
    }

    /// `DELETE` helper.
    #[must_use]
    pub fn delete(&self, path: impl Into<String>) -> RequestBuilder<'_> {
        self.request("DELETE", path)
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
    }
}

/// Fluent builder for a single HTTP request against a [`TestApp`].
pub struct RequestBuilder<'a> {
    app: &'a TestApp,
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    cookies: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    content_type: Option<String>,
}

impl<'a> RequestBuilder<'a> {
    /// Add a request header (name is sent as provided; matching is case-insensitive on the server).
    #[must_use]
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Add a cookie (`Cookie: name=value`, merged if multiple).
    #[must_use]
    pub fn cookie(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.cookies.push((name.into(), value.into()));
        self
    }

    /// Set a raw body and optional `Content-Type`.
    #[must_use]
    pub fn body(mut self, bytes: impl Into<Vec<u8>>, content_type: impl Into<String>) -> Self {
        self.body = Some(bytes.into());
        self.content_type = Some(content_type.into());
        self
    }

    /// Serialize `value` as JSON (`application/json`).
    pub fn json<T: Serialize>(mut self, value: &T) -> Result<Self, TestAppError> {
        let bytes = serde_json::to_vec(value).map_err(TestAppError::Serialize)?;
        self.body = Some(bytes);
        self.content_type = Some("application/json".into());
        Ok(self)
    }

    /// Send the request and parse status / headers / body.
    pub fn send(self) -> Result<TestResponse, TestAppError> {
        if self.path.is_empty() {
            return Err(TestAppError::EmptyPath);
        }
        let path = if self.path.starts_with('/') {
            self.path.clone()
        } else {
            format!("/{}", self.path)
        };

        let mut headers = self.headers;
        if let Some(ct) = &self.content_type {
            if !headers
                .iter()
                .any(|(n, _)| n.eq_ignore_ascii_case("content-type"))
            {
                headers.push(("Content-Type".into(), ct.clone()));
            }
        }
        if let Some(body) = &self.body {
            if !headers
                .iter()
                .any(|(n, _)| n.eq_ignore_ascii_case("content-length"))
            {
                headers.push(("Content-Length".into(), body.len().to_string()));
            }
        }
        if !self.cookies.is_empty() {
            let cookie = self
                .cookies
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("; ");
            headers.push(("Cookie".into(), cookie));
        }
        if !headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("host")) {
            headers.push(("Host".into(), "localhost".into()));
        }

        let mut req = format!("{} {} HTTP/1.1\r\n", self.method.to_ascii_uppercase(), path);
        for (name, value) in &headers {
            req.push_str(name);
            req.push_str(": ");
            req.push_str(value);
            req.push_str("\r\n");
        }
        req.push_str("\r\n");
        let mut bytes = req.into_bytes();
        if let Some(body) = self.body {
            bytes.extend_from_slice(&body);
        }

        // Under parallel test load, may_minihttp can briefly accept then return
        // an empty read (status 0). Retry like security_tests JWKS helpers.
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut last = TestResponse {
            status: 0,
            headers: HashMap::new(),
            body: Vec::new(),
        };
        while Instant::now() < deadline {
            let raw = exchange(&self.app.addr, &bytes)?;
            last = TestResponse::parse(&raw)?;
            if last.status != 0 {
                return Ok(last);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(last)
    }
}

impl fmt::Debug for RequestBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // N5 — never dump Authorization values in Debug.
        let headers: Vec<_> = self
            .headers
            .iter()
            .map(|(n, v)| {
                if n.eq_ignore_ascii_case("authorization") {
                    (n.as_str(), "<redacted>")
                } else {
                    (n.as_str(), v.as_str())
                }
            })
            .collect();
        f.debug_struct("RequestBuilder")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("headers", &headers)
            .field("cookies", &self.cookies.len())
            .field("body_len", &self.body.as_ref().map(|b| b.len()))
            .finish()
    }
}

/// Parsed HTTP response from [`RequestBuilder::send`].
#[derive(Debug, Clone)]
pub struct TestResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl TestResponse {
    fn parse(raw: &[u8]) -> Result<Self, TestAppError> {
        let header_end = raw
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|p| p + 4)
            .unwrap_or(raw.len());
        let head = std::str::from_utf8(&raw[..header_end]).map_err(TestAppError::Utf8)?;
        let mut lines = head.lines();
        let status = lines
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let mut headers = HashMap::new();
        for line in lines {
            if let Some((n, v)) = line.split_once(':') {
                headers.insert(n.trim().to_ascii_lowercase(), v.trim().to_string());
            }
        }
        let body = raw[header_end..].to_vec();
        Ok(Self {
            status,
            headers,
            body,
        })
    }

    /// Response body as UTF-8 text.
    pub fn text(&self) -> Result<&str, TestAppError> {
        std::str::from_utf8(&self.body).map_err(TestAppError::Utf8)
    }

    /// Deserialize JSON body.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, TestAppError> {
        serde_json::from_slice(&self.body).map_err(TestAppError::Deserialize)
    }

    /// Deserialize JSON as [`serde_json::Value`].
    pub fn json_value(&self) -> Result<Value, TestAppError> {
        self.json()
    }

    /// Header lookup (case-insensitive name).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(|s| s.as_str())
    }
}

fn wait_ready(addr: SocketAddr) -> Result<(), TestAppError> {
    // Longer than ServerHandle::wait_ready (~250ms) — parallel nextest needs headroom.
    for _ in 0..400 {
        if TcpStream::connect(addr).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Err(TestAppError::ServerNotReady)
}

fn read_timeout_ms() -> u64 {
    if std::env::var("CI").is_ok()
        || std::env::var("ACT").is_ok()
        || std::env::var("NEXTEST_RUN_ID").is_ok()
    {
        5_000
    } else {
        2_000
    }
}

fn exchange(addr: &SocketAddr, req: &[u8]) -> Result<Vec<u8>, TestAppError> {
    let mut stream = TcpStream::connect(addr).map_err(TestAppError::Connect)?;
    stream.write_all(req).map_err(TestAppError::Io)?;
    stream
        .set_read_timeout(Some(Duration::from_millis(read_timeout_ms())))
        .map_err(TestAppError::Io)?;

    let deadline = Instant::now() + Duration::from_millis(read_timeout_ms());
    let mut buf = Vec::new();
    let mut header_end = None;
    while header_end.is_none() && Instant::now() < deadline {
        let mut tmp = [0u8; 1024];
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = Some(pos + 4);
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                std::thread::sleep(Duration::from_millis(25));
                continue;
            }
            Err(e) => return Err(TestAppError::Io(e)),
        }
    }
    let header_end = header_end.unwrap_or(buf.len());
    let headers = String::from_utf8_lossy(&buf[..header_end]);
    let content_length = headers.lines().find_map(|l| {
        let (n, v) = l.split_once(':')?;
        if n.trim().eq_ignore_ascii_case("content-length") {
            v.trim().parse::<usize>().ok()
        } else {
            None
        }
    });

    if let Some(clen) = content_length {
        let mut body_len = buf.len().saturating_sub(header_end);
        while body_len < clen && Instant::now() < deadline {
            let mut tmp = [0u8; 4096];
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    body_len += n;
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    std::thread::sleep(Duration::from_millis(25));
                    continue;
                }
                Err(e) => return Err(TestAppError::Io(e)),
            }
        }
    } else {
        // No Content-Length: drain until EOF or deadline (do not treat first
        // WouldBlock as end-of-response — parallel lib tests often stall briefly).
        while Instant::now() < deadline {
            let mut tmp = [0u8; 4096];
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    if !buf.is_empty() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => return Err(TestAppError::Io(e)),
            }
        }
    }
    Ok(buf)
}
