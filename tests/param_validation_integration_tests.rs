//! Story 12.4 — pre-handler param validation integration.
#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

use brrtrouter::dispatcher::{Dispatcher, HandlerRequest, HandlerResponse};
use brrtrouter::router::Router;
use brrtrouter::server::param_validation::REASON_PARAMETER_VALIDATION_FAILED;
use brrtrouter::server::{AppService, HttpServer};
use brrtrouter::spec::{ParameterLocation, ParameterMeta, RouteMeta};
use http::Method;
use serde_json::json;
use std::collections::HashMap;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod common;
use common::http::send_request;

fn start_server(
    parameters: Vec<ParameterMeta>,
) -> (
    brrtrouter::server::ServerHandle,
    std::net::SocketAddr,
    Arc<AtomicBool>,
) {
    may::config().set_stack_size(0x8000);
    let called = Arc::new(AtomicBool::new(false));
    let flag = called.clone();

    let route = RouteMeta {
        x_service: None,
        x_brrtrouter_downstream_path: None,
        x_brrtrouter_impl: None,
        method: Method::GET,
        path_pattern: Arc::from("/search"),
        handler_name: Arc::from("search"),
        parameters,
        request_schema: None,
        request_body_required: false,
        request_content_types: Vec::new(),
        response_schema: None,
        example: None,
        responses: HashMap::new(),
        security: Vec::new(),
        example_name: String::new(),
        project_slug: String::new(),
        output_dir: PathBuf::new(),
        base_path: String::new(),
        sse: false,
        estimated_request_body_bytes: None,
        x_brrtrouter_stack_size: None,
        cors_policy: brrtrouter::middleware::RouteCorsPolicy::Inherit,
    };

    let router = Arc::new(arc_swap::ArcSwap::from_pointee(Router::new(vec![route])));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("search", move |req: HandlerRequest| {
            flag.store(true, Ordering::SeqCst);
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: Default::default(),
                body: json!({"ok": true}),
            });
        });
    }

    let service = AppService::new(
        router,
        Arc::new(arc_swap::ArcSwap::from_pointee(dispatcher)),
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
    (handle, addr, called)
}

fn q_param(name: &str, required: bool, ty: Option<&str>) -> ParameterMeta {
    ParameterMeta {
        name: name.to_string(),
        location: ParameterLocation::Query,
        required,
        schema: ty.map(|t| json!({"type": t})),
        style: None,
        explode: None,
    }
}

fn h_param(name: &str, required: bool) -> ParameterMeta {
    ParameterMeta {
        name: name.to_string(),
        location: ParameterLocation::Header,
        required,
        schema: None,
        style: None,
        explode: None,
    }
}

/// P1 — all required query present → handler.
#[test]
fn param_integration_p1_required_query_ok() {
    let (handle, addr, called) = start_server(vec![q_param("q", true, Some("string"))]);
    let resp = send_request(
        &addr,
        "GET /search?q=hello HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    handle.stop();
    assert!(resp.contains("200"), "{resp}");
    assert!(called.load(Ordering::SeqCst));
}

/// N1 — missing required query → 400; no handler.
#[test]
fn param_integration_n1_missing_query() {
    let (handle, addr, called) = start_server(vec![q_param("q", true, None)]);
    let resp = send_request(
        &addr,
        "GET /search HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    handle.stop();
    assert!(resp.contains("400"), "{resp}");
    assert!(resp.contains(REASON_PARAMETER_VALIDATION_FAILED), "{resp}");
    assert!(!called.load(Ordering::SeqCst));
}

/// N2 — missing required header → 400.
#[test]
fn param_integration_n2_missing_header() {
    let (handle, addr, called) = start_server(vec![h_param("X-Client", true)]);
    let resp = send_request(
        &addr,
        "GET /search HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    handle.stop();
    assert!(resp.contains("400"), "{resp}");
    assert!(!called.load(Ordering::SeqCst));
}

/// P3 — required header present.
#[test]
fn param_integration_p3_header_ok() {
    let (handle, addr, called) = start_server(vec![h_param("X-Client", true)]);
    let resp = send_request(
        &addr,
        "GET /search HTTP/1.1\r\nHost: localhost\r\nX-Client: web\r\nConnection: close\r\n\r\n",
    );
    handle.stop();
    assert!(resp.contains("200"), "{resp}");
    assert!(called.load(Ordering::SeqCst));
}
