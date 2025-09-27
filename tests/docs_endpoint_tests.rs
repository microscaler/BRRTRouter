use brrtrouter::server::{HttpServer, ServerHandle};
use brrtrouter::{dispatcher::Dispatcher, router::Router, server::AppService};
use pet_store::registry;
use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

mod tracing_util;
use brrtrouter::middleware::TracingMiddleware;
use tracing_util::TestTracing;
mod common;
use common::http::send_request;

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
        Some(PathBuf::from("examples/pet_store/doc")),
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    (tracing, handle, addr)
}

// send_request moved to common::http

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
        } else if let Some((name, val)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-type") {
                content_type = val.trim().to_string();
            }
        }
    }
    (status, content_type, body)
}

#[test]
fn test_openapi_endpoint() {
    let (_tracing, handle, addr) = start_service();
    let resp = send_request(
        &addr,
        "GET /openapi.yaml HTTP/1.1\r\nHost: localhost\r\n\r\n",
    );
    handle.stop();
    let (status, ct, body) = parse_parts(&resp);
    assert_eq!(status, 200);
    assert_eq!(ct, "text/yaml");
    assert!(body.contains("openapi: 3.1.0"));
}

#[test]
fn test_swagger_ui_endpoint() {
    let (_tracing, handle, addr) = start_service();
    let resp = send_request(&addr, "GET /docs HTTP/1.1\r\nHost: localhost\r\n\r\n");
    handle.stop();
    let (status, ct, body) = parse_parts(&resp);
    assert_eq!(status, 200);
    assert!(ct.starts_with("text/html"));
    assert!(body.contains("SwaggerUIBundle"));
}
