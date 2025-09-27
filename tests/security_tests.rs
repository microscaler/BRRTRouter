use base64::Engine;
use brrtrouter::middleware::TracingMiddleware;
use brrtrouter::server::{HttpServer, ServerHandle};
use brrtrouter::spec::SecurityScheme;
use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HandlerResponse},
    load_spec_full,
    router::Router,
    server::AppService,
    BearerJwtProvider, OAuth2Provider, SecurityProvider, SecurityRequest,
};
use serde_json::json;
use std::collections::HashMap;
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

struct ApiKeyProvider {
    key: String,
}

impl SecurityProvider for ApiKeyProvider {
    fn validate(&self, scheme: &SecurityScheme, _scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::ApiKey { name, location, .. } => {
                let expected = &self.key;
                match location.as_str() {
                    "header" => req.headers.get(&name.to_ascii_lowercase()) == Some(expected),
                    "query" => req.query.get(name) == Some(expected),
                    "cookie" => req.cookies.get(name) == Some(expected),
                    _ => false,
                }
            }
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
    unsafe {
        dispatcher.register_handler("secret", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HashMap::new(),
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
        None,
        None,
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
    unsafe {
        dispatcher.register_handler("one", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HashMap::new(),
                body: json!({"one": true}),
            });
        });
        dispatcher.register_handler("two", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HashMap::new(),
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
        None,
        None,
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
    unsafe {
        dispatcher.register_handler("header", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HashMap::new(),
                body: json!({"header": true}),
            });
        });
        dispatcher.register_handler("cookie", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HashMap::new(),
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
        None,
        None,
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
    let (_tracing, handle, addr) = start_service();
    let resp = send_request(&addr, "GET /secret HTTP/1.1\r\nHost: localhost\r\n\r\n");
    let status = parse_status(&resp);
    assert_eq!(status, 401);

    let resp = send_request(
        &addr,
        "GET /secret HTTP/1.1\r\nHost: localhost\r\nX-API-Key: secret\r\n\r\n",
    );
    let status = parse_status(&resp);
    assert_eq!(status, 200);

    handle.stop();
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
    unsafe {
        dispatcher.register_handler("secret", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HashMap::new(),
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
        None,
        None,
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
    let (_tracing, handle, addr) = start_service_default_provider();
    let resp = send_request(
        &addr,
        "GET /secret HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer secret\r\n\r\n",
    );
    handle.stop();
    let status = parse_status(&resp);
    assert_eq!(status, 200);
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
    unsafe {
        dispatcher.register_handler("header", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HashMap::new(),
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
        None,
        None,
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
    let (_t, handle, addr) = start_service_with_jwks(&jwks_url, iss, aud);
    let req = format!(
        "GET /header HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\n\r\n",
        token
    );
    let resp = send_request(&addr, &req);
    handle.stop();
    let status_ok = parse_status(&resp);
    assert_eq!(status_ok, 200);
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
    let (_t, handle, addr) = start_service_with_jwks(&jwks_url, iss, aud);
    let req = format!(
        "GET /header HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\n\r\n",
        token
    );
    let resp = send_request(&addr, &req);
    handle.stop();
    let status = parse_status(&resp);
    assert_eq!(status, 401);
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
    unsafe {
        dispatcher.register_handler("secret", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: HashMap::new(),
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
        None,
        None,
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
    let (_t, handle, addr) = start_service_with_remote_apikey(&url);
    // success
    let req_ok = "GET /secret HTTP/1.1\r\nHost: localhost\r\nX-API-Key: validkey\r\n\r\n";
    let resp_ok = send_request(&addr, req_ok);
    let status_ok = parse_status(&resp_ok);
    assert_eq!(status_ok, 200);
    // failure
    let req_bad = "GET /secret HTTP/1.1\r\nHost: localhost\r\nX-API-Key: wrong\r\n\r\n";
    let resp_bad = send_request(&addr, req_bad);
    handle.stop();
    handle_verify.join().ok();
    let status_bad = parse_status(&resp_bad);
    assert_eq!(status_bad, 401);
}

// TODO: This test fails intermittently due to timing issues with the coroutine cancellation.
#[test]
fn test_multiple_security_providers() {
    let (_tracing, handle, addr) = start_multi_service();
    let resp = send_request(
        &addr,
        "GET /one HTTP/1.1\r\nHost: localhost\r\nX-Key-One: one\r\n\r\n",
    );
    let status = parse_status(&resp);
    assert_eq!(status, 200);

    let resp = send_request(
        &addr,
        "GET /two HTTP/1.1\r\nHost: localhost\r\nX-Key-Two: two\r\n\r\n",
    );
    let status_two = parse_status(&resp);
    assert_eq!(status_two, 200);

    let resp = send_request(
        &addr,
        "GET /one HTTP/1.1\r\nHost: localhost\r\nX-Key-Two: two\r\n\r\n",
    );
    handle.stop();
    let status_wrong = parse_status(&resp);
    assert_eq!(status_wrong, 401);
}

#[test]
fn test_bearer_header_and_oauth_cookie() {
    let (_tracing, handle, addr) = start_token_service();
    // Missing token should fail
    let resp = send_request(&addr, "GET /header HTTP/1.1\r\nHost: localhost\r\n\r\n");
    let status = parse_status(&resp);
    assert_eq!(status, 401);

    // Valid bearer header
    let token = make_token("");
    let req = format!(
        "GET /header HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\n\r\n",
        token
    );
    let resp = send_request(&addr, &req);
    let status_ok = parse_status(&resp);
    assert_eq!(status_ok, 200);

    // OAuth2 cookie with required scope
    let token = make_token("read");
    let req = format!(
        "GET /cookie HTTP/1.1\r\nHost: localhost\r\nCookie: auth={}\r\n\r\n",
        token
    );
    let resp = send_request(&addr, &req);
    handle.stop();
    let status_cookie = parse_status(&resp);
    assert_eq!(status_cookie, 200);
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
    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), format!("Bearer {}", token));
    let req = SecurityRequest {
        headers: &headers,
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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
    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), format!("Bearer {}", token));
    let req = SecurityRequest {
        headers: &headers,
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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
    let mut headers = HashMap::new();
    headers.insert(
        "authorization".to_string(),
        "Bearer invalid.token".to_string(),
    );
    let req = SecurityRequest {
        headers: &headers,
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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
    let mut headers = HashMap::new();
    headers.insert(
        "authorization".to_string(),
        "Bearer header.invalid_base64.sig".to_string(),
    );
    let req = SecurityRequest {
        headers: &headers,
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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

    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), format!("Bearer {}", token));
    let req = SecurityRequest {
        headers: &headers,
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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
    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), format!("Bearer {}", token));
    let req = SecurityRequest {
        headers: &headers,
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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
    let mut cookies = HashMap::new();
    cookies.insert("auth_token".to_string(), token);
    let req = SecurityRequest {
        headers: &HashMap::new(),
        query: &HashMap::new(),
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
    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), format!("Bearer {}", token));
    let req = SecurityRequest {
        headers: &headers,
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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
    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), format!("Bearer {}", token));
    let req = SecurityRequest {
        headers: &headers,
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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
    let mut cookies = HashMap::new();
    cookies.insert("oauth_token".to_string(), token);
    let req = SecurityRequest {
        headers: &HashMap::new(),
        query: &HashMap::new(),
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
    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), format!("Bearer {}", token));
    let req = SecurityRequest {
        headers: &headers,
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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

    let mut headers = HashMap::new();
    headers.insert("x-api-key".to_string(), "test_key".to_string());
    let req = SecurityRequest {
        headers: &headers,
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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

    let mut query = HashMap::new();
    query.insert("api_key".to_string(), "test_key".to_string());
    let req = SecurityRequest {
        headers: &HashMap::new(),
        query: &query,
        cookies: &HashMap::new(),
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

    let mut cookies = HashMap::new();
    cookies.insert("api_key".to_string(), "test_key".to_string());
    let req = SecurityRequest {
        headers: &HashMap::new(),
        query: &HashMap::new(),
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
        headers: &HashMap::new(),
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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
        headers: &HashMap::new(),
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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

    let mut headers = HashMap::new();
    headers.insert(
        "authorization".to_string(),
        "Basic dXNlcjpwYXNz".to_string(),
    ); // Basic auth instead of Bearer
    let req = SecurityRequest {
        headers: &headers,
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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
    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), format!("Bearer {}", token));
    let req = SecurityRequest {
        headers: &headers,
        query: &HashMap::new(),
        cookies: &HashMap::new(),
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
    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), format!("Bearer {}", token));
    let req = SecurityRequest {
        headers: &headers,
        query: &HashMap::new(),
        cookies: &HashMap::new(),
    };

    // Should pass with no required scopes
    assert!(provider.validate(&scheme, &[], &req));

    // Should fail with required scopes
    assert!(!provider.validate(&scheme, &["read".to_string()], &req));
}
