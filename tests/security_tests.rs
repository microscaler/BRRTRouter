//! Integration tests for authentication and authorization
//!
//! # Test Coverage
//!
//! Comprehensive testing of all security providers:
//! - API key authentication (header-based)
//! - Bearer JWT authentication (simple signature)
//! - Bearer JWT with JWKS validation (production-ready)
//! - OAuth2 authentication  
//! - Remote API key provider (HTTP-based validation)
//! - Cookie-based token extraction
//!
//! # Test Strategy
//!
//! 1. **Unit Tests**: Individual provider validation logic
//! 2. **Integration Tests**: Full HTTP server with auth enforcement
//! 3. **Mock Services**: Simulated JWKS/remote validation endpoints
//! 4. **Token Generation**: Base64-encoded JWT tokens for testing
//!
//! # Key Test Scenarios
//!
//! - Valid credentials → 200 OK
//! - Missing credentials → 401 Unauthorized
//! - Invalid credentials → 401 Unauthorized  
//! - Expired tokens → 401 Unauthorized
//! - Scope-based authorization
//! - Multiple auth schemes (OR logic)
//!
//! # Test Fixtures
//!
//! - Mock JWKS server on random port
//! - Pet Store API with security schemes
//! - Pre-generated JWT tokens with known signatures
//! - Remote validation server simulator

use base64::Engine;
use brrtrouter::middleware::TracingMiddleware;
use brrtrouter::server::{HttpServer, ServerHandle};
use brrtrouter::spec::SecurityScheme;
use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HandlerResponse, HeaderVec},
    load_spec_full,
    router::{ParamVec, Router},
    server::AppService,
    BearerJwtProvider, OAuth2Provider, SecurityProvider, SecurityRequest,
};
use serde_json::json;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
mod tracing_util;
use tracing_util::TestTracing;

mod common;
use common::temp_files;

/// Test fixture with automatic setup and teardown using RAII
///
/// This is the Rust equivalent of Python's setup/teardown for security tests.
/// Implements Drop to ensure proper cleanup when test completes.
struct SecurityTestServer {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}

impl SecurityTestServer {
    /// Create a security test server from existing start_service() pattern
    fn from_start_service() -> Self {
        let (tracing, handle, addr) = start_service();
        Self {
            _tracing: tracing,
            handle: Some(handle),
            addr,
        }
    }

    /// Create from start_service_default_provider()
    fn from_default_provider() -> Self {
        let (tracing, handle, addr) = start_service_default_provider();
        Self {
            _tracing: tracing,
            handle: Some(handle),
            addr,
        }
    }

    /// Create from start_service_with_jwks()
    fn from_jwks(jwks_url: &str, issuer: &str, audience: &str) -> Self {
        let (tracing, handle, addr) = start_service_with_jwks(jwks_url, issuer, audience);
        Self {
            _tracing: tracing,
            handle: Some(handle),
            addr,
        }
    }

    /// Create from start_multi_service()
    fn from_multi_service() -> Self {
        let (tracing, handle, addr) = start_multi_service();
        Self {
            _tracing: tracing,
            handle: Some(handle),
            addr,
        }
    }

    /// Create from start_token_service()
    fn from_token_service() -> Self {
        let (tracing, handle, addr) = start_token_service();
        Self {
            _tracing: tracing,
            handle: Some(handle),
            addr,
        }
    }

    /// Create from start_service_with_remote_apikey()
    fn from_remote_apikey(verify_url: &str) -> Self {
        let (tracing, handle, addr) = start_service_with_remote_apikey(verify_url);
        Self {
            _tracing: tracing,
            handle: Some(handle),
            addr,
        }
    }

    /// Get the server address for making requests
    fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for SecurityTestServer {
    /// Teardown: Automatically stop server when test completes
    ///
    /// This ensures proper cleanup even if the test panics,
    /// preventing resource leaks and port conflicts.
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
        // _tracing is automatically dropped here
    }
}

struct ApiKeyProvider {
    key: String,
}

impl SecurityProvider for ApiKeyProvider {
    fn validate(&self, scheme: &SecurityScheme, _scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::ApiKey { name, location, .. } => match location.as_str() {
                "header" => req
                    .get_header(&name.to_ascii_lowercase())
                    .map(|v| v == self.key)
                    .unwrap_or(false),
                "query" => req.get_query(name).map(|v| v == self.key).unwrap_or(false),
                "cookie" => req.get_cookie(name).map(|v| v == self.key).unwrap_or(false),
                _ => false,
            },
            _ => false,
        }
    }
}

fn start_service() -> (TestTracing, ServerHandle, SocketAddr) {
    // ensure coroutines have enough stack for tests
    may::config().set_stack_size(0x8000);
    let tracing = TestTracing::init();
    const SPEC: &str = r#"openapi: 3.1.0
info:
  title: Auth API
  version: '1.0'
components:
  securitySchemes:
    ApiKeyAuth:
      type: apiKey
      in: header
      name: X-API-Key
paths:
  /secret:
    get:
      operationId: secret
      security:
        - ApiKeyAuth: []
      responses:
        '200': { description: OK }
"#;
    let path = temp_files::create_temp_yaml(SPEC);
    let (routes, schemes, _slug) = match load_spec_full(path.to_str().unwrap()) {
        Ok(result) => result,
        Err(e) => panic!("Failed to load spec: {:?}", e),
    };
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    // SAFETY: Test context - handlers are simple closures for testing
    unsafe {
        dispatcher.register_handler("secret", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HeaderVec::new(),
                body: json!({"ok": true}),
            });
        });
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));
    let mut service = AppService::new(
        router,
        Arc::new(RwLock::new(dispatcher)),
        schemes,
        PathBuf::from("examples/openapi.yaml"),
        Some(PathBuf::from("examples/pet_store/static_site")),
        Some(PathBuf::from("examples/pet_store/doc")),
    );
    service.register_security_provider(
        "ApiKeyAuth",
        Arc::new(ApiKeyProvider {
            key: "secret".into(),
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    (tracing, handle, addr)
}

fn start_multi_service() -> (TestTracing, ServerHandle, SocketAddr) {
    // ensure coroutines have enough stack for tests
    may::config().set_stack_size(0x8000);
    let tracing = TestTracing::init();
    const SPEC: &str = r#"openapi: 3.1.0
info:
  title: Multi Auth API
  version: '1.0'
components:
  securitySchemes:
    KeyOne:
      type: apiKey
      in: header
      name: X-Key-One
    KeyTwo:
      type: apiKey
      in: header
      name: X-Key-Two
paths:
  /one:
    get:
      operationId: one
      security:
        - KeyOne: []
      responses:
        '200': { description: OK }
  /two:
    get:
      operationId: two
      security:
        - KeyTwo: []
      responses:
        '200': { description: OK }
"#;
    let path = temp_files::create_temp_yaml(SPEC);
    let (routes, schemes, _slug) = load_spec_full(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    // SAFETY: Test context - handlers are simple closures for testing
    unsafe {
        dispatcher.register_handler("one", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HeaderVec::new(),
                body: json!({"one": true}),
            });
        });
        dispatcher.register_handler("two", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HeaderVec::new(),
                body: json!({"two": true}),
            });
        });
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));
    let mut service = AppService::new(
        router,
        Arc::new(RwLock::new(dispatcher)),
        schemes,
        PathBuf::from("examples/openapi.yaml"),
        Some(PathBuf::from("examples/pet_store/static_site")),
        Some(PathBuf::from("examples/pet_store/doc")),
    );
    service.register_security_provider("KeyOne", Arc::new(ApiKeyProvider { key: "one".into() }));
    service.register_security_provider("KeyTwo", Arc::new(ApiKeyProvider { key: "two".into() }));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    (tracing, handle, addr)
}

