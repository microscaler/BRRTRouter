//! Integration tests for `brrtrouter::http` fetch helpers and security-provider wiring.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use brrtrouter::dispatcher::HeaderVec;
use brrtrouter::http::{fetch_get, fetch_get_text_with_retry, HttpFetchOptions};
use brrtrouter::router::ParamVec;
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
fn start_sequential_server(
    responses: Vec<(u16, &'static str)>,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{}:{}/resource", addr.ip(), addr.port());
    let handle = thread::spawn(move || {
        let mut index = 0usize;
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else {
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
            let Ok(mut stream) = incoming else {
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
    let (url, handle) =
        start_sequential_server(vec![(503, "unavailable"), (200, r#"{"keys":[]}"#)]);
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
    assert!(parsed
        .get("keys")
        .and_then(|k| k.as_array())
        .is_some_and(|a| !a.is_empty()));
    drop(handle);
}

#[test]
fn jwks_bearer_provider_loads_keys_via_http_fetch() {
    use base64::Engine;
    use brrtrouter::security::JwksBearerProvider;

    let secret = b"http-fetch-jwks-secret";
    let kid = "fetch-kid";
    let k = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(secret);
    let jwks = format!(r#"{{"keys":[{{"kty":"oct","kid":"{kid}","k":"{k}","alg":"HS256"}}]}}"#);

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
    let token = jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap();

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
fn fetch_post_http_sends_json_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!(
        "http://{}:{}/authz/principals/effective",
        addr.ip(),
        addr.port()
    );
    let server = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let req = read_request(&mut stream);
            assert!(req.starts_with("POST "));
            assert!(req
                .to_ascii_lowercase()
                .contains("content-type: application/json"));
            write_response(&mut stream, 200, r#"{"roles":[{"role":"OWNER"}]}"#);
        }
    });

    let options = HttpFetchOptions {
        timeout: Duration::from_secs(2),
        max_body_bytes: 4096,
        extra_headers: vec![("content-type".to_string(), "application/json".to_string())],
    };
    let (status, body) =
        brrtrouter::http::fetch_post(&url, br#"{"user_id":"u1"}"#, &options).unwrap();
    assert_eq!(status, 200);
    assert!(body.windows(7).any(|w| w == b"\"OWNER\""));
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

/// Capture one request on an ephemeral port; returns the URL to call and the captured head.
fn start_capturing_server(
    path: &str,
) -> (
    String,
    Arc<std::sync::Mutex<String>>,
    thread::JoinHandle<()>,
) {
    let seen = Arc::new(std::sync::Mutex::new(String::new()));
    let seen_clone = Arc::clone(&seen);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}:{}{path}", addr.ip(), addr.port());
    let server = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            *seen_clone.lock().unwrap() = read_request(&mut stream);
            write_response(&mut stream, 200, "ok");
        }
    });
    (url, seen, server)
}

fn host_lines(request: &str) -> Vec<String> {
    request
        .lines()
        .filter_map(|l| {
            let lower = l.to_ascii_lowercase();
            lower.strip_prefix("host:").map(|v| v.trim().to_string())
        })
        .collect()
}

fn plain_options(extra_headers: Vec<(String, String)>) -> HttpFetchOptions {
    HttpFetchOptions {
        timeout: Duration::from_secs(2),
        max_body_bytes: 1024,
        extra_headers,
    }
}

/// Strict HTTP/1.1 servers reject a request without Host; fetch must send exactly one, naming
/// the URL's authority (regression guard for the origin-form request target).
#[test]
fn fetch_get_http_sends_one_host_naming_the_url_authority() {
    let (url, seen, server) = start_capturing_server("/jwks");
    let (status, _) = fetch_get(&url, &plain_options(vec![])).unwrap();
    assert_eq!(status, 200);
    server.join().ok();

    let req = seen.lock().unwrap().clone();
    let authority = url.trim_start_matches("http://").trim_end_matches("/jwks");
    assert_eq!(
        host_lines(&req),
        vec![authority.to_string()],
        "request: {req}"
    );
}

#[test]
fn fetch_post_http_sends_one_host_naming_the_url_authority() {
    let (url, seen, server) = start_capturing_server("/hook");
    let (status, _) = brrtrouter::http::fetch_post(&url, b"{}", &plain_options(vec![])).unwrap();
    assert_eq!(status, 200);
    server.join().ok();

    let req = seen.lock().unwrap().clone();
    let authority = url.trim_start_matches("http://").trim_end_matches("/hook");
    assert_eq!(
        host_lines(&req),
        vec![authority.to_string()],
        "request: {req}"
    );
}

/// A caller-supplied Host (virtual-host routing through an ingress) replaces the default,
/// never duplicates it.
#[test]
fn fetch_get_http_extra_host_header_replaces_the_default() {
    let (url, seen, server) = start_capturing_server("/vhost");
    let options = plain_options(vec![("Host".to_string(), "api.example.com".to_string())]);
    let (status, _) = fetch_get(&url, &options).unwrap();
    assert_eq!(status, 200);
    server.join().ok();

    let req = seen.lock().unwrap().clone();
    assert_eq!(
        host_lines(&req),
        vec!["api.example.com".to_string()],
        "request: {req}"
    );
}

// ---------------------------------------------------------------------------------------------
// Connection reuse (the process-wide fetch pool)
// ---------------------------------------------------------------------------------------------

/// Read one request (head and Content-Length body) from a kept-alive connection; `None` at EOF.
fn read_keepalive_request(stream: &mut TcpStream, pending: &mut Vec<u8>) -> Option<String> {
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = pending.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return None,
            Ok(n) => pending.extend_from_slice(&chunk[..n]),
        }
    };
    let head = String::from_utf8_lossy(&pending[..head_end]).into_owned();
    let length: usize = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse().unwrap_or(0))
        })
        .unwrap_or(0);
    while pending.len() < head_end + length {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return None,
            Ok(n) => pending.extend_from_slice(&chunk[..n]),
        }
    }
    pending.drain(..head_end + length);
    Some(head)
}

