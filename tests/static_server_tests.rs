use brrtrouter::server::{HttpServer, ServerHandle};
use brrtrouter::{dispatcher::Dispatcher, router::Router, server::AppService};
use pet_store::registry;
use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Test fixture for static file serving tests with automatic setup and teardown using RAII
///
/// This fixture specifically tests static file serving with the test static files
/// in `tests/staticdata/`. It includes full service integration to ensure static
/// file serving doesn't break when other parts of the system change.
struct StaticFileTestServer {
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}

impl StaticFileTestServer {
    /// Create a new static file test server
    fn new() -> Self {
        // Setup: Configure coroutine stack size
        may::config().set_stack_size(0x8000);

        let (routes, _slug) = brrtrouter::load_spec("examples/openapi.yaml").unwrap();
        let router = Arc::new(RwLock::new(Router::new(routes.clone())));
        let mut dispatcher = Dispatcher::new();
        unsafe {
            registry::register_from_spec(&mut dispatcher, &routes);
        }

        // Important: Uses tests/staticdata for static files and includes doc directory
        // for comprehensive integration testing
        let service = AppService::new(
            router,
            Arc::new(RwLock::new(dispatcher)),
            HashMap::new(),
            PathBuf::from("examples/openapi.yaml"),
            Some(PathBuf::from("tests/staticdata")),
            Some(PathBuf::from("examples/pet_store/doc")),
        );

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let handle = HttpServer(service).start(addr).unwrap();
        handle.wait_ready().unwrap();

        Self {
            handle: Some(handle),
            addr,
        }
    }

    /// Get the server address for making requests
    fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for StaticFileTestServer {
    /// Teardown: Automatically stop server when test completes
    ///
    /// This ensures proper cleanup even if the test panics,
    /// preventing resource leaks and port conflicts.
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
    }
}

mod common;
use common::http::send_request;

fn parse_parts(resp: &str) -> (u16, String) {
    let mut parts = resp.split("\r\n\r\n");
    let headers = parts.next().unwrap_or("");
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
    let server = StaticFileTestServer::new();
    let resp = send_request(&server.addr(), "GET /bundle.js HTTP/1.1\r\nHost: x\r\n\r\n");
    let (status, ct) = parse_parts(&resp);
    assert_eq!(status, 200);
    assert_eq!(ct, "application/javascript");

    // Automatic cleanup!
}

#[test]
fn test_root_served() {
    let server = StaticFileTestServer::new();
    let resp = send_request(&server.addr(), "GET / HTTP/1.1\r\nHost: x\r\n\r\n");
    let (status, ct) = parse_parts(&resp);
    assert_eq!(status, 200);
    assert_eq!(ct, "text/html");

    // Automatic cleanup!
}

#[test]
fn test_traversal_blocked() {
    let server = StaticFileTestServer::new();
    let resp = send_request(
        &server.addr(),
        "GET /../Cargo.toml HTTP/1.1\r\nHost: x\r\n\r\n",
    );
    let (status, _) = parse_parts(&resp);
    assert_eq!(status, 404);

    // Automatic cleanup!
}