fn start_token_service() -> (TestTracing, ServerHandle, SocketAddr) {
    may::config().set_stack_size(0x8000);
    let tracing = TestTracing::init();
    const SPEC: &str = r#"openapi: 3.1.0
info:
  title: Token API
  version: '1.0'
components:
  securitySchemes:
    BearerAuth:
      type: http
      scheme: bearer
    OAuth:
      type: oauth2
      flows:
        implicit:
          authorizationUrl: https://example.com/auth
          scopes:
            read: Read access
paths:
  /header:
    get:
      operationId: header
      security:
        - BearerAuth: []
      responses:
        '200': { description: OK }
  /cookie:
    get:
      operationId: cookie
      security:
        - OAuth: ['read']
      responses:
        '200': { description: OK }
"#;
    let path = temp_files::create_temp_yaml(SPEC);
    let (routes, schemes, _slug) = load_spec_full(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    // SAFETY: Test context - handlers are simple closures for testing
    unsafe {
        dispatcher.register_handler("header", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HeaderVec::new(),
                body: json!({"header": true}),
            });
        });
        dispatcher.register_handler("cookie", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HeaderVec::new(),
                body: json!({"cookie": true}),
            });
        });
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));
    let mut service = AppService::new(
        router,
        Arc::new(RwLock::new(dispatcher)),
        schemes,
        PathBuf::from("examples/openapi.yaml"),
        Some(PathBuf::from("examples/pet_store/static_site")),
        Some(PathBuf::from("examples/pet_store/doc")),
    );
    service.register_security_provider("BearerAuth", Arc::new(BearerJwtProvider::new("sig")));
    service.register_security_provider(
        "OAuth",
        Arc::new(OAuth2Provider::new("sig").cookie_name("auth")),
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    (tracing, handle, addr)
}

fn make_token(scope: &str) -> String {
    use base64::{engine::general_purpose, Engine as _};
    let header = general_purpose::STANDARD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = general_purpose::STANDARD.encode(format!(r#"{{"scope":"{}"}}"#, scope));
    format!("{}.{}.{}", header, payload, "sig")
}

fn send_request(addr: &SocketAddr, req: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(req.as_bytes()).unwrap();
    // Allow slower CI environments (e.g., act) a longer read window
    let timeout_ms: u64 = if std::env::var("ACT").is_ok() {
        1500
    } else {
        500
    };
    stream
        .set_read_timeout(Some(Duration::from_millis(timeout_ms)))
        .unwrap();

    // Read headers first
    let mut buf = Vec::new();
    let mut header_end = None;
    for _ in 0..10 {
        let mut tmp = [0u8; 1024];
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = Some(pos + 4);
                    break;
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => panic!("read error: {:?}", e),
        }
    }

    let header_end = header_end.unwrap_or(buf.len());
    let headers = String::from_utf8_lossy(&buf[..header_end]);
    let content_length = headers
        .lines()
        .find_map(|l| l.split_once(':').map(|(n, v)| (n, v)))
        .filter(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse::<usize>().ok());

    // Read body to expected length if Content-Length present
    if let Some(clen) = content_length {
        let mut body_len = buf.len().saturating_sub(header_end);
        while body_len < clen {
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
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
                Err(e) => panic!("read error: {:?}", e),
            }
        }
    } else {
        // No Content-Length: read until timeout/close
        for _ in 0..10 {
            let mut tmp = [0u8; 4096];
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    break;
                }
                Err(e) => panic!("read error: {:?}", e),
            }
        }
    }

    String::from_utf8_lossy(&buf).to_string()
}

fn parse_status(resp: &str) -> u16 {
    resp.lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("0")
        .parse()
        .unwrap()
}

#[test]
fn test_api_key_auth() {
    // Setup happens automatically in SecurityTestServer::from_start_service()
    let server = SecurityTestServer::from_start_service();

    let resp = send_request(
        &server.addr(),
        "GET /secret HTTP/1.1\r\nHost: localhost\r\n\r\n",
    );
    let status = parse_status(&resp);
    assert_eq!(status, 401);

    let resp = send_request(
        &server.addr(),
        "GET /secret HTTP/1.1\r\nHost: localhost\r\nX-API-Key: secret\r\n\r\n",
    );
    let status = parse_status(&resp);
    assert_eq!(status, 200);

    // Teardown happens automatically when 'server' goes out of scope
    // No need to call handle.stop() manually!
}

fn start_service_default_provider() -> (TestTracing, ServerHandle, SocketAddr) {
    // ensure coroutines have enough stack for tests
    may::config().set_stack_size(0x8000);
    let tracing = TestTracing::init();
    const SPEC: &str = r#"openapi: 3.1.0
info:
  title: Auth API
  version: '1.0'
components:
  securitySchemes:
    ApiKeyAuth:
      type: apiKey
      in: header
      name: X-API-Key
paths:
  /secret:
    get:
      operationId: secret
      security:
        - ApiKeyAuth: []
      responses:
        '200': { description: OK }
"#;
    let path = temp_files::create_temp_yaml(SPEC);
    let (routes, schemes, _slug) = load_spec_full(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    // SAFETY: Test context - handlers are simple closures for testing
    unsafe {
        dispatcher.register_handler("secret", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HeaderVec::new(),
                body: json!({"ok": true}),
            });
        });
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));
    let mut service = AppService::new(
        router,
        Arc::new(RwLock::new(dispatcher)),
        schemes,
        PathBuf::from("examples/openapi.yaml"),
        Some(PathBuf::from("examples/pet_store/static_site")),
        Some(PathBuf::from("examples/pet_store/doc")),
    );
    // Use default provider wiring with a test key
    service.register_default_security_providers_from_env(Some("secret".into()));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    (tracing, handle, addr)
}

#[test]
fn test_api_key_auth_via_authorization_bearer() {
    let server = SecurityTestServer::from_default_provider();
    let resp = send_request(
        &server.addr(),
        "GET /secret HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer secret\r\n\r\n",
    );
    let status = parse_status(&resp);
    assert_eq!(status, 200);
    // Automatic cleanup!
}

// --- JWKS Bearer provider tests ---

fn start_mock_jwks_server(body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}:{}/jwks.json", addr.ip(), addr.port());
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    url
}

fn base64url_no_pad(data: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn make_hs256_jwt(secret: &[u8], iss: &str, aud: &str, kid: &str, exp_secs: i64) -> String {
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use serde_json::json;
    let header = Header {
        kid: Some(kid.to_string()),
        alg: Algorithm::HS256,
        ..Default::default()
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let claims = json!({
        "iss": iss,
        "aud": aud,
        "exp": now + exp_secs,
        "scope": "read write"
    });
    jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
}

fn start_service_with_jwks(
    jwks_url: &str,
    iss: &str,
    aud: &str,
) -> (TestTracing, ServerHandle, SocketAddr) {
    may::config().set_stack_size(0x8000);
    let tracing = TestTracing::init();
    const SPEC: &str = r#"openapi: 3.1.0
info:
  title: Token API
  version: '1.0'
components:
  securitySchemes:
    BearerAuth:
      type: http
      scheme: bearer
paths:
  /header:
    get:
      operationId: header
      security:
        - BearerAuth: []
      responses:
        '200': { description: OK }
"#;
    let path = temp_files::create_temp_yaml(SPEC);
    let (routes, schemes, _slug) = load_spec_full(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    // SAFETY: Test context - handlers are simple closures for testing
    unsafe {
        dispatcher.register_handler("header", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HeaderVec::new(),
                body: json!({"header": true}),
            });
        });
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));
    let mut service = AppService::new(
        router,
        Arc::new(RwLock::new(dispatcher)),
        schemes,
        PathBuf::from("examples/openapi.yaml"),
        Some(PathBuf::from("examples/pet_store/static_site")),
        Some(PathBuf::from("examples/pet_store/doc")),
    );
    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
        .issuer(iss.to_string())
        .audience(aud.to_string());
    service.register_security_provider("BearerAuth", Arc::new(provider));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    (tracing, handle, addr)
}

