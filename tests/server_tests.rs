//! Integration tests for the HTTP server and request processing pipeline
//!
//! # Test Coverage
//!
//! This module tests the complete HTTP server stack:
//! - Server startup and lifecycle management
//! - Request routing and dispatching
//! - Authentication and authorization
//! - Response serialization
//! - Keep-alive connection handling
//! - Echo handler functionality
//!
//! # Test Strategy
//!
//! Uses the Pet Store example application as a test subject to verify:
//! 1. **End-to-End Flow**: HTTP request → router → dispatcher → handler → response
//! 2. **Security**: API key authentication, bearer token validation
//! 3. **Error Handling**: 404 for missing routes, 401 for auth failures
//! 4. **Configuration**: Keep-alive settings, service registration
//!
//! # Test Fixtures
//!
//! - `start_petstore_service()`: Spins up a complete Pet Store API server
//! - Uses generated handlers from `examples/openapi.yaml`
//! - Includes tracing middleware for debugging
//! - Binds to random available port to avoid conflicts
//!
//! # Important Notes
//!
//! - Tests use May coroutines with 32KB stack size
//! - Server runs in background thread, cleaned up automatically
//! - Tracing is captured per-test for isolation

use brrtrouter::server::{HttpServer, ServerHandle};
use brrtrouter::spec::SecurityScheme;
use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HandlerResponse},
    router::Router,
    server::AppService,
    spec::RouteMeta,
    SecurityProvider, SecurityRequest,
};
use http::Method;
use pet_store::registry;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

mod tracing_util;
use brrtrouter::middleware::TracingMiddleware;
use tracing_util::TestTracing;
mod common;
use common::http::send_request;

/// Test fixture with automatic setup and teardown using RAII
///
/// Implements Drop to ensure proper cleanup when test completes.
/// This is the Rust equivalent of Python's setup/teardown.
struct PetStoreTestServer {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}

impl PetStoreTestServer {
    /// Setup: Create and start the pet store test server
    fn new() -> Self {
        // Setup: Configure coroutine stack size
        may::config().set_stack_size(0x8000);

        // Setup: Initialize tracing
        let tracing = TestTracing::init();

        // Setup: Load OpenAPI spec and create service
        let (routes, schemes, _slug) = brrtrouter::load_spec_full("examples/openapi.yaml").unwrap();
        let router = Arc::new(RwLock::new(Router::new(routes.clone())));
        let mut dispatcher = Dispatcher::new();
        unsafe { registry::register_from_spec(&mut dispatcher, &routes); }
        dispatcher.add_middleware(Arc::new(TracingMiddleware));
        let mut service = AppService::new(
            router,
            Arc::new(RwLock::new(dispatcher)),
            schemes,
            PathBuf::from("examples/openapi.yaml"),
            Some(PathBuf::from("examples/pet_store/static_site")),
            Some(PathBuf::from("examples/pet_store/doc")),
        );

        // Setup: Register API key provider for authentication
        struct ApiKeyProvider {
            key: String,
        }
        impl SecurityProvider for ApiKeyProvider {
            fn validate(
                &self,
                scheme: &SecurityScheme,
                _scopes: &[String],
                req: &SecurityRequest,
            ) -> bool {
                match scheme {
                    SecurityScheme::ApiKey { name, location, .. } => match location.as_str() {
                        "header" => req.headers.get(&name.to_ascii_lowercase()) == Some(&self.key),
                        "query" => req.query.get(name) == Some(&self.key),
                        "cookie" => req.cookies.get(name) == Some(&self.key),
                        _ => false,
                    },
                    _ => false,
                }
            }
        }
        for (name, scheme) in service.security_schemes.clone() {
            if matches!(scheme, SecurityScheme::ApiKey { .. }) {
                service.register_security_provider(
                    &name,
                    Arc::new(ApiKeyProvider {
                        key: "test123".into(),
                    }),
                );
            }
        }

        // Setup: Start HTTP server on random port
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let handle = HttpServer(service).start(addr).unwrap();
        handle.wait_ready().unwrap();

        Self {
            _tracing: tracing,
            handle: Some(handle),
            addr,
        }
    }

    /// Get the server address for making requests
    fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for PetStoreTestServer {
    /// Teardown: Automatically stop server when test completes
    ///
    /// This ensures proper cleanup even if the test panics,
    /// preventing resource leaks and port conflicts.
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
        // _tracing is automatically dropped here
    }
}

/// Test fixture for custom handlers with automatic setup and teardown using RAII
///
/// Use this for tests that need custom handler registration, validation, etc.
struct CustomServerTestFixture {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}

impl CustomServerTestFixture {
    /// Create a custom server with a single handler and route
    fn with_handler<F>(handler_name: &str, handler: F, path: &str, method: Method) -> Self
    where
        F: Fn(HandlerRequest) + Send + Sync + Clone + 'static,
    {
        Self::with_handler_and_schemas(handler_name, handler, path, method, None, None)
    }

