use brrtrouter::dispatcher::Dispatcher;
use brrtrouter::router::Router;
use brrtrouter::server::AppService;
use brrtrouter::server::{HttpServer, ServerHandle};
use brrtrouter::spec::SecurityScheme;
use brrtrouter::{SecurityProvider, SecurityRequest};
use pet_store::registry;

use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

mod tracing_util;
use brrtrouter::middleware::TracingMiddleware;
use tracing_util::TestTracing;
mod common;
use common::http::send_request;

/// Test fixture for SSE (Server-Sent Events) tests with automatic setup and teardown using RAII
///
/// This fixture tests the `/events` endpoint with proper authentication.
struct SseTestServer {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}

impl SseTestServer {
    fn new() -> Self {
        std::env::set_var("BRRTR_STACK_SIZE", "0x8000");
        let config = brrtrouter::runtime_config::RuntimeConfig::from_env();
        may::config().set_stack_size(config.stack_size);
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
            Some(PathBuf::from("examples/pet_store/static_site")),
            Some(PathBuf::from("examples/pet_store/doc")),
        );

        // Register ApiKey provider so /events (secured) can be accessed in test
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

        Self {
            _tracing: tracing,
            handle: Some(handle),
            addr,
        }
    }

    fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for SseTestServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
    }
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
    let server = SseTestServer::new();
    let resp = send_request(
        &server.addr(),
        "GET /events HTTP/1.1\r\nHost: localhost\r\nX-API-Key: test123\r\n\r\n",
    );
    let (status, ct, body) = parse_parts(&resp);
    assert_eq!(status, 200);
    assert_eq!(ct, "text/event-stream");
    let events: Vec<_> = body
        .lines()
        .filter(|l| l.starts_with("data: "))
        .map(|l| l[6..].trim())
        .collect();
    assert_eq!(events, ["tick 0", "tick 1", "tick 2"]);

    // Automatic cleanup!
}
