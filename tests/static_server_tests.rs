use brrtrouter::{dispatcher::Dispatcher, router::Router, server::AppService};
use brrtrouter::server::{HttpServer, ServerHandle};
use pet_store::registry;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

fn start_service() -> (ServerHandle, SocketAddr) {
    let (routes, _slug) = brrtrouter::load_spec("examples/openapi.yaml").unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    unsafe { registry::register_from_spec(&mut dispatcher, &routes); }
    let service = AppService::new(
        router,
        Arc::new(RwLock::new(dispatcher)),
        HashMap::new(),
        PathBuf::from("examples/openapi.yaml"),
        Some(PathBuf::from("tests/staticdata")),
        None,
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    (handle, addr)
}

fn send_request(addr: &SocketAddr, req: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(req.as_bytes()).unwrap();
    stream.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    let mut buf = Vec::new();
    loop {
        let mut tmp = [0u8; 1024];
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => panic!("read error: {:?}", e),
        }
    }
    String::from_utf8_lossy(&buf).to_string()
}

fn parse_parts(resp: &str) -> (u16, String) {
    let mut parts = resp.split("\r\n\r\n");
    let headers = parts.next().unwrap_or("");
    let mut status = 0;
    let mut content_type = String::new();
    for line in headers.lines() {
        if line.starts_with("HTTP/1.1") {
            status = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap();
        } else if let Some((name, val)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-type") {
                content_type = val.trim().to_string();
            }
        }
    }
    (status, content_type)
}

#[test]
fn test_js_served() {
    let (handle, addr) = start_service();
    let resp = send_request(&addr, "GET /bundle.js HTTP/1.1\r\nHost: x\r\n\r\n");
    handle.stop();
    let (status, ct) = parse_parts(&resp);
    assert_eq!(status, 200);
    assert_eq!(ct, "application/javascript");
}

#[test]
fn test_traversal_blocked() {
    let (handle, addr) = start_service();
    let resp = send_request(&addr, "GET /../Cargo.toml HTTP/1.1\r\nHost: x\r\n\r\n");
    handle.stop();
    let (status, _) = parse_parts(&resp);
    assert_eq!(status, 404);
}
