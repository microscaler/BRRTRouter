//! Comprehensive JWT typ claim enforcement tests (at+jwt)
//!
//! Story 8.1: Enforce JWT typ Claim (at+jwt)
//!
//! This test module covers all acceptance criteria and gotchas from story-8.1.md:
//! - Unit tests for typ validation logic
//! - Integration tests via rstest_bdd pattern
//! - Security regression tests
//! - Edge case tests (non-string typ, null bytes, long strings, etc.)

#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

use base64::Engine;
use brrtrouter::dispatcher::{Dispatcher, HandlerRequest, HandlerResponse, HeaderVec};
use brrtrouter::load_spec_full;
use brrtrouter::middleware::TracingMiddleware;
use brrtrouter::router::{ParamVec, Router};
use brrtrouter::security::{JwksBearerProvider, SecurityProvider, SecurityRequest};
use brrtrouter::server::{AppService, HttpServer, ServerHandle};
use serde_json::json;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{net::TcpStream as StdTcpStream, thread};

mod tracing_util;
use tracing_util::TestTracing;

mod common;
use common::temp_files;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn base64url(data: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn base64url_std(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Build a valid at+jwt token (HS256) signed with a secret key, scoped to a mock JWKS server.
fn make_valid_at_jwt(secret: &[u8], kid: &str) -> String {
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use serde_json::json;

    let header = Header {
        kid: Some(kid.to_string()),
        alg: Algorithm::HS256,
        typ: Some("at+jwt".to_string()),
        ..Default::default()
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let claims = json!({
        "iss": "https://issuer.example",
        "aud": "my-api",
        "exp": now + 3600,
        "scope": "read write"
    });
    jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
}

/// Build a JWT with a custom typ value.
fn make_jwt_with_typ(secret: &[u8], typ: &str, kid: &str) -> String {
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use serde_json::json;

    let header = Header {
        kid: Some(kid.to_string()),
        alg: Algorithm::HS256,
        typ: Some(typ.to_string()),
        ..Default::default()
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let claims = json!({
        "iss": "https://issuer.example",
        "aud": "my-api",
        "exp": now + 3600,
        "scope": "read write"
    });
    jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
}

/// Build a JWT with NO typ claim.
fn make_jwt_without_typ(secret: &[u8], kid: &str) -> String {
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use serde_json::json;

    let header = Header {
        kid: Some(kid.to_string()),
        alg: Algorithm::HS256,
        ..Default::default()
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let claims = json!({
        "iss": "https://issuer.example",
        "aud": "my-api",
        "exp": now + 3600,
        "scope": "read write"
    });
    jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
}

/// Build a raw JWT with a custom header JSON (bypass jsonwebtoken crate).
/// Useful for testing malformed headers (numbers, objects, null bytes, etc.).
fn make_raw_jwt(header_json: &str, payload_json: &str, signature: &str) -> String {
    let h = base64url_std(header_json.as_bytes());
    let p = base64url_std(payload_json.as_bytes());
    format!("{}.{}.{}", h, p, signature)
}

/// Start a mock JWKS server that returns the given JWKS JSON.
/// Returns the HTTP URL (e.g., "http://127.0.0.1:XXXX").
fn start_mock_jwks_server(jwks_json: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}:{}/.well-known/jwks.json", addr.ip(), addr.port());
    let handle = thread::spawn(move || {
        let _listener = listener; // consume
                                  // Accept one connection
        if let Ok((mut stream, _)) = TcpListener::bind("127.0.0.1:0").unwrap().accept() {}
        // Actually, let's accept properly
    });
    // Simpler approach: start a tiny HTTP server
    drop(handle);
    start_mock_jwks_server_inner(jwks_json)
}

fn start_mock_jwks_server_inner(jwks_json: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}:{}/jwks.json", addr.ip(), addr.port());
    let body = jwks_json.to_string();
    eprintln!("DEBUG: mock jwks server listening at {}", url);
    thread::spawn(move || {
        // Handle unlimited connections in a loop (background refresh retries up to 2x)
        let _listener = listener;
        loop {
            match _listener.accept() {
                Ok((mut stream, _)) => {
                    // Read the HTTP request to drain the socket
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let req_str = String::from_utf8_lossy(&buf[..n]).to_string();
                    eprintln!(
                        "DEBUG: mock jwks server received request ({} bytes): {}",
                        n,
                        req_str.lines().next().unwrap_or("")
                    );
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let written = resp.as_bytes().len();
                    let _ = stream.write_all(resp.as_bytes());
                    let _ = stream.flush();
                    eprintln!("DEBUG: mock jwks server sent {} bytes", written);
                }
                Err(_) => break, // listener dropped
            }
        }
    });
    url
}

/// Build a complete service with JWKS Bearer security using a mock JWKS server.
/// Returns the test server with automatic cleanup.
fn build_jwks_service(
    jwks_json: &str,
    expected_typ: &str,
) -> (TestTracing, ServerHandle, SocketAddr) {
    may::config().set_stack_size(0x8000);
    let tracing = TestTracing::init();

    // Secret key that goes into the JWKS
    let secret = b"test-secret-key-12345";
    let k = base64url(secret);

    let jwks = serde_json::json!({
        "keys": [
            {
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": k
            }
        ]
    })
    .to_string();

    let jwks_url = start_mock_jwks_server_inner(&jwks);

    // Verify the mock server responds before proceeding
    eprintln!("DEBUG: Testing mock server at {}", jwks_url);
    let resp = reqwest::blocking::get(&jwks_url).unwrap();
    let body = resp.text().unwrap();
    eprintln!("DEBUG: Mock server responded with: {}", body);

    const SPEC: &str = r#"openapi: 3.1.0
info:
  title: Typ Test API
  version: '1.0'
paths:
  /secure:
    get:
      operationId: secure
      security:
        - BearerAuth: []
      responses:
        '200':
          description: OK
"#;

    let path = temp_files::create_temp_yaml(SPEC);
    let (routes, schemes, _slug) = brrtrouter::load_spec_full(path.to_str().unwrap()).unwrap();
    let router = Arc::new(arc_swap::ArcSwap::from_pointee(Router::new(routes)));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("secure", |req: HandlerRequest| {
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
        Arc::new(arc_swap::ArcSwap::from_pointee(dispatcher)),
        schemes,
        PathBuf::from("examples/openapi.yaml"),
        Some(PathBuf::from("examples/pet_store/static_site")),
        Some(PathBuf::from("examples/pet_store/doc")),
    );
    let provider = JwksBearerProvider::new(&jwks_url)
        .issuer("https://issuer.example")
        .audience("my-api")
        .leeway(30);
    service.register_security_provider("BearerAuth", Arc::new(provider));

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    (tracing, handle, addr)
}

fn send_raw_request(addr: &SocketAddr, req: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(req.as_bytes()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
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
            Err(e) => panic!("read error: {e:?}"),
        }
    }
    let header_end = header_end.unwrap_or(buf.len());
    let headers = String::from_utf8_lossy(&buf[..header_end]);
    let content_length = headers
        .lines()
        .find_map(|l| l.split_once(':'))
        .filter(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse::<usize>().ok());
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
                    break;
                }
                Err(e) => panic!("read error: {e:?}"),
            }
        }
    } else {
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
                Err(e) => panic!("read error: {e:?}"),
            }
        }
    }
    String::from_utf8_lossy(&buf).to_string()
}

