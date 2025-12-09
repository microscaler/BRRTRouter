#![allow(clippy::unwrap_used, clippy::expect_used)]

use brrtrouter::server::{HttpServer, ServerHandle};
use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HandlerResponse},
    router::Router,
    server::AppService,
    spec::{ResponseSpec, RouteMeta},
};
use http::Method;
use serde_json::json;
use std::collections::HashMap;

use std::net::{SocketAddr, TcpListener};
mod common;
use common::http::send_request;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Test fixture for multi-response content negotiation tests with automatic setup and teardown using RAII
///
/// This fixture tests OpenAPI response specifications with multiple content types.
struct MultiResponseTestServer {
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}

impl MultiResponseTestServer {
    /// Create a new multi-response test server with custom response specs
    fn new() -> Self {
        may::config().set_stack_size(0x8000);

        let responses = {
            let mut m = HashMap::new();
            let mut inner = HashMap::new();
            inner.insert(
                "text/plain".to_string(),
                ResponseSpec {
                    schema: None,
                    example: None,
                },
            );
            m.insert(201u16, inner);
            m
        };

        let route = RouteMeta {
            method: Method::POST,
            path_pattern: Arc::from("/resp"),
            handler_name: Arc::from("h"),
            parameters: vec![],
            request_schema: None,
            request_body_required: false,
            response_schema: None,
            example: None,
            responses,
            security: vec![],
            example_name: String::new(),
            project_slug: String::new(),
            output_dir: PathBuf::new(),
            base_path: String::new(),
            sse: false,
            estimated_request_body_bytes: None,
            x_brrtrouter_stack_size: None,
            cors_config: None,
        };

        let router = Arc::new(RwLock::new(Router::new(vec![route])));
        let mut dispatcher = Dispatcher::new();
        unsafe {
            dispatcher.register_handler("h", |_req: HandlerRequest| {
                let resp = HandlerResponse::json(201, json!("ok"));
                let _ = _req.reply_tx.send(resp);
            });
        }

        // Include static and doc directories for comprehensive integration testing
        let service = AppService::new(
            router,
            Arc::new(RwLock::new(dispatcher)),
            HashMap::new(),
            PathBuf::from("examples/openapi.yaml"),
            Some(PathBuf::from("examples/pet_store/static_site")),
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

    fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for MultiResponseTestServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
    }
}

fn parse_parts(resp: &str) -> (u16, String) {
    let mut parts = resp.split("\r\n\r\n");
    let headers = parts.next().unwrap_or("");
    let mut status = 0;
    let mut ct = String::new();
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
                ct = v.trim().to_string();
            }
        }
    }
    (status, ct)
}

#[test]
fn test_select_content_type_from_spec() {
    let server = MultiResponseTestServer::new();
    let resp = send_request(&server.addr(), "POST /resp HTTP/1.1\r\nHost: x\r\n\r\n");
    let (status, ct) = parse_parts(&resp);
    assert_eq!(status, 201);
    assert_eq!(ct, "text/plain");

    // Automatic cleanup!
}