/// HTTP/1.1 server that keeps connections alive. Each connection answers up to
/// `per_connection` requests with `body`, then closes without saying so (as a server whose idle
/// timeout fired would). Returns the base URL and the count of accepted connections.
fn start_keepalive_server(
    body: &'static str,
    per_connection: usize,
) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let accepted = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&accepted);
    thread::spawn(move || {
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else { break };
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            thread::spawn(move || {
                let mut pending = Vec::new();
                for _ in 0..per_connection {
                    if read_keepalive_request(&mut stream, &mut pending).is_none() {
                        return;
                    }
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                        body.len()
                    );
                    if stream.write_all(response.as_bytes()).is_err() {
                        return;
                    }
                }
            });
        }
    });
    (format!("http://{}:{}", addr.ip(), addr.port()), accepted)
}

#[test]
fn fetches_to_one_origin_reuse_one_connection() {
    let (base, accepted) = start_keepalive_server("pooled", usize::MAX);
    for i in 0..5 {
        let (status, body) =
            fetch_get(&format!("{base}/item/{i}"), &plain_options(vec![])).unwrap();
        assert_eq!(status, 200);
        assert_eq!(body, b"pooled");
    }
    assert_eq!(
        accepted.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "five sequential fetches to one origin must share one kept-alive connection"
    );
}