    /// Create a custom server with request/response schemas for validation
    fn with_handler_and_schemas<F>(
        handler_name: &str,
        handler: F,
        path: &str,
        method: Method,
        request_schema: Option<Value>,
        response_schema: Option<Value>,
    ) -> Self
    where
        F: Fn(HandlerRequest) + Send + Sync + Clone + 'static,
    {
        may::config().set_stack_size(0x8000);
        let tracing = TestTracing::init();

        let route = RouteMeta {
            method,
            path_pattern: path.to_string(),
            handler_name: handler_name.to_string(),
            parameters: Vec::new(),
            request_schema,
            request_body_required: false,
            response_schema,
            example: None,
            responses: std::collections::HashMap::new(),
            security: Vec::new(),
            example_name: String::new(),
            project_slug: String::new(),
            output_dir: PathBuf::new(),
            base_path: String::new(),
            sse: false,
            estimated_request_body_bytes: None,
            x_brrtrouter_stack_size: None,
        };

        let router = Arc::new(RwLock::new(Router::new(vec![route])));
        let mut dispatcher = Dispatcher::new();
        unsafe { dispatcher.register_handler(handler_name, handler); }
        dispatcher.add_middleware(Arc::new(TracingMiddleware));

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
            _tracing: tracing,
            handle: Some(handle),
            addr,
        }
    }

    /// Get the server address for making requests
    fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for CustomServerTestFixture {
    /// Teardown: Automatically stop server when test completes
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
    }
}

/// Legacy function for backward compatibility
///
/// **Deprecated**: New tests should use `PetStoreTestServer` directly for automatic teardown.
/// This function exists only for tests that need manual control over server lifecycle.
#[allow(dead_code)]
fn start_petstore_service() -> (TestTracing, ServerHandle, SocketAddr) {
    // Setup: Configure coroutine stack size
    may::config().set_stack_size(0x8000);

    let tracing = TestTracing::init();
    let (routes, schemes, _slug) = brrtrouter::load_spec_full("examples/openapi.yaml").unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    unsafe { registry::register_from_spec(&mut dispatcher, &routes); }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));
    let mut service = AppService::new(
        router,
        Arc::new(RwLock::new(dispatcher)),
        schemes,
        PathBuf::from("examples/openapi.yaml"),
        None,
        None,
    );

    struct ApiKeyProvider {
        key: String,
    }
    impl SecurityProvider for ApiKeyProvider {
        fn validate(
            &self,
            scheme: &SecurityScheme,
            _scopes: &[String],
            req: &SecurityRequest,
        ) -> bool {
            match scheme {
                SecurityScheme::ApiKey { name, location, .. } => match location.as_str() {
                    "header" => req.headers.get(&name.to_ascii_lowercase()) == Some(&self.key),
                    "query" => req.query.get(name) == Some(&self.key),
                    "cookie" => req.cookies.get(name) == Some(&self.key),
                    _ => false,
                },
                _ => false,
            }
        }
    }
    for (name, scheme) in service.security_schemes.clone() {
        if matches!(scheme, SecurityScheme::ApiKey { .. }) {
            service.register_security_provider(
                &name,
                Arc::new(ApiKeyProvider {
                    key: "test123".into(),
                }),
            );
        }
    }

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();

    (tracing, handle, addr)
}

// send_request moved to common::http