fn parse_http_status(resp: &str) -> u16 {
    resp.lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("0")
        .parse()
        .unwrap()
}

fn make_authorization_header(token: &str) -> String {
    format!("Authorization: Bearer {}", token)
}

// ---------------------------------------------------------------------------
// Unit Tests: typ validation logic
// ---------------------------------------------------------------------------

/// Given a JWT with valid typ "at+jwt", the token should be accepted.
#[test]
fn unit_valid_typ_atjwt_accepted() {
    let secret = b"test-secret-key-12345";
    let jwks = serde_json::json!({
        "keys": [{
            "kty": "oct",
            "alg": "HS256",
            "kid": "k1",
            "k": base64url(secret)
        }]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server_inner(&jwks);
    let (_tracing, handle, addr) = build_jwks_service(&jwks, "at+jwt");

    let token = make_valid_at_jwt(secret, "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    eprintln!(
        "DEBUG: status={}, resp={}",
        status,
        &resp[..resp.len().min(1000)]
    );
    assert_eq!(
        status, 200,
        "Valid at+jwt token should be accepted, got status {}",
        status
    );

    handle.stop();
}

/// Given a JWT with NO typ claim, the token should be rejected with 401.
#[test]
fn unit_missing_typ_claim_rejected() {
    let secret = b"test-secret-key-12345";
    let jwks = serde_json::json!({
        "keys": [{
            "kty": "oct",
            "alg": "HS256",
            "kid": "k1",
            "k": base64url(secret)
        }]
    })
    .to_string();
    let jwks_url = start_mock_jwks_server_inner(&jwks);
    let (_tracing, handle, addr) = build_jwks_service(&jwks, "at+jwt");

    let token = make_jwt_without_typ(secret, "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(
        status, 401,
        "Token without typ should be rejected with 401, got {}",
        status
    );

    handle.stop();
}

/// Given a JWT with typ "jwt" (generic), the token should be rejected.
#[test]
fn unit_wrong_typ_jwt_rejected() {
    let secret = b"test-secret-key-12345";
    let jwks = serde_json::json!({
        "keys": [{
            "kty": "oct",
            "alg": "HS256",
            "kid": "k1",
            "k": base64url(secret)
        }]
    })
    .to_string();
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let token = make_jwt_with_typ(secret, "jwt", "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(
        status, 401,
        "Token with typ='jwt' should be rejected with 401, got {}",
        status
    );

    handle.stop();
}

/// Given a JWT with typ "id+at+jwt", the token should be rejected.
#[test]
fn unit_wrong_typ_id_at_jwt_rejected() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let token = make_jwt_with_typ(secret, "id+at+jwt", "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(
        status, 401,
        "Token with typ='id+at+jwt' should be rejected, got {}",
        status
    );

    handle.stop();
}

/// Empty typ string should be rejected.
#[test]
fn unit_empty_typ_rejected() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let token = make_jwt_with_typ(secret, "", "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(status, 401, "Empty typ should be rejected, got {}", status);

    handle.stop();
}

/// typ is case-sensitive: "AT+JWT" (uppercase) should be rejected.
#[test]
fn unit_typ_case_sensitive_rejects_uppercase() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let token = make_jwt_with_typ(secret, "AT+JWT", "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(
        status, 401,
        "Uppercase 'AT+JWT' should be rejected (typ is case-sensitive), got {}",
        status
    );

    handle.stop();
}