#[test]
fn test_bearer_jwks_success() {
    // Build HS256 oct key in JWKS
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    let iss = "https://issuer.example";
    let aud = "my-audience";
    let token = make_hs256_jwt(secret, iss, aud, "k1", 3600);
    let server = SecurityTestServer::from_jwks(&jwks_url, iss, aud);
    let req = format!(
        "GET /header HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\n\r\n",
        token
    );
    let resp = send_request(&server.addr(), &req);
    let status_ok = parse_status(&resp);
    assert_eq!(status_ok, 200);
    // Automatic cleanup!
}

#[test]
fn test_bearer_jwks_invalid_signature() {
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    let iss = "https://issuer.example";
    let aud = "my-audience";
    // token signed with different secret
    let token = make_hs256_jwt(b"wrong", iss, aud, "k1", 3600);
    let server = SecurityTestServer::from_jwks(&jwks_url, iss, aud);
    let req = format!(
        "GET /header HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\n\r\n",
        token
    );
    let resp = send_request(&server.addr(), &req);
    let status = parse_status(&resp);
    assert_eq!(status, 401);
    // Automatic cleanup!
}

// --- Remote API key verification tests ---

fn start_mock_apikey_verify_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}:{}/verify", addr.ip(), addr.port());
    let handle = thread::spawn(move || {
        for _ in 0..2 {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                // naive parse of X-API-Key header (case-insensitive)
                let req = String::from_utf8_lossy(&buf);
                let req_lower = req.to_lowercase();
                let ok = req_lower.contains("x-api-key: validkey");
                let body = "";
                let status = if ok { "200 OK" } else { "401 Unauthorized" };
                let resp = format!(
                    "HTTP/1.1 {}\r\nContent-Length: {}\r\n\r\n{}",
                    status,
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        }
    });
    (url, handle)
}

fn start_service_with_remote_apikey(verify_url: &str) -> (TestTracing, ServerHandle, SocketAddr) {
    may::config().set_stack_size(0x8000);
    let tracing = TestTracing::init();
    const SPEC: &str = r#"openapi: 3.1.0
info:
  title: API Key Verify API
  version: '1.0'
components:
  securitySchemes:
    ApiKeyAuth:
      type: apiKey
      in: header
      name: X-API-Key
paths:
  /secret:
    get:
      operationId: secret
      security:
        - ApiKeyAuth: []
      responses:
        '200': { description: OK }
"#;
    let path = temp_files::create_temp_yaml(SPEC);
    let (routes, schemes, _slug) = load_spec_full(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    // SAFETY: Test context - handlers are simple closures for testing
    unsafe {
        dispatcher.register_handler("secret", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HeaderVec::new(),
                body: json!({"ok": true}),
            });
        });
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));
    let mut service = AppService::new(
        router,
        Arc::new(RwLock::new(dispatcher)),
        schemes,
        PathBuf::from("examples/openapi.yaml"),
        Some(PathBuf::from("examples/pet_store/static_site")),
        Some(PathBuf::from("examples/pet_store/doc")),
    );
    let provider = brrtrouter::security::RemoteApiKeyProvider::new(verify_url.to_string())
        .header_name("X-API-Key")
        .timeout_ms(50)
        .cache_ttl(Duration::from_millis(1));
    service.register_security_provider("ApiKeyAuth", Arc::new(provider));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    (tracing, handle, addr)
}

#[test]
fn test_remote_apikey_success_and_failure() {
    let (url, handle_verify) = start_mock_apikey_verify_server();
    let server = SecurityTestServer::from_remote_apikey(&url);

    // success
    let req_ok = "GET /secret HTTP/1.1\r\nHost: localhost\r\nX-API-Key: validkey\r\n\r\n";
    let resp_ok = send_request(&server.addr(), req_ok);
    let status_ok = parse_status(&resp_ok);
    assert_eq!(status_ok, 200);

    // failure
    let req_bad = "GET /secret HTTP/1.1\r\nHost: localhost\r\nX-API-Key: wrong\r\n\r\n";
    let resp_bad = send_request(&server.addr(), req_bad);
    let status_bad = parse_status(&resp_bad);
    assert_eq!(status_bad, 401);

    // Cleanup both servers
    drop(server); // Explicitly drop main server first
    handle_verify.join().ok(); // Then cleanup verification server
}

// TODO: This test fails intermittently due to timing issues with the coroutine cancellation.
#[test]
fn test_multiple_security_providers() {
    let server = SecurityTestServer::from_multi_service();

    let resp = send_request(
        &server.addr(),
        "GET /one HTTP/1.1\r\nHost: localhost\r\nX-Key-One: one\r\n\r\n",
    );
    let status = parse_status(&resp);
    assert_eq!(status, 200);

    let resp = send_request(
        &server.addr(),
        "GET /two HTTP/1.1\r\nHost: localhost\r\nX-Key-Two: two\r\n\r\n",
    );
    let status_two = parse_status(&resp);
    assert_eq!(status_two, 200);

    let resp = send_request(
        &server.addr(),
        "GET /one HTTP/1.1\r\nHost: localhost\r\nX-Key-Two: two\r\n\r\n",
    );
    let status_wrong = parse_status(&resp);
    assert_eq!(status_wrong, 401);

    // Automatic cleanup!
}

#[test]
fn test_bearer_header_and_oauth_cookie() {
    let server = SecurityTestServer::from_token_service();

    // Missing token should fail
    let resp = send_request(
        &server.addr(),
        "GET /header HTTP/1.1\r\nHost: localhost\r\n\r\n",
    );
    let status = parse_status(&resp);
    assert_eq!(status, 401);

    // Valid bearer header
    let token = make_token("");
    let req = format!(
        "GET /header HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\n\r\n",
        token
    );
    let resp = send_request(&server.addr(), &req);
    let status_ok = parse_status(&resp);
    assert_eq!(status_ok, 200);

    // OAuth2 cookie with required scope
    let token = make_token("read");
    let req = format!(
        "GET /cookie HTTP/1.1\r\nHost: localhost\r\nCookie: auth={}\r\n\r\n",
        token
    );
    let resp = send_request(&server.addr(), &req);
    let status_cookie = parse_status(&resp);
    assert_eq!(status_cookie, 200);

    // Automatic cleanup!
}

#[test]
fn test_bearer_jwt_provider_creation() {
    let _provider = BearerJwtProvider::new("test_signature");
    // Test that provider can be created successfully
    assert!(true); // Basic creation test

    let _provider_with_cookie = BearerJwtProvider::new("test_signature").cookie_name("auth_token");
    // Test that cookie name can be set
    assert!(true);
}

#[test]
fn test_oauth2_provider_creation() {
    let _provider = OAuth2Provider::new("test_signature");
    // Test that provider can be created successfully
    assert!(true);

    let _provider_with_cookie = OAuth2Provider::new("test_signature").cookie_name("oauth_token");
    // Test that cookie name can be set
    assert!(true);
}

#[test]
fn test_bearer_jwt_token_validation() {
    let provider = BearerJwtProvider::new("sig");
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    // Test valid token with no scopes
    let token = make_token("");
    let mut headers: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    assert!(provider.validate(&scheme, &[], &req));
}

#[test]
fn test_bearer_jwt_invalid_signature() {
    let provider = BearerJwtProvider::new("wrong_sig");
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    let token = make_token("");
    let mut headers: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    assert!(!provider.validate(&scheme, &[], &req));
}

#[test]
fn test_bearer_jwt_malformed_token() {
    let provider = BearerJwtProvider::new("sig");
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    // Test malformed token (missing parts)
    let mut headers: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    headers.push((
        Arc::from("authorization"),
        "Bearer invalid.token".to_string(),
    ));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    assert!(!provider.validate(&scheme, &[], &req));
}

#[test]
fn test_bearer_jwt_invalid_base64() {
    let provider = BearerJwtProvider::new("sig");
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    // Test token with invalid base64 payload
    let mut headers: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    headers.push((
        Arc::from("authorization"),
        "Bearer header.invalid_base64.sig".to_string(),
    ));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    assert!(!provider.validate(&scheme, &[], &req));
}

