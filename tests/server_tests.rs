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

fn start_petstore_service() -> (TestTracing, ServerHandle, SocketAddr) {
    // ensure coroutines have enough stack for tests
    may::config().set_stack_size(0x8000);
    let tracing = TestTracing::init();
    let (routes, schemes, _slug) = brrtrouter::load_spec_full("examples/openapi.yaml").unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));
    let mut service = AppService::new(
        router,
        Arc::new(RwLock::new(dispatcher)),
        schemes,
        PathBuf::from("examples/openapi.yaml"),
        None,
        None,
    );
    // Register a simple ApiKey provider to satisfy spec security in tests
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
    let (_tracing, handle, addr) = start_petstore_service();
    let resp = send_request(
        &addr,
        "GET /pets HTTP/1.1\r\nHost: localhost\r\nX-API-Key: test123\r\n\r\n",
    );
    handle.stop();
    let (status, body) = parse_response(&resp);
    assert_eq!(status, 200);
    assert!(body.is_array());
}

#[test]
fn test_route_404() {
    let (_tracing, handle, addr) = start_petstore_service();
    let resp = send_request(&addr, "GET /nope HTTP/1.1\r\nHost: localhost\r\n\r\n");
    handle.stop();
    let (status, _body) = parse_response(&resp);
    assert_eq!(status, 404);
}

#[test]
fn test_panic_recovery() {
    may::config().set_stack_size(0x8000);
    let _tracing = TestTracing::init();
    fn panic_handler(_req: HandlerRequest) {
        panic!("boom");
    }
    let route = RouteMeta {
        method: Method::GET,
        path_pattern: "/panic".to_string(),
        handler_name: "panic".to_string(),
        parameters: Vec::new(),
        request_schema: None,
        request_body_required: false,
        response_schema: None,
        example: None,
        responses: std::collections::HashMap::new(),
        security: Vec::new(),
        example_name: String::new(),
        project_slug: String::new(),
        output_dir: PathBuf::new(),
        base_path: String::new(),
        sse: false,
    };
    let router = Arc::new(RwLock::new(Router::new(vec![route])));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("panic", panic_handler);
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

    let resp = send_request(&addr, "GET /panic HTTP/1.1\r\nHost: localhost\r\n\r\n");
    handle.stop();
    let (status, body) = parse_response(&resp);
    assert_eq!(status, 500);
    assert!(body.get("error").is_some());
}

#[test]
fn test_headers_and_cookies() {
    may::config().set_stack_size(0x8000);
    let _tracing = TestTracing::init();
    fn header_handler(req: HandlerRequest) {
        let response = HandlerResponse {
            status: 200,
            headers: HashMap::new(),
            body: json!({
                "headers": req.headers,
                "cookies": req.cookies,
            }),
        };
        let _ = req.reply_tx.send(response);
    }

    let route = RouteMeta {
        method: Method::GET,
        path_pattern: "/headertest".to_string(),
        handler_name: "header".to_string(),
        parameters: Vec::new(),
        request_schema: None,
        request_body_required: false,
        response_schema: None,
        example: None,
        responses: std::collections::HashMap::new(),
        security: Vec::new(),
        example_name: String::new(),
        project_slug: String::new(),
        output_dir: PathBuf::new(),
        base_path: String::new(),
        sse: false,
    };
    let router = Arc::new(RwLock::new(Router::new(vec![route])));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("header", header_handler);
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

    let request = concat!(
        "GET /headertest HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "X-Test: value\r\n",
        "X-Other: foo\r\n",
        "Cookie: session=abc123; theme=dark\r\n",
        "\r\n"
    );
    let resp = send_request(&addr, request);
    handle.stop();
    let (status, body) = parse_response(&resp);
    assert_eq!(status, 200);
    assert_eq!(body["headers"]["x-test"], "value");
    assert_eq!(body["headers"]["x-other"], "foo");
    assert_eq!(body["cookies"]["session"], "abc123");
    assert_eq!(body["cookies"]["theme"], "dark");
}

#[test]
fn test_status_201_json() {
    may::config().set_stack_size(0x8000);
    let _tracing = TestTracing::init();
    fn create_handler(req: HandlerRequest) {
        let response = HandlerResponse {
            status: 201,
            headers: HashMap::new(),
            body: json!({"created": true}),
        };
        let _ = req.reply_tx.send(response);
    }

    let route = RouteMeta {
        method: Method::POST,
        path_pattern: "/created".to_string(),
        handler_name: "create".to_string(),
        parameters: Vec::new(),
        request_schema: None,
        request_body_required: false,
        response_schema: None,
        example: None,
        responses: std::collections::HashMap::new(),
        security: Vec::new(),
        example_name: String::new(),
        project_slug: String::new(),
        output_dir: PathBuf::new(),
        base_path: String::new(),
        sse: false,
    };
    let router = Arc::new(RwLock::new(Router::new(vec![route])));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("create", create_handler);
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

    let resp = send_request(&addr, "POST /created HTTP/1.1\r\nHost: localhost\r\n\r\n");
    handle.stop();
    let (status, body) = parse_response(&resp);
    let (_, ct, _) = parse_response_parts(&resp);
    assert_eq!(status, 201);
    assert_eq!(ct, "application/json");
    assert_eq!(body["created"], true);
}

