#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]
//! The application request hook: runs before routing and security, can set
//! headers the route's security then sees, and can answer a request itself.

use brrtrouter::dispatcher::{Dispatcher, HandlerResponse};
use brrtrouter::router::Router;
use brrtrouter::server::{AppService, HttpServer, RequestHook, ServerHandle};
use brrtrouter::spec::SecurityScheme;
use brrtrouter::{SecurityProvider, SecurityRequest};
use pet_store::registry;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;

mod tracing_util;
use tracing_util::TestTracing;

mod common;
use common::http::send_request;

struct Fixture {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.stop();
        }
    }
}

/// Accepts the pet store's API key only.
struct ApiKeyProvider;

impl SecurityProvider for ApiKeyProvider {
    fn validate(&self, scheme: &SecurityScheme, _scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::ApiKey { name, .. } => req
                .get_header(&name.to_ascii_lowercase())
                .is_some_and(|v| v == "test123"),
            _ => false,
        }
    }
}

fn start(hook: Option<RequestHook>) -> Fixture {
    std::env::set_var("BRRTR_STACK_SIZE", "0x8000");
    let config = brrtrouter::runtime_config::RuntimeConfig::from_env();
    may::config().set_stack_size(config.stack_size);
    let tracing = TestTracing::init();
    let (routes, schemes, _slug) = brrtrouter::load_spec_full("examples/openapi.yaml").unwrap();
    let router = Arc::new(arc_swap::ArcSwap::from_pointee(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    let mut service = AppService::new(
        router,
        Arc::new(arc_swap::ArcSwap::from_pointee(dispatcher)),
        schemes,
        PathBuf::from("examples/openapi.yaml"),
        Some(PathBuf::from("examples/pet_store/static_site")),
        Some(PathBuf::from("examples/pet_store/doc")),
    );
    for (name, scheme) in service.security_schemes.clone() {
        if matches!(scheme, SecurityScheme::ApiKey { .. }) {
            service.register_security_provider(&name, Arc::new(ApiKeyProvider));
        }
    }
    service.set_request_hook(hook);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let handle = HttpServer(service).start(addr).unwrap();
    handle.wait_ready().unwrap();
    Fixture {
        _tracing: tracing,
        handle: Some(handle),
        addr,
    }
}

fn status(resp: &str) -> u16 {
    resp.lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// The BFF shape: a session cookie becomes the credential the route requires.
fn session_hook() -> RequestHook {
    Arc::new(|ctx| {
        if ctx.path == "/hooked" {
            let echo = ctx.query_param("echo").unwrap_or("").to_string();
            return Some(HandlerResponse::json(
                418,
                serde_json::json!({ "answered": "by the hook", "echo": echo }),
            ));
        }
        if ctx.cookie("session") == Some("good") {
            ctx.set_header("X-API-Key", "test123");
        } else {
            // A browser never gets to present the key itself.
            ctx.remove_header("x-api-key");
        }
        None
    })
}

fn get(f: &Fixture, path: &str, extra: &str) -> String {
    let port = f.addr.port();
    send_request(
        &f.addr,
        &format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{extra}\r\n"),
    )
}

#[test]
fn hook_sets_the_credential_route_security_checks() {
    let f = start(Some(session_hook()));
    assert_eq!(status(&get(&f, "/pets", "Cookie: session=good\r\n")), 200);
    assert_eq!(status(&get(&f, "/pets", "Cookie: session=bad\r\n")), 401);
    assert_eq!(status(&get(&f, "/pets", "")), 401);
}

#[test]
fn hook_can_remove_headers_before_security() {
    let f = start(Some(session_hook()));
    assert_eq!(status(&get(&f, "/pets", "X-API-Key: test123\r\n")), 401);
}

#[test]
fn hook_can_answer_a_request() {
    let f = start(Some(session_hook()));
    let resp = get(&f, "/hooked?echo=a%20b", "");
    assert_eq!(status(&resp), 418, "{resp}");
    assert!(resp.contains("by the hook"), "{resp}");
    assert!(resp.contains("\"echo\":\"a b\""), "query decoded: {resp}");
}

#[test]
fn builtin_endpoints_do_not_reach_the_hook() {
    let f = start(Some(Arc::new(|_ctx| {
        Some(HandlerResponse::json(500, serde_json::json!({})))
    })));
    assert_eq!(status(&get(&f, "/health", "")), 200);
    assert_eq!(status(&get(&f, "/pets", "X-API-Key: test123\r\n")), 500);
}

#[test]
fn without_a_hook_nothing_changes() {
    let f = start(None);
    assert_eq!(status(&get(&f, "/pets", "X-API-Key: test123\r\n")), 200);
    assert_eq!(status(&get(&f, "/pets", "")), 401);
}