#[test]
fn an_oversize_body_is_not_returned_to_the_pool() {
    // 2 KiB body against a 1 KiB limit: the fetch fails and that connection is closed, not reused
    // with half a response still unread on it.
    let big: &'static str = Box::leak("x".repeat(2048).into_boxed_str());
    let (base, accepted) = start_keepalive_server(big, usize::MAX);
    assert!(matches!(
        fetch_get(&format!("{base}/big"), &plain_options(vec![])),
        Err(brrtrouter::http::HttpFetchError::BodyTooLarge)
    ));
    let options = HttpFetchOptions {
        max_body_bytes: 4096,
        ..plain_options(vec![])
    };
    let (status, body) = fetch_get(&format!("{base}/big"), &options).unwrap();
    assert_eq!(status, 200);
    assert_eq!(body.len(), 2048);
    assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn a_post_after_the_server_closed_the_idle_connection_succeeds() {
    // One request per connection, then a silent close: the pooled connection is dead by the
    // second POST. A POST is never retried, so this passes only because the dead connection is
    // detected before the request is written.
    let (base, accepted) = start_keepalive_server("done", 1);
    let url = format!("{base}/hook");
    let (status, _) =
        brrtrouter::http::fetch_post(&url, b"{\"n\":1}", &plain_options(vec![])).unwrap();
    assert_eq!(status, 200);
    thread::sleep(Duration::from_millis(50));
    let (status, _) =
        brrtrouter::http::fetch_post(&url, b"{\"n\":2}", &plain_options(vec![])).unwrap();
    assert_eq!(status, 200);
    assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn a_caller_host_header_bypasses_the_pool_and_still_wins() {
    let (base, accepted) = start_keepalive_server("vhost", usize::MAX);
    let options = plain_options(vec![("Host".to_string(), "api.example.com".to_string())]);
    for _ in 0..2 {
        let (status, _) = fetch_get(&format!("{base}/v"), &options).unwrap();
        assert_eq!(status, 200);
    }
    // not pooled: one connection per fetch
    assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 2);
}

/// Kept-alive server that answers `answer` requests on each connection, then reads the next
/// request and closes without answering it (a server dying, or closing an idle connection just
/// as the request arrives). Returns the base URL, accepted connections and requests received.
fn start_midflight_close_server(
    answer: usize,
) -> (
    String,
    Arc<std::sync::atomic::AtomicUsize>,
    Arc<std::sync::atomic::AtomicUsize>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let accepted = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let received = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (a, r) = (Arc::clone(&accepted), Arc::clone(&received));
    thread::spawn(move || {
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else { break };
            a.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let r = Arc::clone(&r);
            thread::spawn(move || {
                let mut pending = Vec::new();
                for _ in 0..answer {
                    if read_keepalive_request(&mut stream, &mut pending).is_none() {
                        return;
                    }
                    r.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                        .is_err()
                    {
                        return;
                    }
                }
                if read_keepalive_request(&mut stream, &mut pending).is_some() {
                    r.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                // close without answering
            });
        }
    });
    (
        format!("http://{}:{}", addr.ip(), addr.port()),
        accepted,
        received,
    )
}

#[test]
fn a_get_cut_off_mid_flight_on_a_reused_connection_is_retried_once() {
    // Every connection answers one request, then takes the next and closes on it. The second
    // GET goes out on the reused connection, is cut off, and is sent once more on a new one.
    let (base, accepted, received) = start_midflight_close_server(1);
    let url = format!("{base}/thing");
    assert_eq!(fetch_get(&url, &plain_options(vec![])).unwrap().0, 200);
    assert_eq!(fetch_get(&url, &plain_options(vec![])).unwrap().0, 200);
    assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(received.load(std::sync::atomic::Ordering::SeqCst), 3);
    let stats = brrtrouter::http::fetch_pool_stats().expect("pool on by default");
    assert!(stats.stale_retries >= 1, "{stats:?}");
}

#[test]
fn a_post_cut_off_mid_flight_is_not_resent() {
    // The POST reached the server before the connection died: it may have had its effect, so
    // it fails instead of being sent twice.
    let (base, accepted, received) = start_midflight_close_server(1);
    let url = format!("{base}/hook");
    let (status, _) =
        brrtrouter::http::fetch_post(&url, b"{\"n\":1}", &plain_options(vec![])).unwrap();
    assert_eq!(status, 200);
    let err = brrtrouter::http::fetch_post(&url, b"{\"n\":2}", &plain_options(vec![]))
        .expect_err("a POST cut off after sending is not retried");
    assert!(
        matches!(
            err,
            brrtrouter::http::HttpFetchError::Request(_)
                | brrtrouter::http::HttpFetchError::Response(_)
        ),
        "{err:?}"
    );
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        received.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "the second POST was sent exactly once"
    );
    assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 1);
}