#[test]
fn test_text_plain_error() {
    may::config().set_stack_size(0x8000);
    let _tracing = TestTracing::init();
    fn text_handler(req: HandlerRequest) {
        let response = HandlerResponse {
            status: 400,
            headers: HashMap::new(),
            body: json!("bad request"),
        };
        let _ = req.reply_tx.send(response);
    }

    let route = RouteMeta {
        method: Method::GET,
        path_pattern: "/text".to_string(),
        handler_name: "text".to_string(),
        parameters: Vec::new(),
        request_schema: None,
        request_body_required: false,
        response_schema: None,
        example: None,
        responses: std::collections::HashMap::new(),
        security: Vec::new(),
        example_name: String::new(),
        project_slug: String::new(),
        output_dir: PathBuf::new(),
        base_path: String::new(),
        sse: false,
    };
    let router = Arc::new(RwLock::new(Router::new(vec![route])));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("text", text_handler);
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

    let resp = send_request(&addr, "GET /text HTTP/1.1\r\nHost: localhost\r\n\r\n");
    handle.stop();
    let (status, body) = parse_response(&resp);
    let (_, ct, raw_body) = parse_response_parts(&resp);
    assert_eq!(status, 400);
    assert_eq!(ct, "text/plain");
    assert_eq!(raw_body, "bad request");
    assert_eq!(body, Value::String("bad request".to_string()));
}

#[test]
fn test_request_body_validation_failure() {
    may::config().set_stack_size(0x8000);
    let _tracing = TestTracing::init();
    fn echo_handler(req: HandlerRequest) {
        let response = HandlerResponse {
            status: 200,
            headers: HashMap::new(),
            body: json!({"ok": true}),
        };
        let _ = req.reply_tx.send(response);
    }

    let route = RouteMeta {
        method: Method::POST,
        path_pattern: "/echo".to_string(),
        handler_name: "echo".to_string(),
        parameters: Vec::new(),
        request_schema: Some(json!({
            "type": "object",
            "properties": {"name": {"type": "string"}},
            "required": ["name"]
        })),
        request_body_required: false,
        response_schema: None,
        example: None,
        responses: std::collections::HashMap::new(),
        security: Vec::new(),
        example_name: String::new(),
        project_slug: String::new(),
        output_dir: PathBuf::new(),
        base_path: String::new(),
        sse: false,
    };
    let router = Arc::new(RwLock::new(Router::new(vec![route])));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("echo", echo_handler);
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

    let request = concat!(
        "POST /echo HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Content-Type: application/json\r\n",
        "Content-Length: 12\r\n",
        "\r\n",
        "{\"name\":123}"
    );
    let resp = send_request(&addr, request);
    handle.stop();
    let (status, _body) = parse_response(&resp);
    assert_eq!(status, 400);
}

#[test]
fn test_response_body_validation_failure() {
    may::config().set_stack_size(0x8000);
    let _tracing = TestTracing::init();
    fn bad_handler(req: HandlerRequest) {
        let response = HandlerResponse {
            status: 200,
            headers: HashMap::new(),
            body: json!({"name": 123}),
        };
        let _ = req.reply_tx.send(response);
    }

    let route = RouteMeta {
        method: Method::GET,
        path_pattern: "/bad".to_string(),
        handler_name: "bad".to_string(),
        parameters: Vec::new(),
        request_schema: None,
        request_body_required: false,
        response_schema: Some(json!({
            "type": "object",
            "properties": {"name": {"type": "string"}},
            "required": ["name"]
        })),
        example: None,
        responses: std::collections::HashMap::new(),
        security: Vec::new(),
        example_name: String::new(),
        project_slug: String::new(),
        output_dir: PathBuf::new(),
        base_path: String::new(),
        sse: false,
    };
    let router = Arc::new(RwLock::new(Router::new(vec![route])));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("bad", bad_handler);
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

    let resp = send_request(&addr, "GET /bad HTTP/1.1\r\nHost: localhost\r\n\r\n");
    handle.stop();
    let (status, _body) = parse_response(&resp);
    assert_eq!(status, 400);
}