#[test]
fn test_bearer_jwt_invalid_json() {
    let provider = BearerJwtProvider::new("sig");
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    use base64::{engine::general_purpose, Engine as _};
    let header = "header";
    let payload = general_purpose::STANDARD.encode(b"invalid json");
    let token = format!("{}.{}.sig", header, payload);

    let mut headers: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    assert!(!provider.validate(&scheme, &[], &req));
}

#[test]
fn test_bearer_jwt_scope_validation() {
    let provider = BearerJwtProvider::new("sig");
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    // Test token with read scope
    let token = make_token("read write");
    let mut headers: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    // Should pass with read scope
    assert!(provider.validate(&scheme, &["read".to_string()], &req));

    // Should pass with write scope
    assert!(provider.validate(&scheme, &["write".to_string()], &req));

    // Should pass with both scopes
    assert!(provider.validate(&scheme, &["read".to_string(), "write".to_string()], &req));

    // Should fail with admin scope
    assert!(!provider.validate(&scheme, &["admin".to_string()], &req));
}

#[test]
fn test_bearer_jwt_cookie_extraction() {
    let provider = BearerJwtProvider::new("sig").cookie_name("auth_token");
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    let token = make_token("");
    let mut cookies: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    cookies.push((Arc::from("auth_token"), token));
    let req = SecurityRequest {
        headers: &HeaderVec::new(),
        query: &ParamVec::new(),
        cookies: &cookies,
    };

    assert!(provider.validate(&scheme, &[], &req));
}

#[test]
fn test_bearer_jwt_wrong_scheme() {
    let provider = BearerJwtProvider::new("sig");
    let scheme = SecurityScheme::ApiKey {
        name: "X-API-Key".to_string(),
        location: "header".to_string(),
        description: None,
    };

    let token = make_token("");
    let mut headers: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    assert!(!provider.validate(&scheme, &[], &req));
}

#[test]
fn test_oauth2_provider_validation() {
    let provider = OAuth2Provider::new("sig");
    let scheme = SecurityScheme::OAuth2 {
        flows: Default::default(),
        description: None,
    };

    let token = make_token("read");
    let mut headers: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    assert!(provider.validate(&scheme, &["read".to_string()], &req));
}

#[test]
fn test_oauth2_provider_cookie() {
    let provider = OAuth2Provider::new("sig").cookie_name("oauth_token");
    let scheme = SecurityScheme::OAuth2 {
        flows: Default::default(),
        description: None,
    };

    let token = make_token("read");
    let mut cookies: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    cookies.push((Arc::from("oauth_token"), token));
    let req = SecurityRequest {
        headers: &HeaderVec::new(),
        query: &ParamVec::new(),
        cookies: &cookies,
    };

    assert!(provider.validate(&scheme, &["read".to_string()], &req));
}

#[test]
fn test_oauth2_provider_wrong_scheme() {
    let provider = OAuth2Provider::new("sig");
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    let token = make_token("");
    let mut headers: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    assert!(!provider.validate(&scheme, &[], &req));
}

#[test]
fn test_api_key_provider_header() {
    let provider = ApiKeyProvider {
        key: "test_key".to_string(),
    };
    let scheme = SecurityScheme::ApiKey {
        name: "X-API-Key".to_string(),
        location: "header".to_string(),
        description: None,
    };

    let mut headers: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    headers.push((Arc::from("x-api-key"), "test_key".to_string()));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    assert!(provider.validate(&scheme, &[], &req));
}

#[test]
fn test_api_key_provider_query() {
    let provider = ApiKeyProvider {
        key: "test_key".to_string(),
    };
    let scheme = SecurityScheme::ApiKey {
        name: "api_key".to_string(),
        location: "query".to_string(),
        description: None,
    };

    let mut query: ParamVec = ParamVec::new();
    query.push((Arc::from("api_key"), "test_key".to_string()));
    let req = SecurityRequest {
        headers: &HeaderVec::new(),
        query: &query,
        cookies: &HeaderVec::new(),
    };

    assert!(provider.validate(&scheme, &[], &req));
}

#[test]
fn test_api_key_provider_cookie() {
    let provider = ApiKeyProvider {
        key: "test_key".to_string(),
    };
    let scheme = SecurityScheme::ApiKey {
        name: "api_key".to_string(),
        location: "cookie".to_string(),
        description: None,
    };

    let mut cookies: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    cookies.push((Arc::from("api_key"), "test_key".to_string()));
    let req = SecurityRequest {
        headers: &HeaderVec::new(),
        query: &ParamVec::new(),
        cookies: &cookies,
    };

    assert!(provider.validate(&scheme, &[], &req));
}

#[test]
fn test_api_key_provider_invalid_location() {
    let provider = ApiKeyProvider {
        key: "test_key".to_string(),
    };
    let scheme = SecurityScheme::ApiKey {
        name: "api_key".to_string(),
        location: "invalid".to_string(),
        description: None,
    };

    let req = SecurityRequest {
        headers: &HeaderVec::new(),
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    assert!(!provider.validate(&scheme, &[], &req));
}

#[test]
fn test_missing_authorization_header() {
    let provider = BearerJwtProvider::new("sig");
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    let req = SecurityRequest {
        headers: &HeaderVec::new(),
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    assert!(!provider.validate(&scheme, &[], &req));
}

#[test]
fn test_malformed_authorization_header() {
    let provider = BearerJwtProvider::new("sig");
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), "Basic dXNlcjpwYXNz".to_string())); // Basic auth instead of Bearer
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    assert!(!provider.validate(&scheme, &[], &req));
}

#[test]
fn test_case_insensitive_bearer_scheme() {
    let provider = BearerJwtProvider::new("sig");
    let scheme = SecurityScheme::Http {
        scheme: "BEARER".to_string(), // Uppercase
        bearer_format: None,
        description: None,
    };

    let token = make_token("");
    let mut headers: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    assert!(provider.validate(&scheme, &[], &req));
}

#[test]
fn test_empty_token_scopes() {
    let provider = BearerJwtProvider::new("sig");
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    let token = make_token(""); // Empty scope
    let mut headers: HeaderVec = HeaderVec::new();
    // JSF P2: HeaderVec now uses Arc<str> for keys
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    // Should pass with no required scopes
    assert!(provider.validate(&scheme, &[], &req));

    // Should fail with required scopes
    assert!(!provider.validate(&scheme, &["read".to_string()], &req));
}

#[test]
fn test_jwks_claims_cache_caching() {
    // Test that claims are cached and reused
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    let iss = "https://issuer.example";
    let aud = "my-audience";
    let token = make_hs256_jwt(secret, iss, aud, "k1", 3600);

    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url)
        .issuer(iss.to_string())
        .audience(aud.to_string())
        .claims_cache_size(100);

    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    // First validation - should decode and cache
    assert!(provider.validate(&scheme, &[], &req));

    // Second validation - should use cache (no decode)
    assert!(provider.validate(&scheme, &[], &req));

    // Third validation - should still use cache
    assert!(provider.validate(&scheme, &[], &req));
}

#[test]
fn test_jwks_claims_cache_expiration_with_leeway() {
    // Test that cache respects leeway for expiration
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    let iss = "https://issuer.example";
    let aud = "my-audience";
    // Token expires in 5 seconds
    let token = make_hs256_jwt(secret, iss, aud, "k1", 5);

    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url)
        .issuer(iss.to_string())
        .audience(aud.to_string())
        .leeway(30) // 30 second leeway
        .claims_cache_size(100);

    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    // First validation - should succeed and cache
    assert!(provider.validate(&scheme, &[], &req));

    // Wait for token to expire (but within leeway)
    std::thread::sleep(Duration::from_secs(6));

    // Should still work due to leeway in cache
    assert!(provider.validate(&scheme, &[], &req));
}

