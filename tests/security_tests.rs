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
use std::time::Duration;
mod tracing_util;
use tracing_util::TestTracing;

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

fn write_temp(content: &str) -> std::path::PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("sec_spec_{}_{}.yaml", std::process::id(), nanos));
    std::fs::write(&path, content).unwrap();
    path
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
    let path = write_temp(SPEC);
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
    let path = write_temp(SPEC);
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
    let path = write_temp(SPEC);
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
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let mut buf = Vec::new();
    loop {
        let mut tmp = [0u8; 1024];
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                break
            }
            Err(e) => panic!("read error: {:?}", e),
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
    let (mut tracing, handle, addr) = start_service();
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