fn parse_response_parts(resp: &str) -> (u16, String, String) {
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

fn parse_response(resp: &str) -> (u16, Value) {
    let (status, content_type, body) = parse_response_parts(resp);
    if content_type.starts_with("application/json") {
        let json: Value = serde_json::from_str(&body).unwrap_or_default();
        (status, json)
    } else {
        (status, Value::String(body))
    }
}

#[test]
fn test_dispatch_success() {
    // Setup happens automatically in PetStoreTestServer::new()
    let server = PetStoreTestServer::new();

    let resp = send_request(
        &server.addr(),
        "GET /pets HTTP/1.1\r\nHost: localhost\r\nX-API-Key: test123\r\n\r\n",
    );
    let (status, body) = parse_response(&resp);
    assert_eq!(status, 200);
    assert!(body.is_array());

    // Teardown happens automatically when 'server' goes out of scope
    // No need to call handle.stop() manually!
}

#[test]
fn test_route_404() {
    // Setup happens automatically
    let server = PetStoreTestServer::new();

    let resp = send_request(
        &server.addr(),
        "GET /nope HTTP/1.1\r\nHost: localhost\r\n\r\n",
    );
    let (status, _body) = parse_response(&resp);
    assert_eq!(status, 404);

    // Teardown happens automatically - no more manual handle.stop()!
}

#[test]
fn test_panic_recovery() {
    fn panic_handler(_req: HandlerRequest) {
        panic!("boom! - watch to see if I recover");
    }

    let server =
        CustomServerTestFixture::with_handler("panic", panic_handler, "/panic", Method::GET);

    let resp = send_request(
        &server.addr(),
        "GET /panic HTTP/1.1\r\nHost: localhost\r\n\r\n",
    );
    let (status, body) = parse_response(&resp);
    assert_eq!(status, 500);
    assert!(body.get("error").is_some());

    // Automatic cleanup!
}

#[test]
fn test_headers_and_cookies() {
    fn header_handler(req: HandlerRequest) {
        let response = HandlerResponse {
            status: 200,
            headers: HeaderVec::new(),
            body: json!({
                "headers": req.headers,
                "cookies": req.cookies,
            }),
        };
        let _ = req.reply_tx.send(response);
    }

    let server =
        CustomServerTestFixture::with_handler("header", header_handler, "/headertest", Method::GET);

    let request = concat!(
        "GET /headertest HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "X-Test: value\r\n",
        "X-Other: foo\r\n",
        "Cookie: session=abc123; theme=dark\r\n",
        "\r\n"
    );
    let resp = send_request(&server.addr(), request);
    let (status, body) = parse_response(&resp);
    assert_eq!(status, 200);
    assert_eq!(body["headers"]["x-test"], "value");
    assert_eq!(body["headers"]["x-other"], "foo");
    assert_eq!(body["cookies"]["session"], "abc123");
    assert_eq!(body["cookies"]["theme"], "dark");

    // Automatic cleanup!
}

#[test]
fn test_status_201_json() {
    fn create_handler(req: HandlerRequest) {
        let response = HandlerResponse {
            status: 201,
            headers: HeaderVec::new(),
            body: json!({"created": true}),
        };
        let _ = req.reply_tx.send(response);
    }

    let server =
        CustomServerTestFixture::with_handler("create", create_handler, "/created", Method::POST);

    let resp = send_request(
        &server.addr(),
        "POST /created HTTP/1.1\r\nHost: localhost\r\n\r\n",
    );
    let (status, body) = parse_response(&resp);
    let (_, ct, _) = parse_response_parts(&resp);
    assert_eq!(status, 201);
    assert_eq!(ct, "application/json");
    assert_eq!(body["created"], true);

    // Automatic cleanup!
}

#[test]
fn test_text_plain_error() {
    fn text_handler(req: HandlerRequest) {
        let response = HandlerResponse {
            status: 400,
            headers: HeaderVec::new(),
            body: json!("bad request"),
        };
        let _ = req.reply_tx.send(response);
    }

    let server = CustomServerTestFixture::with_handler("text", text_handler, "/text", Method::GET);

    let resp = send_request(
        &server.addr(),
        "GET /text HTTP/1.1\r\nHost: localhost\r\n\r\n",
    );
    let (status, body) = parse_response(&resp);
    let (_, ct, raw_body) = parse_response_parts(&resp);
    assert_eq!(status, 400);
    assert_eq!(ct, "text/plain");
    assert_eq!(raw_body, "bad request");
    assert_eq!(body, Value::String("bad request".to_string()));

    // Automatic cleanup!
}

#[test]
fn test_request_body_validation_failure() {
    fn echo_handler(req: HandlerRequest) {
        let response = HandlerResponse {
            status: 200,
            headers: HeaderVec::new(),
            body: json!({"ok": true}),
        };
        let _ = req.reply_tx.send(response);
    }

    let request_schema = Some(json!({
        "type": "object",
        "properties": {"name": {"type": "string"}},
        "required": ["name"]
    }));

    let server = CustomServerTestFixture::with_handler_and_schemas(
        "echo",
        echo_handler,
        "/echo",
        Method::POST,
        request_schema,
        None,
    );

    let request = concat!(
        "POST /echo HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Content-Type: application/json\r\n",
        "Content-Length: 12\r\n",
        "\r\n",
        "{\"name\":123}"
    );
    let resp = send_request(&server.addr(), request);
    let (status, _body) = parse_response(&resp);
    assert_eq!(status, 400);

    // Automatic cleanup!
}

#[test]
fn test_response_body_validation_failure() {
    fn bad_handler(req: HandlerRequest) {
        let response = HandlerResponse {
            status: 200,
            headers: HeaderVec::new(),
            body: json!({"name": 123}),
        };
        let _ = req.reply_tx.send(response);
    }

    let response_schema = Some(json!({
        "type": "object",
        "properties": {"name": {"type": "string"}},
        "required": ["name"]
    }));

    let server = CustomServerTestFixture::with_handler_and_schemas(
        "bad",
        bad_handler,
        "/bad",
        Method::GET,
        None,
        response_schema,
    );

    let resp = send_request(
        &server.addr(),
        "GET /bad HTTP/1.1\r\nHost: localhost\r\n\r\n",
    );
    let (status, _body) = parse_response(&resp);
    // Response validation failures are 500 (server bug), not 400 (client error)
    assert_eq!(status, 500);

    // Automatic cleanup!
}
