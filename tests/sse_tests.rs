use brrtrouter::dispatcher::Dispatcher;
use brrtrouter::router::Router;
use brrtrouter::server::AppService;
use brrtrouter::server::{HttpServer, ServerHandle};
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
        .set_read_timeout(Some(Duration::from_millis(200)))
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

fn parse_parts(resp: &str) -> (u16, String, String) {
    let mut parts = resp.split("\r\n\r\n");
    let headers = parts.next().unwrap_or("");
    let body = parts.next().unwrap_or("").to_string();
    let mut status = 0;
    let mut content_type = String::new();
    for line in headers.lines() {
        if line.starts_with("HTTP/1.1") {
            status = line
                .split_whitespace()
                .nth(1)
                .unwrap_or("0")
                .parse()
                .unwrap();
        } else if let Some((n, v)) = line.split_once(':') {
            if n.eq_ignore_ascii_case("content-type") {
                content_type = v.trim().to_string();
            }
        }
    }
    (status, content_type, body)
}

#[test]
fn test_event_stream() {
    let (_tracing, handle, addr) = start_service();
    let resp = send_request(&addr, "GET /events HTTP/1.1\r\nHost: localhost\r\n\r\n");
    handle.stop();
    let (status, ct, body) = parse_parts(&resp);
    assert_eq!(status, 200);
    assert_eq!(ct, "text/event-stream");
    let events: Vec<_> = body
        .lines()
        .filter(|l| l.starts_with("data: "))
        .map(|l| l[6..].trim())
        .collect();
    assert_eq!(events, ["tick 0", "tick 1", "tick 2"]);
}
