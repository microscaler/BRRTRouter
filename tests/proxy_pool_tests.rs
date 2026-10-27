//! Proxied calls reuse kept-alive connections to the downstream (one pool per downstream origin),
//! and a connection the downstream closed while idle is replaced before a request is written to
//! it, so even a POST (never resent) survives a downstream's idle timeout.

use brrtrouter::dispatcher::{HandlerRequest, HeaderVec};
use brrtrouter::ids::RequestId;
use brrtrouter::router::ParamVec;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Downstream that answers `answer` requests per connection, then reads the next one and closes
/// without answering it. Returns the port and the number of requests received.
fn midflight_close_downstream(answer: usize) -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = Arc::new(AtomicUsize::new(0));
    let r = received.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let r = r.clone();
            std::thread::spawn(move || {
                let mut pending = Vec::new();
                for _ in 0..answer {
                    if !read_request(&mut stream, &mut pending) {
                        return;
                    }
                    r.fetch_add(1, Ordering::SeqCst);
                    let body = br#"{"ok":true}"#;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                        body.len()
                    );
                    if stream.write_all(head.as_bytes()).is_err() || stream.write_all(body).is_err()
                    {
                        return;
                    }
                }
                if read_request(&mut stream, &mut pending) {
                    r.fetch_add(1, Ordering::SeqCst);
                }
            });
        }
    });
    (port, received)
}

/// Read one request (head and Content-Length body); `false` at EOF.
fn read_request(stream: &mut std::net::TcpStream, pending: &mut Vec<u8>) -> bool {
    let mut buf = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = pending.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => return false,
            Ok(k) => pending.extend_from_slice(&buf[..k]),
        }
    };
    let head = String::from_utf8_lossy(&pending[..head_end]).to_ascii_lowercase();
    let length: usize = head
        .lines()
        .find_map(|l| {
            l.strip_prefix("content-length:")
                .map(|v| v.trim().parse().unwrap_or(0))
        })
        .unwrap_or(0);
    while pending.len() < head_end + length {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => return false,
            Ok(k) => pending.extend_from_slice(&buf[..k]),
        }
    }
    pending.drain(..head_end + length);
    true
}

/// Downstream that keeps connections alive and answers up to `per_connection` requests on each,
/// then closes it without saying so. Returns the port and the count of accepted connections.
fn keepalive_downstream(per_connection: usize) -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            c.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || {
                let mut pending = Vec::new();
                let mut buf = [0u8; 4096];
                for _ in 0..per_connection {
                    let head_end = loop {
                        if let Some(i) = pending.windows(4).position(|w| w == b"\r\n\r\n") {
                            break i + 4;
                        }
                        match stream.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(k) => pending.extend_from_slice(&buf[..k]),
                        }
                    };
                    let head = String::from_utf8_lossy(&pending[..head_end]).to_ascii_lowercase();
                    let length: usize = head
                        .lines()
                        .find_map(|l| {
                            l.strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    while pending.len() < head_end + length {
                        match stream.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(k) => pending.extend_from_slice(&buf[..k]),
                        }
                    }
                    pending.drain(..head_end + length);
                    let body = br#"{"ok":true}"#;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                        body.len()
                    );
                    if stream.write_all(head.as_bytes()).is_err() || stream.write_all(body).is_err()
                    {
                        return;
                    }
                }
            });
        }
    });
    (port, count)
}

fn request(method: http::Method) -> HandlerRequest {
    let (tx, _rx) = may::sync::mpsc::channel();
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("accept"), "application/json".to_string()));
    HandlerRequest {
        request_id: RequestId::new(),
        method,
        path: "/thing".to_string(),
        handler_name: "bff".to_string(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        raw_query: None,
        headers,
        cookies: HeaderVec::new(),
        body: Some(serde_json::json!({"n": 1})),
        jwt_claims: None,
        reply_tx: tx,
        queue_guard: None,
        span: tracing::Span::none(),
    }
}

// One test: SERVICE_HTTP_PORT is process-wide.
#[test]
fn proxied_calls_reuse_connections_and_survive_a_downstream_idle_close() {
    std::env::remove_var("POD_NAMESPACE");

    let (port, conns) = keepalive_downstream(usize::MAX);
    std::env::set_var("SERVICE_HTTP_PORT", port.to_string());
    for _ in 0..5 {
        let mut get = request(http::Method::GET);
        get.body = None;
        let res = brrtrouter::http::proxy_untyped(&get, "127.0.0.1", "/thing");
        assert_eq!(res.status, 200, "{:?}", res.body);
    }
    let res = brrtrouter::http::proxy_untyped(&request(http::Method::POST), "127.0.0.1", "/thing");
    assert_eq!(res.status, 200, "{:?}", res.body);
    assert_eq!(
        conns.load(Ordering::SeqCst),
        1,
        "sequential proxied calls to one downstream share one connection"
    );

    // One request per connection, then a silent close (the downstream's idle timeout).
    let (port, conns) = keepalive_downstream(1);
    std::env::set_var("SERVICE_HTTP_PORT", port.to_string());
    for _ in 0..3 {
        let res =
            brrtrouter::http::proxy_untyped(&request(http::Method::POST), "127.0.0.1", "/thing");
        assert_eq!(
            res.status, 200,
            "POST after the downstream closed idle: {:?}",
            res.body
        );
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    assert_eq!(conns.load(Ordering::SeqCst), 3);

    // Mid-flight: the downstream takes the request on a reused connection and closes on it.
    // A GET is sent once more on a new connection; a POST is not resent (it may have had its
    // effect) and the caller gets a 502.
    let (port, received) = midflight_close_downstream(1);
    std::env::set_var("SERVICE_HTTP_PORT", port.to_string());
    let mut get = request(http::Method::GET);
    get.body = None;
    assert_eq!(
        brrtrouter::http::proxy_untyped(&get, "127.0.0.1", "/thing").status,
        200
    );
    let res = brrtrouter::http::proxy_untyped(&get, "127.0.0.1", "/thing");
    assert_eq!(
        res.status, 200,
        "GET cut off mid-flight is retried: {:?}",
        res.body
    );
    assert_eq!(received.load(Ordering::SeqCst), 3);

    let (port, received) = midflight_close_downstream(1);
    std::env::set_var("SERVICE_HTTP_PORT", port.to_string());
    let post = request(http::Method::POST);
    assert_eq!(
        brrtrouter::http::proxy_untyped(&post, "127.0.0.1", "/thing").status,
        200
    );
    let res = brrtrouter::http::proxy_untyped(&post, "127.0.0.1", "/thing");
    assert_eq!(
        res.status, 502,
        "POST cut off mid-flight is not resent: {:?}",
        res.body
    );
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(
        received.load(Ordering::SeqCst),
        2,
        "the second POST was sent exactly once"
    );

    let stats = brrtrouter::http::proxy_pool_stats().expect("pool on by default");
    assert!(stats.connections_reused >= 5, "{stats:?}");
    assert!(stats.connections_discarded >= 2, "{stats:?}");
}
