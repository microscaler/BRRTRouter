//! A BFF passes a proxied downstream's 2xx JSON body through as it came (no parse, no
//! re-serialisation, no response validation), unless the operation sets
//! `x-brrtrouter-validate-response: true`; then the text is parsed and checked against the
//! operation's response schema. Errors keep the parsed path.

use brrtrouter::dispatcher::{Dispatcher, HandlerRequest};
use brrtrouter::router::Router;
use brrtrouter::server::{AppService, HttpServer, ServerHandle};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

/// Off-schema on purpose (`symbol` is a number), with spacing serde_json would not write.
const BAD: &str = r#"[ {"symbol": 1, "price": 2.50} ]"#;
const GOOD: &str = r#"[ {"symbol": "SPX", "price": 2.50} ]"#;
const NOT_FOUND: &str = r#"{"error":"nope"}"#;

fn downstream() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            let mut buf = [0u8; 8192];
            let mut n = 0;
            while !buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut buf[n..]) {
                    Ok(0) | Err(_) => break,
                    Ok(m) => n += m,
                }
            }
            let head = String::from_utf8_lossy(&buf[..n]);
            let (status, body) = if head.starts_with("GET /down/bad ") {
                ("200 OK", BAD)
            } else if head.starts_with("GET /down/good ") {
                ("200 OK", GOOD)
            } else {
                ("404 Not Found", NOT_FOUND)
            };
            let _ = s.write_all(
                format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
        }
    });
    port
}

fn spec() -> String {
    let schema = r#"{"type":"array","items":{"type":"object","properties":{"symbol":{"type":"string"},"price":{"type":"number"}}}}"#;
    let op = |path: &str, id: &str, down: &str, validate: bool| {
        let flag = if validate {
            r#""x-brrtrouter-validate-response":true,"#
        } else {
            ""
        };
        format!(
            r#""{path}":{{"get":{{"operationId":"{id}","x-service":"127.0.0.1","x-brrtrouter-downstream-path":"{down}",{flag}"responses":{{"200":{{"description":"rows","content":{{"application/json":{{"schema":{schema}}}}}}},"404":{{"description":"missing","content":{{"application/json":{{"schema":{{"type":"object"}}}}}}}}}}}}}}"#
        )
    };
    format!(
        r#"{{"openapi":"3.1.0","info":{{"title":"passthrough","version":"1"}},"paths":{{{},{},{},{}}}}}"#,
        op("/pass_bad", "pass_bad", "/down/bad", false),
        op("/check_bad", "check_bad", "/down/bad", true),
        op("/check_good", "check_good", "/down/good", true),
        op("/missing", "missing", "/down/missing", false),
    )
}

fn start() -> (ServerHandle, SocketAddr) {
    may::config().set_stack_size(2 * 1024 * 1024 / std::mem::size_of::<usize>());
    std::env::set_var("SERVICE_HTTP_PORT", downstream().to_string());
    std::env::remove_var("POD_NAMESPACE");
    let dir = tempfile::tempdir().unwrap().keep();
    let path = dir.join("openapi.json");
    std::fs::write(&path, spec()).unwrap();
    let path = path.to_str().unwrap().to_string();
    let (routes, schemes, _slug) = brrtrouter::load_spec_full(&path).expect("spec");
    let mut dispatcher = Dispatcher::new();
    for r in &routes {
        let target = r.x_brrtrouter_downstream_path.clone().unwrap();
        unsafe {
            dispatcher.register_handler(&r.handler_name, move |req: HandlerRequest| {
                let rsp = brrtrouter::http::proxy_untyped(&req, "127.0.0.1", &target);
                req.reply(rsp);
            });
        }
    }
    let router = Arc::new(arc_swap::ArcSwap::from_pointee(Router::new(routes.clone())));
    let mut service = AppService::new(
        router,
        Arc::new(arc_swap::ArcSwap::from_pointee(dispatcher)),
        schemes,
        path.into(),
        None,
        None,
    );
    service.resolve_security(&routes);
    let addr = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let handle = HttpServer(service).start(addr).expect("server");
    for _ in 0..200 {
        if TcpStream::connect(addr).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    (handle, addr)
}

/// (status, lowercased head, body)
fn get(addr: SocketAddr, path: &str) -> (u16, String, String) {
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(
        s,
        "GET {path} HTTP/1.1\r\nHost: bff\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    // Read the head, then exactly Content-Length bytes (the server may keep the socket open).
    loop {
        let n = s.read(&mut buf).unwrap();
        assert!(n > 0, "closed early: {:?}", String::from_utf8_lossy(&out));
        out.extend_from_slice(&buf[..n]);
        let Some(end) = out.windows(4).position(|w| w == b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&out[..end]).to_ascii_lowercase();
        let cl: usize = head
            .lines()
            .find_map(|l| l.strip_prefix("content-length:"))
            .map(|v| v.trim().parse().unwrap())
            .unwrap_or(0);
        while out.len() < end + 4 + cl {
            let n = s.read(&mut buf).unwrap();
            assert!(n > 0);
            out.extend_from_slice(&buf[..n]);
        }
        let status = head[9..12].parse().unwrap();
        let body = String::from_utf8(out[end + 4..end + 4 + cl].to_vec()).unwrap();
        return (status, head, body);
    }
}

#[test]
fn proxied_json_passes_through_and_validates_only_when_asked() {
    let (app, addr) = start();

    // Without the flag: the downstream's bytes, exactly (spacing kept, off-schema not refused).
    let (status, head, body) = get(addr, "/pass_bad");
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, BAD);
    assert!(head.contains("content-type: application/json"), "{head}");
    assert!(!head.contains("x-brrtrouter-raw-encoding"), "{head}");

    // With the flag: an off-schema body is refused as before ...
    let (status, _, body) = get(addr, "/check_bad");
    assert_eq!(status, 500, "{body}");
    assert!(body.contains("Response validation failed"), "{body}");

    // ... and a valid one still goes out as the downstream wrote it.
    let (status, _, body) = get(addr, "/check_good");
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, GOOD);

    // A downstream error keeps the parsed path.
    let (status, _, body) = get(addr, "/missing");
    assert_eq!(status, 404, "{body}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"error": "nope"})
    );
    drop(app);
}