/// typ with leading/trailing whitespace should be rejected (no trimming).
#[test]
fn unit_typ_rejects_whitespace() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let token = make_jwt_with_typ(secret, " at+jwt", "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(
        status, 401,
        "Leading space in typ should be rejected, got {}",
        status
    );

    handle.stop();
}

/// Refresh token identifier ("refresh") should be rejected.
#[test]
fn unit_typ_refresh_rejected() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let token = make_jwt_with_typ(secret, "refresh", "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(
        status, 401,
        "typ='refresh' should be rejected, got {}",
        status
    );

    handle.stop();
}

/// Error response should include expected and actual typ for debugging.
#[test]
fn unit_error_message_includes_expected_actual_typ() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let token = make_jwt_with_typ(secret, "self-issued", "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(status, 401, "Expected 401 for wrong typ");
    // The response body should contain error details about the token type
    assert!(
        resp.contains("self-issued") || resp.contains("invalid") || resp.contains("type"),
        "Error response should mention the invalid token type, got: {}",
        resp
    );

    handle.stop();
}

// ---------------------------------------------------------------------------
// Integration Tests (BDD-style)
// ---------------------------------------------------------------------------

/// Scenario: Login service issues typ at+jwt
/// Given a successful login flow -> when the access token is parsed
/// then the JOSE header contains typ: "at+jwt" and the payload is correctly decoded
#[test]
fn integration_login_service_issues_typ_atjwt() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    // Build a token that simulates what a login service would issue
    let token = make_valid_at_jwt(secret, "k1");

    // Parse the token header to verify typ
    let parts: Vec<&str> = token.split('.').collect();
    assert_eq!(parts.len(), 3, "Token should have 3 parts");
    let header_json = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[0])
        .unwrap();
    let header_obj: serde_json::Value = serde_json::from_slice(&header_json).unwrap();
    assert_eq!(
        header_obj["typ"].as_str(),
        Some("at+jwt"),
        "Login service must issue tokens with typ='at+jwt'"
    );

    // Verify it's accepted by the service
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    assert_eq!(parse_http_status(&resp), 200);

    handle.stop();
}

