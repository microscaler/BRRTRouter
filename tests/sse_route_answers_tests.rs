#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]
//! An `x-sse` route that answers without a stream (a proxied downstream error,
//! a refusal) is written with its own Content-Type, not framed as an event stream.

use brrtrouter::dispatcher::{Dispatcher, HandlerRequest, HandlerResponse, HeaderVec};
use brrtrouter::router::Router;
use brrtrouter::server::{AppService, HttpServer, ServerHandle};
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

/// The pet store, with `stream_events` (`GET /events`, `x-sse: true`) answering `answer`.
fn start(answer: HandlerResponse) -> Fixture {
    start_with(move || answer.clone())
}

/// The pet store, with `stream_events` answering whatever `answer` builds per request.
fn start_with(answer: impl Fn() -> HandlerResponse + Send + Clone + 'static) -> Fixture {
    std::env::set_var("BRRTR_STACK_SIZE", "0x40000");
    let config = brrtrouter::runtime_config::RuntimeConfig::from_env();
    may::config().set_stack_size(config.stack_size);
    let tracing = TestTracing::init();
    let (routes, schemes, _slug) = brrtrouter::load_spec_full("examples/openapi.yaml").unwrap();
    let router = Arc::new(arc_swap::ArcSwap::from_pointee(Router::new(routes.clone())));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
        dispatcher.register_handler("stream_events", move |req: HandlerRequest| {
            let _ = req.reply_tx.send(answer());
        });
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

fn get_events(f: &Fixture) -> String {
    let port = f.addr.port();
    send_request(
        &f.addr,
        &format!("GET /events HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-API-Key: test123\r\n\r\n"),
    )
}

fn content_type(resp: &str) -> String {
    resp.lines()
        .take_while(|l| !l.is_empty())
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case("content-type")
                .then(|| v.trim().to_ascii_lowercase())
        })
        .unwrap_or_default()
}

#[test]
fn a_non_stream_answer_keeps_its_content_type() {
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("content-type"), "application/json".to_string()));
    let f = start(HandlerResponse::new(
        401,
        headers,
        serde_json::json!({ "error": "unauthorized" }),
    ));
    let resp = get_events(&f);
    assert!(resp.starts_with("HTTP/1.1 401"), "{resp}");
    assert!(
        content_type(&resp).starts_with("application/json"),
        "{resp}"
    );
    assert!(resp.contains("unauthorized"), "{resp}");
}

#[test]
fn a_buffered_event_body_is_still_an_event_stream() {
    let f = start(HandlerResponse::json(
        200,
        serde_json::Value::String("data: tick\n\n".to_string()),
    ));
    let resp = get_events(&f);
    assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
    assert!(
        content_type(&resp).starts_with("text/event-stream"),
        "{resp}"
    );
}
