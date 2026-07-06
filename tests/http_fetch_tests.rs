//! Integration tests for `brrtrouter::http` fetch helpers and security-provider wiring.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use brrtrouter::dispatcher::{HeaderVec, ParamVec};
use brrtrouter::http::{fetch_get, fetch_get_text_with_retry, HttpFetchOptions};
use brrtrouter::security::{RemoteApiKeyProvider, SecurityProvider, SecurityRequest};
use brrtrouter::spec::SecurityScheme;

fn read_request(stream: &mut TcpStream) -> String {
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).unwrap_or(0);
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

fn write_response(stream: &mut TcpStream, status: u16, body: &str) {
    let reason = match status {
        200..=299 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes()).unwrap();
}

/// Bind an ephemeral port and serve `responses` in order (last response repeats).
fn start_sequential_server(responses: Vec<(u16, &'static str)>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{}:{}/resource", addr.ip(), addr.port());
    let handle = thread::spawn(move || {
        let mut index = 0usize;
        for incoming in listener.incoming() {
            let Ok((mut stream, _)) = incoming else {
                break;
            };
            let _req = read_request(&mut stream);
            let (status, body) = responses
                .get(index)
                .copied()
                .unwrap_or_else(|| *responses.last().unwrap_or(&(200, "")));
            if index + 1 < responses.len() {
                index += 1;
            }
            write_response(&mut stream, status, body);
        }
    });
    (base, handle)
}

/// Server that validates `X-API-Key: validkey` like the remote API key mock in security_tests.
fn start_api_key_verify_server(max_connections: usize) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}:{}/verify", addr.ip(), addr.port());
    let handle = thread::spawn(move || {
        for incoming in listener.incoming().take(max_connections) {
            let Ok((mut stream, _)) = incoming else {
                break;
            };
            let req = read_request(&mut stream);
            let ok = req.to_ascii_lowercase().contains("x-api-key: validkey");
            let status = if ok { 200 } else { 401 };
            write_response(&mut stream, status, "");
        }
    });
    (url, handle)
}

#[test]
fn fetch_get_http_returns_json_body() {
    let body = r#"{"keys":[]}"#;
    let (url, handle) = start_sequential_server(vec![(200, body)]);
    let options = HttpFetchOptions {
        timeout: Duration::from_secs(2),
        max_body_bytes: 4096,
        extra_headers: Vec::new(),
    };

    let (status, bytes) = fetch_get(&url, &options).unwrap();
    assert_eq!(status, 200);
    assert_eq!(std::str::from_utf8(&bytes).unwrap(), body);

    drop(handle);
}

#[test]
fn fetch_get_http_rejects_oversize_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}:{}/big", addr.ip(), addr.port());
    let body = "x".repeat(5000);
    let server = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = read_request(&mut stream);
            write_response(&mut stream, 200, &body);
        }
    });

    let options = HttpFetchOptions {
        timeout: Duration::from_secs(2),
        max_body_bytes: 128,
        extra_headers: Vec::new(),
    };
    assert!(fetch_get(&url, &options).is_err());
    server.join().ok();
}

#[test]
fn fetch_get_http_sends_extra_headers() {
    let seen = Arc::new(std::sync::Mutex::new(String::new()));
    let seen_clone = Arc::clone(&seen);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}:{}/hdr", addr.ip(), addr.port());
    let server = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let req = read_request(&mut stream);
            *seen_clone.lock().unwrap() = req;
            write_response(&mut stream, 200, "ok");
        }
    });

    let options = HttpFetchOptions {
        timeout: Duration::from_secs(2),
        max_body_bytes: 1024,
        extra_headers: vec![("X-API-Key".to_string(), "validkey".to_string())],
    };
    let (status, bytes) = fetch_get(&url, &options).unwrap();
    assert_eq!(status, 200);
    assert_eq!(bytes, b"ok");
    let req = seen.lock().unwrap().clone();
    assert!(
        req.to_ascii_lowercase().contains("x-api-key: validkey"),
        "expected header in request: {req}"
    );
    server.join().ok();
}