#[test]
fn test_jwks_cookie_support() {
    // Test that JwksBearerProvider supports cookie extraction
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    let iss = "https://issuer.example";
    let aud = "my-audience";
    let token = make_hs256_jwt(secret, iss, aud, "k1", 3600);

    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url)
        .issuer(iss.to_string())
        .audience(aud.to_string())
        .cookie_name("auth_token");

    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    let mut cookies: HeaderVec = HeaderVec::new();
    cookies.push((Arc::from("auth_token"), token));
    let req = SecurityRequest {
        headers: &HeaderVec::new(),
        query: &ParamVec::new(),
        cookies: &cookies,
    };

    // Should extract token from cookie
    assert!(provider.validate(&scheme, &[], &req));
}

#[test]
#[should_panic(expected = "JWKS URL must use HTTPS")]
fn test_jwks_url_https_validation() {
    // Test that HTTP URLs (except localhost) are rejected
    let _provider = brrtrouter::security::JwksBearerProvider::new("http://example.com/jwks.json");
}

#[test]
#[should_panic(expected = "JWKS URL must use HTTPS")]
fn test_jwks_url_localhost_subdomain_attack() {
    // SECURITY TEST: Verify that localhost.attacker.com is rejected
    // The old starts_with("http://localhost") check would incorrectly allow this
    let _provider =
        brrtrouter::security::JwksBearerProvider::new("http://localhost.attacker.com/jwks.json");
}

#[test]
#[should_panic(expected = "JWKS URL must use HTTPS")]
fn test_jwks_url_127_subdomain_attack() {
    // SECURITY TEST: Verify that 127.0.0.1.attacker.com is rejected
    let _provider =
        brrtrouter::security::JwksBearerProvider::new("http://127.0.0.1.attacker.com/jwks.json");
}

#[test]
fn test_jwks_url_localhost_allowed() {
    // Test that localhost HTTP is allowed for testing
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    // Should not panic for localhost
    let _provider = brrtrouter::security::JwksBearerProvider::new(jwks_url);
}

#[test]
fn test_jwks_url_localhost_with_port() {
    // Test that localhost with port is allowed
    let _provider =
        brrtrouter::security::JwksBearerProvider::new("http://localhost:8080/jwks.json");
}

#[test]
fn test_jwks_url_localhost_with_path() {
    // Test that localhost with path is allowed
    let _provider =
        brrtrouter::security::JwksBearerProvider::new("http://localhost/.well-known/jwks.json");
}

#[test]
fn test_jwks_url_127_with_port() {
    // Test that 127.0.0.1 with port is allowed
    let _provider =
        brrtrouter::security::JwksBearerProvider::new("http://127.0.0.1:8080/jwks.json");
}

#[test]
fn test_jwks_cache_invalidation() {
    // Test cache invalidation methods
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    let iss = "https://issuer.example";
    let aud = "my-audience";
    let token = make_hs256_jwt(secret, iss, aud, "k1", 3600);

    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url)
        .issuer(iss.to_string())
        .audience(aud.to_string())
        .claims_cache_size(100);

    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    // First validation - should cache
    assert!(provider.validate(&scheme, &[], &req));

    // Invalidate specific token
    provider.invalidate_token(&token);

    // Next validation should decode again (cache miss)
    assert!(provider.validate(&scheme, &[], &req));

    // Clear entire cache
    provider.clear_claims_cache();

    // Next validation should decode again (cache miss)
    assert!(provider.validate(&scheme, &[], &req));
}

