//! A proxied GET whose first attempt is cut off after the request was sent (the downstream
//! closes without answering, as a pod leaving in a rollout does) is sent once more and
//! succeeds. A POST that meets the same close is not resent.

use brrtrouter::dispatcher::{HandlerRequest, HeaderVec};
use brrtrouter::ids::RequestId;
use brrtrouter::router::ParamVec;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Downstream that drops the first `drops` connections after reading the request head and
/// answers `200 {"ok":true}` on the rest. Returns the port and the connection counter.
fn flaky_downstream(drops: usize) -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let n = c.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 4096];
            let mut head = Vec::new();
            while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(k) => head.extend_from_slice(&buf[..k]),
                }
            }
            if n < drops {
                drop(stream);
                continue;
            }
            let body = br#"{"ok":true}"#;
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(body);
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
        body: None,
        jwt_claims: None,
        reply_tx: tx,
        queue_guard: None,
        span: tracing::Span::none(),
    }
}

// One test: SERVICE_HTTP_PORT is process-wide.
#[test]
fn get_cut_off_once_is_retried_post_is_not() {
    std::env::remove_var("POD_NAMESPACE");

    let (port, conns) = flaky_downstream(1);
    std::env::set_var("SERVICE_HTTP_PORT", port.to_string());
    let res = brrtrouter::http::proxy_untyped(&request(http::Method::GET), "127.0.0.1", "/thing");
    assert_eq!(res.status, 200, "GET after one cut-off: {:?}", res.body);
    assert_eq!(conns.load(Ordering::SeqCst), 2);

    let (port, conns) = flaky_downstream(1);
    std::env::set_var("SERVICE_HTTP_PORT", port.to_string());
    let res = brrtrouter::http::proxy_untyped(&request(http::Method::POST), "127.0.0.1", "/thing");
    assert_eq!(res.status, 502, "POST is not resent: {:?}", res.body);
    assert_eq!(conns.load(Ordering::SeqCst), 1);
}