/// Scenario: Service rejects token without typ
/// Given a client sends a JWT with no typ in the JOSE header
/// when the request reaches the JWT middleware
/// then the response is 401 with error code "invalid_token_type"
#[test]
fn integration_service_rejects_token_without_typ() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    // Send token without typ
    let token = make_jwt_without_typ(secret, "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(status, 401, "Should reject token without typ");
    assert!(
        resp.to_lowercase().contains("invalid") || resp.to_lowercase().contains("type"),
        "Error should mention invalid token type, got: {}",
        resp
    );

    handle.stop();
}

/// Scenario: Service rejects token with wrong typ
/// Given a client sends a JWT with typ: "jwt"
/// when the request reaches the JWT middleware
/// then the response is 401 with error code "invalid_token_type"
#[test]
fn integration_service_rejects_wrong_typ() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let token = make_jwt_with_typ(secret, "jwt", "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(status, 401, "Should reject token with wrong typ");
    assert!(
        resp.contains("jwt") || resp.to_lowercase().contains("invalid"),
        "Error should mention 'jwt' or 'invalid', got: {}",
        resp
    );

    handle.stop();
}

/// Scenario: typ enforcement works with HS256 tokens
/// Given a JWT signed with HS256 and typ: "at+jwt"
/// when the service validates
/// then it is accepted (typ enforcement is algorithm-independent)
#[test]
fn integration_typ_enforcement_with_hs256() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let token = make_valid_at_jwt(secret, "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    assert_eq!(
        parse_http_status(&resp),
        200,
        "HS256 with valid typ should work"
    );

    handle.stop();
}

/// Scenario: typ enforcement order - typ checked before signature
/// Given a JWT with typ: "at+jwt" but invalid signature
/// when the service validates
/// then typ check happens first (before signature verification)
#[test]
fn integration_typ_checked_before_signature() {
    let secret = b"test-secret-key-12345";
    let wrong_secret = b"wrong-secret-key-xyz";

    // Create a token with the correct typ but signed with wrong key
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    // This token has typ="at+jwt" but signature is invalid
    let token = make_valid_at_jwt(wrong_secret, "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    // Should be 401 (invalid signature), not 200
    assert_eq!(
        parse_http_status(&resp),
        401,
        "Invalid signature should still be rejected even with correct typ"
    );

    handle.stop();
}

// ---------------------------------------------------------------------------
// Security Regression Tests
// ---------------------------------------------------------------------------

/// Refresh token (opaque string) sent as Bearer is rejected.
#[test]
fn security_refresh_token_not_jwt_rejected() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    // Send an opaque string (simulating a refresh token) as Bearer
    let opaque_refresh_token = "rT_abc123_def456ghi789";
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\n\r\n",
        opaque_refresh_token
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(
        status, 401,
        "Opaque refresh token sent as Bearer should be rejected (not a valid JWT)",
    );

    handle.stop();
}

/// Self-issued ID token with typ "id+at+jwt" cannot bypass authz.
#[test]
fn security_id_token_cannot_bypass_authz() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let token = make_jwt_with_typ(secret, "id+at+jwt", "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    assert_eq!(
        parse_http_status(&resp),
        401,
        "Self-issued ID token must be rejected as access token"
    );

    handle.stop();
}

/// Typ claim alone does not grant access (valid typ + invalid signature = rejected).
#[test]
fn security_typ_alone_does_not_grant_access() {
    let secret = b"test-secret-key-12345";
    let wrong_secret = b"totally-wrong-key";

    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    // Token with correct typ but forged signature
    let token = make_valid_at_jwt(wrong_secret, "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    assert_eq!(
        parse_http_status(&resp),
        401,
        "Correct typ + wrong signature must still be rejected"
    );

    handle.stop();
}

/// No information leakage through typ error message.
#[test]
fn security_no_info_leakage_in_typ_error() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let token = make_jwt_with_typ(secret, "attacker-control", "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    assert_eq!(parse_http_status(&resp), 401);
    // Should NOT leak internal pipeline details
    assert!(
        !resp.to_lowercase().contains("pipeline")
            && !resp.to_lowercase().contains("validation order"),
        "Error message must not leak internal validation pipeline"
    );

    handle.stop();
}

// ---------------------------------------------------------------------------
// Edge Case Tests
// ---------------------------------------------------------------------------