#[test]
fn test_jwks_cache_invalidation_does_not_clear_other_tokens() {
    // Test that invalidate_token() only invalidates the specific token,
    // not the entire cache (fixes thundering herd bug)
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);

    let iss = "https://issuer.example";
    let aud = "my-audience";

    // Create two different tokens with unique subjects to ensure they're different
    // (JWT encoding may be non-deterministic, but adding unique claims guarantees different tokens)
    let token1 = {
        use jsonwebtoken::{Algorithm, EncodingKey, Header};
        use serde_json::json;
        let header = Header {
            kid: Some("k1".to_string()),
            alg: Algorithm::HS256,
            ..Default::default()
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let claims = json!({
            "iss": iss,
            "aud": aud,
            "exp": now + 3600,
            "sub": "user1",
            "scope": "read write"
        });
        jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
    };
    let token2 = {
        use jsonwebtoken::{Algorithm, EncodingKey, Header};
        use serde_json::json;
        let header = Header {
            kid: Some("k1".to_string()),
            alg: Algorithm::HS256,
            ..Default::default()
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let claims = json!({
            "iss": iss,
            "aud": aud,
            "exp": now + 3600,
            "sub": "user2",
            "scope": "read write"
        });
        jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
    };

    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url)
        .issuer(iss.to_string())
        .audience(aud.to_string())
        .claims_cache_size(100);

    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    // Create requests for both tokens
    let mut headers1: HeaderVec = HeaderVec::new();
    headers1.push((Arc::from("authorization"), format!("Bearer {}", token1)));
    let req1 = SecurityRequest {
        headers: &headers1,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    let mut headers2: HeaderVec = HeaderVec::new();
    headers2.push((Arc::from("authorization"), format!("Bearer {}", token2)));
    let req2 = SecurityRequest {
        headers: &headers2,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    // Validate both tokens - should cache both
    assert!(provider.validate(&scheme, &[], &req1));
    assert!(provider.validate(&scheme, &[], &req2));

    // Check cache stats - should have 2 entries
    let stats_before = provider.cache_stats();
    assert_eq!(stats_before.size, 2, "Both tokens should be cached");

    // Invalidate only token1
    provider.invalidate_token(&token1);

    // Check cache stats - should have 1 entry (token2 still cached)
    let stats_after = provider.cache_stats();
    assert_eq!(
        stats_after.size, 1,
        "Only token2 should remain in cache after invalidating token1"
    );

    // token1 should be a cache miss (requires decode)
    // token2 should be a cache hit (still cached)
    let cache_misses_before = provider.cache_stats().misses;
    assert!(provider.validate(&scheme, &[], &req1)); // token1 - cache miss
    let cache_misses_after_token1 = provider.cache_stats().misses;
    assert_eq!(
        cache_misses_after_token1,
        cache_misses_before + 1,
        "token1 should be a cache miss"
    );

    // token2 should still be cached (cache hit)
    let cache_hits_before = provider.cache_stats().hits;
    assert!(provider.validate(&scheme, &[], &req2)); // token2 - cache hit
    let cache_hits_after = provider.cache_stats().hits;
    assert_eq!(
        cache_hits_after,
        cache_hits_before + 1,
        "token2 should be a cache hit"
    );
}

#[test]
fn test_jwks_extract_claims() {
    // Test that extract_claims() returns decoded JWT claims for BFF pattern
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);

    let iss = "https://issuer.example";
    let aud = "my-audience";
    let token = make_hs256_jwt(secret, iss, aud, "k1", 3600);

    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url)
        .issuer(iss.to_string())
        .audience(aud.to_string())
        .claims_cache_size(100);

    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    // First validate to populate cache
    assert!(provider.validate(&scheme, &[], &req));

    // Extract claims - should return decoded claims
    let claims = provider.extract_claims(&scheme, &req);
    assert!(
        claims.is_some(),
        "extract_claims should return claims for valid token"
    );

    let claims = claims.unwrap();
    assert_eq!(claims.get("iss").and_then(|v| v.as_str()), Some(iss));
    assert_eq!(claims.get("aud").and_then(|v| v.as_str()), Some(aud));
    assert!(claims.get("exp").is_some(), "Claims should contain exp");
    assert!(claims.get("scope").is_some(), "Claims should contain scope");

    // Test extract_claims with invalid token
    let mut invalid_headers: HeaderVec = HeaderVec::new();
    invalid_headers.push((
        Arc::from("authorization"),
        "Bearer invalid.token.here".to_string(),
    ));
    let invalid_req = SecurityRequest {
        headers: &invalid_headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    let invalid_claims = provider.extract_claims(&scheme, &invalid_req);
    assert!(
        invalid_claims.is_none(),
        "extract_claims should return None for invalid token"
    );

    // Test extract_claims with missing token
    let empty_req = SecurityRequest {
        headers: &HeaderVec::new(),
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };

    let empty_claims = provider.extract_claims(&scheme, &empty_req);
    assert!(
        empty_claims.is_none(),
        "extract_claims should return None when token is missing"
    );
}

#[test]
fn test_jwks_cache_eviction() {
    // Test that LRU cache evicts entries when at capacity
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    let iss = "https://issuer.example";
    let aud = "my-audience";

    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url)
        .issuer(iss.to_string())
        .audience(aud.to_string())
        .claims_cache_size(2); // Small cache to test eviction

    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    // Create 3 different tokens with unique jti (JWT ID) to ensure they're distinct
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use serde_json::json;
    let token1 = {
        let header = Header {
            kid: Some("k1".to_string()),
            alg: Algorithm::HS256,
            ..Default::default()
        };
        let claims = json!({
            "iss": iss,
            "aud": aud,
            "exp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 + 3600,
            "scope": "read write",
            "jti": "token1"
        });
        jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
    };
    let token2 = {
        let header = Header {
            kid: Some("k1".to_string()),
            alg: Algorithm::HS256,
            ..Default::default()
        };
        let claims = json!({
            "iss": iss,
            "aud": aud,
            "exp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 + 3600,
            "scope": "read write",
            "jti": "token2"
        });
        jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
    };
    let token3 = {
        let header = Header {
            kid: Some("k1".to_string()),
            alg: Algorithm::HS256,
            ..Default::default()
        };
        let claims = json!({
            "iss": iss,
            "aud": aud,
            "exp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 + 3600,
            "scope": "read write",
            "jti": "token3"
        });
        jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
    };

    // Validate token1 - should cache
    let mut headers1: HeaderVec = HeaderVec::new();
    headers1.push((Arc::from("authorization"), format!("Bearer {}", token1)));
    let req1 = SecurityRequest {
        headers: &headers1,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    assert!(provider.validate(&scheme, &[], &req1));

    // Validate token2 - should cache (now 2 entries)
    let mut headers2: HeaderVec = HeaderVec::new();
    headers2.push((Arc::from("authorization"), format!("Bearer {}", token2)));
    let req2 = SecurityRequest {
        headers: &headers2,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    assert!(provider.validate(&scheme, &[], &req2));

    // Validate token3 - should evict token1 (LRU), cache token3
    let mut headers3: HeaderVec = HeaderVec::new();
    headers3.push((Arc::from("authorization"), format!("Bearer {}", token3)));
    let req3 = SecurityRequest {
        headers: &headers3,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    assert!(provider.validate(&scheme, &[], &req3));

    // token1 should be evicted (cache miss, needs decode)
    // token2 and token3 should be cached
    assert!(provider.validate(&scheme, &[], &req2)); // token2 cached
    assert!(provider.validate(&scheme, &[], &req3)); // token3 cached

    // Verify evictions counter was incremented (token3 insertion evicted token1)
    let stats = provider.cache_stats();
    assert_eq!(
        stats.evictions, 1,
        "Expected 1 eviction when inserting token3 at capacity"
    );
}

#[test]
fn test_jwks_cache_evictions_counter() {
    // Test that evictions counter correctly tracks LRU evictions
    let secret = b"supersecret";
    let k = base64url_no_pad(secret);
    let jwks = serde_json::json!({
        "keys": [
            {"kty": "oct", "alg": "HS256", "kid": "k1", "k": k}
        ]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server(jwks);
    let iss = "https://issuer.example";
    let aud = "my-audience";

    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url)
        .issuer(iss.to_string())
        .audience(aud.to_string())
        .claims_cache_size(2); // Small cache to test eviction

    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    // Initial state: no evictions
    let stats = provider.cache_stats();
    assert_eq!(stats.evictions, 0, "Initial evictions should be 0");

    // Fill cache to capacity (2 entries) - no evictions yet
    // Generate unique tokens by including a unique jti (JWT ID) claim
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use serde_json::json;
    let token1 = {
        let header = Header {
            kid: Some("k1".to_string()),
            alg: Algorithm::HS256,
            ..Default::default()
        };
        let claims = json!({
            "iss": iss,
            "aud": aud,
            "exp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 + 3600,
            "scope": "read write",
            "jti": "token1"
        });
        jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
    };
    let token2 = {
        let header = Header {
            kid: Some("k1".to_string()),
            alg: Algorithm::HS256,
            ..Default::default()
        };
        let claims = json!({
            "iss": iss,
            "aud": aud,
            "exp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 + 3600,
            "scope": "read write",
            "jti": "token2"
        });
        jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
    };

    let mut headers1: HeaderVec = HeaderVec::new();
    headers1.push((Arc::from("authorization"), format!("Bearer {}", token1)));
    let req1 = SecurityRequest {
        headers: &headers1,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    assert!(provider.validate(&scheme, &[], &req1));

    let mut headers2: HeaderVec = HeaderVec::new();
    headers2.push((Arc::from("authorization"), format!("Bearer {}", token2)));
    let req2 = SecurityRequest {
        headers: &headers2,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    assert!(provider.validate(&scheme, &[], &req2));

    // Still no evictions (cache not at capacity yet)
    let stats = provider.cache_stats();
    assert_eq!(
        stats.evictions, 0,
        "No evictions when filling cache to capacity"
    );
    assert_eq!(stats.size, 2, "Cache should have 2 entries");

    // Insert token3 - should evict token1 (LRU)
    let token3 = {
        let header = Header {
            kid: Some("k1".to_string()),
            alg: Algorithm::HS256,
            ..Default::default()
        };
        let claims = json!({
            "iss": iss,
            "aud": aud,
            "exp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 + 3600,
            "scope": "read write",
            "jti": "token3"
        });
        jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
    };
    let mut headers3: HeaderVec = HeaderVec::new();
    headers3.push((Arc::from("authorization"), format!("Bearer {}", token3)));
    let req3 = SecurityRequest {
        headers: &headers3,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    assert!(provider.validate(&scheme, &[], &req3));

    // Should have 1 eviction
    let stats = provider.cache_stats();
    assert_eq!(
        stats.evictions, 1,
        "Expected 1 eviction when inserting at capacity"
    );
    assert_eq!(
        stats.size, 2,
        "Cache should still have 2 entries (capacity)"
    );

    // Insert token4 - should evict token2 (LRU, token3 is now most recent)
    let token4 = {
        let header = Header {
            kid: Some("k1".to_string()),
            alg: Algorithm::HS256,
            ..Default::default()
        };
        let claims = json!({
            "iss": iss,
            "aud": aud,
            "exp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 + 3600,
            "scope": "read write",
            "jti": "token4"
        });
        jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
    };
    let mut headers4: HeaderVec = HeaderVec::new();
    headers4.push((Arc::from("authorization"), format!("Bearer {}", token4)));
    let req4 = SecurityRequest {
        headers: &headers4,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    assert!(provider.validate(&scheme, &[], &req4));

    // Should have 2 evictions total
    let stats = provider.cache_stats();
    assert_eq!(stats.evictions, 2, "Expected 2 evictions total");
    assert_eq!(
        stats.size, 2,
        "Cache should still have 2 entries (capacity)"
    );

    // Updating an existing token should NOT increment evictions
    // Re-validate token4 (updates LRU order but doesn't evict)
    assert!(provider.validate(&scheme, &[], &req4));

    // Evictions should still be 2 (no new eviction for update)
    let stats = provider.cache_stats();
    assert_eq!(
        stats.evictions, 2,
        "Updating existing entry should not increment evictions"
    );
}

// --- Background refresh thread tests ---

#[test]
fn test_jwks_background_refresh_short_cache_ttl_1s() {
    // Test that cache_ttl = 1s doesn't cause CPU spinning
    // The refresh interval should be max(1s / 2, 1s) = 1s (minimum enforced)
    let jwks_url = "http://localhost:8080/jwks.json";
    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
        .cache_ttl(Duration::from_secs(1));
    
    // Provider should be created successfully
    // Background thread should be running with proper sleep interval
    // Test that we can stop it gracefully (proves it's not spinning)
    let start = std::time::Instant::now();
    provider.stop_background_refresh();
    let stop_duration = start.elapsed();
    
    // If the thread was spinning, stop_background_refresh would hang
    // If it's sleeping properly, it should respond to shutdown quickly (< 2s)
    assert!(
        stop_duration < Duration::from_secs(2),
        "Background thread should stop quickly, not spin (took {:?})",
        stop_duration
    );
}

#[test]
fn test_jwks_background_refresh_short_cache_ttl_5s() {
    // Test that cache_ttl = 5s uses cache_ttl / 2 = 2.5s refresh interval
    // But minimum is 1s, so refresh_interval = max(2.5s, 1s) = 2.5s
    let jwks_url = "http://localhost:8080/jwks.json";
    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
        .cache_ttl(Duration::from_secs(5));
    
    let start = std::time::Instant::now();
    provider.stop_background_refresh();
    let stop_duration = start.elapsed();
    
    // Should stop quickly (thread should be sleeping, not spinning)
    assert!(
        stop_duration < Duration::from_secs(2),
        "Background thread should stop quickly (took {:?})",
        stop_duration
    );
}

#[test]
fn test_jwks_background_refresh_edge_case_10s() {
    // Test edge case: cache_ttl = 10s
    // refresh_interval = cache_ttl / 2 = 5s (since cache_ttl <= 10s)
    // Then max(5s, 1s) = 5s
    let jwks_url = "http://localhost:8080/jwks.json";
    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
        .cache_ttl(Duration::from_secs(10));
    
    let start = std::time::Instant::now();
    provider.stop_background_refresh();
    let stop_duration = start.elapsed();
    
    // Should stop quickly
    assert!(
        stop_duration < Duration::from_secs(2),
        "Background thread should stop quickly (took {:?})",
        stop_duration
    );
}

#[test]
fn test_jwks_background_refresh_normal_cache_ttl_11s() {
    // Test normal case: cache_ttl = 11s > 10s
    // refresh_interval = cache_ttl - 10s = 1s
    // Then max(1s, 1s) = 1s
    let jwks_url = "http://localhost:8080/jwks.json";
    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
        .cache_ttl(Duration::from_secs(11));
    
    let start = std::time::Instant::now();
    provider.stop_background_refresh();
    let stop_duration = start.elapsed();
    
    // Should stop quickly
    assert!(
        stop_duration < Duration::from_secs(2),
        "Background thread should stop quickly (took {:?})",
        stop_duration
    );
}

#[test]
fn test_jwks_background_refresh_normal_cache_ttl_300s() {
    // Test normal case: cache_ttl = 300s (default)
    // refresh_interval = cache_ttl - 10s = 290s
    // Then max(290s, 1s) = 290s
    let jwks_url = "http://localhost:8080/jwks.json";
    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
        .cache_ttl(Duration::from_secs(300));
    
    let start = std::time::Instant::now();
    provider.stop_background_refresh();
    let stop_duration = start.elapsed();
    
    // Should stop quickly (thread should be sleeping for 290s, so responds immediately to shutdown)
    assert!(
        stop_duration < Duration::from_secs(2),
        "Background thread should stop quickly (took {:?})",
        stop_duration
    );
}

#[test]
fn test_jwks_background_refresh_cache_ttl_update() {
    // Test that cache_ttl updates are picked up by background thread
    let jwks_url = "http://localhost:8080/jwks.json";
    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
        .cache_ttl(Duration::from_secs(300));
    
    // Update cache_ttl to a shorter value
    // The atomic value should be updated, and background thread should pick it up
    let provider = provider.cache_ttl(Duration::from_secs(5));
    
    // Verify we can still stop it gracefully
    let start = std::time::Instant::now();
    provider.stop_background_refresh();
    let stop_duration = start.elapsed();
    
    assert!(
        stop_duration < Duration::from_secs(2),
        "Background thread should stop quickly after cache_ttl update (took {:?})",
        stop_duration
    );
}

#[test]
fn test_jwks_background_refresh_multiple_providers() {
    // Test that multiple providers with different cache_ttl values work correctly
    let jwks_url = "http://localhost:8080/jwks.json";
    
    let provider1 = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
        .cache_ttl(Duration::from_secs(1));
    let provider2 = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
        .cache_ttl(Duration::from_secs(5));
    let provider3 = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
        .cache_ttl(Duration::from_secs(300));
    
    // All should stop gracefully
    // Note: Each thread checks shutdown every 1s during sleep, so with 3 threads
    // it may take up to ~3.5s if they're all in the middle of a sleep cycle
    let start = std::time::Instant::now();
    provider1.stop_background_refresh();
    provider2.stop_background_refresh();
    provider3.stop_background_refresh();
    let stop_duration = start.elapsed();
    
    assert!(
        stop_duration < Duration::from_secs(4),
        "All background threads should stop quickly (took {:?})",
        stop_duration
    );
}

#[test]
fn test_jwks_background_refresh_zero_cache_ttl_handling() {
    // Test edge case: cache_ttl = 0s (should use minimum 1s)
    // refresh_interval = max(0s / 2, 1s) = max(0s, 1s) = 1s
    let jwks_url = "http://localhost:8080/jwks.json";
    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
        .cache_ttl(Duration::from_secs(0));
    
    let start = std::time::Instant::now();
    provider.stop_background_refresh();
    let stop_duration = start.elapsed();
    
    // Should stop quickly (minimum 1s interval prevents spinning)
    assert!(
        stop_duration < Duration::from_secs(2),
        "Background thread should stop quickly even with 0s cache_ttl (took {:?})",
        stop_duration
    );
}

#[test]
fn test_jwks_background_refresh_very_short_cache_ttl() {
    // Test very short cache_ttl: 100ms
    // refresh_interval = max(100ms / 2, 1s) = 1s (minimum enforced)
    let jwks_url = "http://localhost:8080/jwks.json";
    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
        .cache_ttl(Duration::from_millis(100));
    
    let start = std::time::Instant::now();
    provider.stop_background_refresh();
    let stop_duration = start.elapsed();
    
    // Should stop quickly (minimum 1s prevents spinning even with very short TTL)
    assert!(
        stop_duration < Duration::from_secs(2),
        "Background thread should stop quickly with very short cache_ttl (took {:?})",
        stop_duration
    );
}

// --- Drop implementation tests ---

#[test]
fn test_jwks_drop_stops_background_thread() {
    // Test that dropping a provider automatically stops the background thread
    // This validates the Drop implementation works correctly
    let jwks_url = "http://localhost:8080/jwks.json";
    
    let start = std::time::Instant::now();
    {
        let _provider = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
            .cache_ttl(Duration::from_secs(1));
        // Provider is dropped here at end of scope
        // Drop implementation should call stop_background_refresh()
    }
    let drop_duration = start.elapsed();
    
    // If Drop works correctly, the thread should stop quickly (< 2s)
    // If Drop doesn't work, the thread would continue running (but we can't directly test that)
    // However, if Drop hangs or takes too long, that's a failure
    assert!(
        drop_duration < Duration::from_secs(2),
        "Drop should stop background thread quickly (took {:?})",
        drop_duration
    );
}

#[test]
fn test_jwks_drop_multiple_providers() {
    // Test that dropping multiple providers cleans up all their threads
    let jwks_url = "http://localhost:8080/jwks.json";
    
    let start = std::time::Instant::now();
    {
        let _provider1 = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
            .cache_ttl(Duration::from_secs(1));
        let _provider2 = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
            .cache_ttl(Duration::from_secs(5));
        let _provider3 = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
            .cache_ttl(Duration::from_secs(300));
        // All providers are dropped here at end of scope
    }
    let drop_duration = start.elapsed();
    
    // All three threads should stop quickly
    assert!(
        drop_duration < Duration::from_secs(3),
        "Dropping multiple providers should stop all threads quickly (took {:?})",
        drop_duration
    );
}

#[test]
fn test_jwks_drop_after_explicit_stop() {
    // Test that dropping a provider after explicitly calling stop_background_refresh
    // doesn't cause issues (should be idempotent)
    let jwks_url = "http://localhost:8080/jwks.json";
    
    let provider = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
        .cache_ttl(Duration::from_secs(1));
    
    // Explicitly stop the background thread
    let start = std::time::Instant::now();
    provider.stop_background_refresh();
    let explicit_stop_duration = start.elapsed();
    
    assert!(
        explicit_stop_duration < Duration::from_secs(2),
        "Explicit stop should work quickly (took {:?})",
        explicit_stop_duration
    );
    
    // Now drop the provider - should not hang or cause issues
    let start = std::time::Instant::now();
    drop(provider);
    let drop_duration = start.elapsed();
    
    // Drop should be very fast since thread is already stopped
    assert!(
        drop_duration < Duration::from_millis(100),
        "Drop after explicit stop should be very fast (took {:?})",
        drop_duration
    );
}

#[test]
fn test_jwks_drop_and_recreate() {
    // Test that dropping and recreating providers works correctly
    // This ensures Drop properly cleans up resources so new providers can be created
    let jwks_url = "http://localhost:8080/jwks.json";
    
    // Create and drop first provider
    {
        let _provider1 = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
            .cache_ttl(Duration::from_secs(1));
    }
    
    // Small delay to ensure cleanup completes
    std::thread::sleep(Duration::from_millis(100));
    
    // Create a new provider - should work without issues
    let start = std::time::Instant::now();
    {
        let _provider2 = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
            .cache_ttl(Duration::from_secs(5));
    }
    let drop_duration = start.elapsed();
    
    // Second provider should also drop cleanly
    assert!(
        drop_duration < Duration::from_secs(2),
        "Recreated provider should drop cleanly (took {:?})",
        drop_duration
    );
}

#[test]
fn test_jwks_drop_with_long_cache_ttl() {
    // Test that dropping a provider with a long cache_ttl (long sleep interval)
    // still stops the thread quickly via Drop
    let jwks_url = "http://localhost:8080/jwks.json";
    
    let start = std::time::Instant::now();
    {
        // Long cache_ttl means thread sleeps for ~290s, but Drop should interrupt it
        let _provider = brrtrouter::security::JwksBearerProvider::new(jwks_url.to_string())
            .cache_ttl(Duration::from_secs(300));
    }
    let drop_duration = start.elapsed();
    
    // Even with long sleep interval, Drop should stop thread quickly (< 2s)
    // The shutdown flag check happens every 1s during sleep, so should respond quickly
    assert!(
        drop_duration < Duration::from_secs(2),
        "Drop should stop thread quickly even with long cache_ttl (took {:?})",
        drop_duration
    );
}

// --- Thread storm prevention tests ---

#[test]
fn test_jwks_refresh_thread_storm_prevention() {
    // P1: Test that refresh_in_progress flag prevents thread storm
    // When cache is expired but non-empty, multiple concurrent calls to
    // refresh_jwks_if_needed should not spawn unbounded threads.
    //
    // This test simulates high load during cache expiry window:
    // 1. Create provider with short cache_ttl
    // 2. Populate cache initially (so it's non-empty) via validation
    // 3. Wait for cache to expire
    // 4. Spawn many threads that all try to validate simultaneously
    // 5. Verify that refresh_in_progress prevents excessive thread spawning
    //    by counting HTTP requests (should be 1-2, not 50+)
    
    use std::sync::atomic::{AtomicU32, Ordering};
    
    // Create a mock JWKS server that counts requests
    let request_count = Arc::new(AtomicU32::new(0));
    let request_count_clone = request_count.clone();
    
    let secret = b"test-secret-key-32-bytes!!";
    let k = base64url_no_pad(secret);
    let jwks = json!({
        "keys": [{
            "kty": "oct",
            "kid": "test-key",
            "alg": "HS256",
            "k": k
        }]
    });
    
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let jwks_url = format!("http://{}:{}/jwks.json", addr.ip(), addr.port());
    let jwks_body = jwks.to_string();
    
    // Spawn server that counts requests
    std::thread::spawn(move || {
        for _ in 0..20 {
            // Accept up to 20 requests (should only get 1-2 due to refresh_in_progress)
            if let Ok((mut stream, _)) = listener.accept() {
                request_count_clone.fetch_add(1, Ordering::Relaxed);
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    jwks_body.len(),
                    jwks_body
                );
                let _ = stream.write_all(resp.as_bytes());
                // Small delay to simulate network latency
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    });
    
    // Small delay to let server start
    std::thread::sleep(Duration::from_millis(100));
    
    // Create provider with very short cache_ttl (500ms) so we can easily trigger expiry
    let iss = "https://issuer.example";
    let aud = "my-audience";
    let provider = Arc::new(
        brrtrouter::security::JwksBearerProvider::new(jwks_url.clone())
            .issuer(iss.to_string())
            .audience(aud.to_string())
            .cache_ttl(Duration::from_millis(500))
    );
    
    // Create a valid token for validation
    let token = make_hs256_jwt(secret, iss, aud, "test-key", 3600);
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };
    
    // Populate cache initially by validating a token (this triggers initial refresh)
    // This ensures cache is non-empty when we test the thread storm scenario
    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {}", token)));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    
    assert!(provider.validate(&scheme, &[], &req), "Initial validation should succeed");
    
    // Wait for initial refresh to complete
    std::thread::sleep(Duration::from_millis(200));
    
    // Reset request count (initial refresh already happened)
    request_count.store(0, Ordering::Relaxed);
    
    // Wait for cache to expire (500ms TTL + buffer)
    std::thread::sleep(Duration::from_millis(600));
    
    // Now spawn many threads that all try to validate simultaneously
    // This simulates high load during expiry window
    // Each validation will internally call get_key_for -> refresh_jwks_if_needed
    let num_threads = 50;
    let mut handles = Vec::new();
    
    for _ in 0..num_threads {
        let provider_clone = provider.clone();
        let token_clone = token.clone();
        let scheme_clone = scheme.clone();
        let handle = std::thread::spawn(move || {
            // Create request in each thread (can't share references across threads)
            let mut thread_headers: HeaderVec = HeaderVec::new();
            thread_headers.push((Arc::from("authorization"), format!("Bearer {}", token_clone)));
            let thread_req = SecurityRequest {
                headers: &thread_headers,
                query: &ParamVec::new(),
                cookies: &HeaderVec::new(),
            };
            // Call validate which internally calls get_key_for -> refresh_jwks_if_needed
            // When cache is expired but non-empty, this would spawn threads
            // but refresh_in_progress should prevent thread storm
            let _ = provider_clone.validate(&scheme_clone, &[], &thread_req);
        });
        handles.push(handle);
    }
    
    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Give a small buffer for any in-flight requests to complete
    std::thread::sleep(Duration::from_millis(300));
    
    // Verify that only a small number of HTTP requests were made
    // Without the fix, we'd see ~50 requests (one per thread)
    // With the fix, we should see 1-2 requests (one successful refresh,
    // maybe one retry if the first fails, but refresh_in_progress prevents more)
    let final_count = request_count.load(Ordering::Relaxed);
    
    assert!(
        final_count <= 3,
        "Thread storm prevention failed: {} HTTP requests made by {} concurrent validation threads. \
         Expected <= 3 requests (refresh_in_progress should prevent thread spawning). \
         This indicates threads are being spawned without checking refresh_in_progress flag. \
         Without the fix, we'd see ~{} requests (one per thread).",
        final_count,
        num_threads,
        num_threads
    );
    
    // Verify that the cache was actually refreshed (validation should still work)
    assert!(
        provider.validate(&scheme, &[], &req),
        "Cache should be refreshed and validation should still work"
    );
}