#[test]
fn fetch_get_text_with_retry_succeeds_after_transient_failure() {
    let (url, handle) = start_sequential_server(vec![
        (503, "unavailable"),
        (200, r#"{"keys":[]}"#),
    ]);
    let options = HttpFetchOptions {
        timeout: Duration::from_secs(2),
        max_body_bytes: 4096,
        extra_headers: Vec::new(),
    };

    let text = fetch_get_text_with_retry(&url, &options, 2);
    assert_eq!(text.as_deref(), Some(r#"{"keys":[]}"#));
    drop(handle);
}

#[test]
fn fetch_get_text_with_retry_returns_none_when_all_attempts_fail() {
    let (url, handle) = start_sequential_server(vec![(404, "missing"), (500, "error")]);
    let options = HttpFetchOptions {
        timeout: Duration::from_secs(2),
        max_body_bytes: 4096,
        extra_headers: Vec::new(),
    };

    assert!(fetch_get_text_with_retry(&url, &options, 2).is_none());
    drop(handle);
}

#[test]
fn remote_api_key_provider_validates_via_http_fetch() {
    let (verify_url, handle) = start_api_key_verify_server(2);
    let provider = RemoteApiKeyProvider::new(verify_url)
        .timeout_ms(2000)
        .cache_ttl(Duration::from_millis(1))
        .header_name("X-API-Key");

    let scheme = SecurityScheme::ApiKey {
        name: "X-API-Key".to_string(),
        location: "header".to_string(),
        description: None,
    };

    let mut ok_headers: HeaderVec = HeaderVec::new();
    ok_headers.push((Arc::from("x-api-key"), "validkey".to_string()));
    let ok_req = SecurityRequest {
        headers: &ok_headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    assert!(provider.validate(&scheme, &[], &ok_req));

    let mut bad_headers: HeaderVec = HeaderVec::new();
    bad_headers.push((Arc::from("x-api-key"), "wrong".to_string()));
    let bad_req = SecurityRequest {
        headers: &bad_headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    assert!(!provider.validate(&scheme, &[], &bad_req));

    handle.join().ok();
}

#[test]
fn fetch_get_jwks_shaped_document_via_retry_helper() {
    let jwks = r#"{"keys":[{"kty":"oct","kid":"k1","k":"abc","alg":"HS256"}]}"#;
    let (url, handle) = start_sequential_server(vec![(200, jwks)]);
    let options = HttpFetchOptions {
        timeout: Duration::from_millis(500),
        max_body_bytes: 256 * 1024,
        extra_headers: Vec::new(),
    };

    let body = fetch_get_text_with_retry(&url, &options, 2).expect("jwks body");
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(parsed.get("keys").and_then(|k| k.as_array()).is_some_and(|a| !a.is_empty()));
    drop(handle);
}

#[test]
fn jwks_bearer_provider_loads_keys_via_http_fetch() {
    use base64::Engine;
    use brrtrouter::security::JwksBearerProvider;

    let secret = b"http-fetch-jwks-secret";
    let kid = "fetch-kid";
    let k = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(secret);
    let jwks = format!(
        r#"{{"keys":[{{"kty":"oct","kid":"{kid}","k":"{k}","alg":"HS256"}}]}}"#
    );

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let jwks_url = format!("http://{}:{}/jwks.json", addr.ip(), addr.port());
    let server = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = read_request(&mut stream);
            write_response(&mut stream, 200, &jwks);
        }
    });

    let provider = JwksBearerProvider::new(&jwks_url)
        .issuer("test-iss")
        .audience("test-aud")
        .cache_ttl(Duration::from_secs(60));

    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use serde_json::json;
    let header = Header {
        kid: Some(kid.to_string()),
        alg: Algorithm::HS256,
        typ: Some("at+jwt".to_string()),
        ..Default::default()
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let claims = json!({
        "iss": "test-iss",
        "aud": "test-aud",
        "exp": now + 3600,
    });
    let token =
        jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap();

    let mut headers: HeaderVec = HeaderVec::new();
    headers.push((Arc::from("authorization"), format!("Bearer {token}")));
    let req = SecurityRequest {
        headers: &headers,
        query: &ParamVec::new(),
        cookies: &HeaderVec::new(),
    };
    let scheme = brrtrouter::spec::SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: None,
        description: None,
    };

    assert!(provider.validate(&scheme, &[], &req));
    provider.stop_background_refresh();
    server.join().ok();
}

#[test]
fn fetch_get_connect_error_on_dead_port() {
    let options = HttpFetchOptions {
        timeout: Duration::from_millis(200),
        max_body_bytes: 1024,
        extra_headers: Vec::new(),
    };
    let result = fetch_get("http://127.0.0.1:1/closed", &options);
    assert!(result.is_err());
}