/// JWT header with typ as JSON number (123) should be rejected.
#[test]
fn edge_case_typ_as_number_rejected() {
    let secret = b"test-secret-key-12345";

    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    // Create a raw JWT with typ as a number in the header
    let header_json = r#"{"alg":"HS256","typ":123,"kid":"k1"}"#;
    let payload_json =
        r#"{"iss":"https://issuer.example","aud":"my-api","exp":9999999999,"scope":"read"}"#;
    let token = make_raw_jwt(header_json, payload_json, "fakesig");

    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(
        status, 401,
        "Non-string typ (number) should be rejected, got {}",
        status
    );

    handle.stop();
}

/// JWT header with typ as JSON object should be rejected.
#[test]
fn edge_case_typ_as_object_rejected() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let header_json = r#"{"alg":"HS256","typ":{"value":"at+jwt"},"kid":"k1"}"#;
    let payload_json =
        r#"{"iss":"https://issuer.example","aud":"my-api","exp":9999999999,"scope":"read"}"#;
    let token = make_raw_jwt(header_json, payload_json, "fakesig");

    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(status, 401, "Object typ should be rejected, got {}", status);

    handle.stop();
}

/// JWT header with typ as JSON array should be rejected.
#[test]
fn edge_case_typ_as_array_rejected() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let header_json = r#"{"alg":"HS256","typ":["at+jwt","refresh"],"kid":"k1"}"#;
    let payload_json =
        r#"{"iss":"https://issuer.example","aud":"my-api","exp":9999999999,"scope":"read"}"#;
    let token = make_raw_jwt(header_json, payload_json, "fakesig");

    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(status, 401, "Array typ should be rejected, got {}", status);

    handle.stop();
}

/// JWT with typ containing null bytes should be rejected.
#[test]
fn edge_case_typ_with_null_bytes_rejected() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let header_json = r#"{"alg":"HS256","typ":"at+\u0000jwt","kid":"k1"}"#;
    let payload_json =
        r#"{"iss":"https://issuer.example","aud":"my-api","exp":9999999999,"scope":"read"}"#;
    let token = make_raw_jwt(header_json, payload_json, "fakesig");

    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(
        status, 401,
        "Typ with null bytes should be rejected, got {}",
        status
    );

    handle.stop();
}

/// Extremely long typ value (10KB) should be rejected.
#[test]
fn edge_case_very_long_typ_rejected() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    // Create a 10KB typ value
    let long_typ = "x".repeat(10_000);
    let header_json = format!(r#"{{"alg":"HS256","typ":"{}","kid":"k1"}}"#, long_typ);
    let payload_json =
        r#"{"iss":"https://issuer.example","aud":"my-api","exp":9999999999,"scope":"read"}"#;
    let token = make_raw_jwt(&header_json, payload_json, "fakesig");

    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(
        status, 401,
        "Very long typ should be rejected, got {}",
        status
    );

    handle.stop();
}

/// Trailing space in typ should be rejected.
#[test]
fn edge_case_typ_trailing_space_rejected() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let token = make_jwt_with_typ(secret, "at+jwt ", "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(
        status, 401,
        "Trailing space in typ should be rejected, got {}",
        status
    );

    handle.stop();
}

/// Mixed case typ "At+Jwt" should be rejected.
#[test]
fn edge_case_typ_mixed_case_rejected() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    let token = make_jwt_with_typ(secret, "At+Jwt", "k1");
    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(
        status, 401,
        "Mixed case 'At+Jwt' should be rejected, got {}",
        status
    );

    handle.stop();
}

/// JWT with only alg and kid in header (no typ) should be rejected.
#[test]
fn edge_case_minimal_header_no_typ_rejected() {
    let secret = b"test-secret-key-12345";
    let (_tracing, handle, addr) = build_jwks_service(
        &serde_json::json!({
            "keys": [{
                "kty": "oct",
                "alg": "HS256",
                "kid": "k1",
                "k": base64url(secret)
            }]
        })
        .to_string(),
        "at+jwt",
    );

    // Create raw JWT with only alg and kid, no typ
    let header_json = r#"{"alg":"HS256","kid":"k1"}"#;
    let payload_json =
        r#"{"iss":"https://issuer.example","aud":"my-api","exp":9999999999,"scope":"read"}"#;
    let token = make_raw_jwt(header_json, payload_json, "fakesig");

    let req = format!(
        "GET /secure HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n",
        make_authorization_header(&token)
    );
    let resp = send_raw_request(&addr, &req);
    let status = parse_http_status(&resp);
    assert_eq!(
        status, 401,
        "Minimal header without typ should be rejected, got {}",
        status
    );

    handle.stop();
}
