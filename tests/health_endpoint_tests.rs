use brrtrouter::server::{HttpServer, ServerHandle};
use brrtrouter::{dispatcher::Dispatcher, router::Router, server::AppService};
use pet_store::registry;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

mod tracing_util;
use brrtrouter::middleware::TracingMiddleware;
use tracing_util::TestTracing;

fn start_service() -> (TestTracing, ServerHandle, SocketAddr) {
    // ensure coroutines have enough stack for tests
    std::env::set_var("BRRTR_STACK_SIZE", "0x8000");
    let config = brrtrouter::runtime_config::RuntimeConfig::from_env();
    may::config().set_stack_size(config.stack_size);
    let tracing = TestTracing::init();
    let (routes, _slug) = brrtrouter::load_spec("examples/openapi.yaml").unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));
    let service = AppService::new(
        router,
        Arc::new(RwLock::new(dispatcher)),
        HashMap::new(),
        PathBuf::from("examples/openapi.yaml"),
        None,
        None,
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    (tracing, handle, addr)
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

fn parse_response(resp: &str) -> (u16, serde_json::Value) {
    let mut parts = resp.split("\r\n\r\n");
    let headers = parts.next().unwrap_or("");
    let body = parts.next().unwrap_or("");
    let mut status = 0;
    for line in headers.lines() {
        if line.starts_with("HTTP/1.1") {
            status = line
                .split_whitespace()
                .nth(1)
                .unwrap_or("0")
                .parse()
                .unwrap();
        }
    }
    let json: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    (status, json)
}

#[test]
fn test_health_endpoint() {
    let (_tracing, handle, addr) = start_service();
    let resp = send_request(&addr, "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n");
    handle.stop();
    let (status, body) = parse_response(&resp);
    assert_eq!(status, 200);
    assert_eq!(body["status"], "ok");
}
