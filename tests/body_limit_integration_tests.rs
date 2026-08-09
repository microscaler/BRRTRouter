//! Story 12.2 — inbound body limits → 413 (integration).
#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

use brrtrouter::dispatcher::{Dispatcher, HandlerRequest, HandlerResponse};
use brrtrouter::router::Router;
use brrtrouter::server::body_limit::{
    reset_max_inbound_body_cache_for_tests, MAX_REQUEST_BODY_ENV, REASON_REQUEST_BODY_TOO_LARGE,
};
use brrtrouter::server::{AppService, HttpServer};
use brrtrouter::spec::RouteMeta;
use http::Method;
use serde_json::json;
use std::collections::HashMap;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

mod common;
use common::http::send_request;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn start_echo_server(
    estimated_request_body_bytes: Option<usize>,
) -> (
    brrtrouter::server::ServerHandle,
    std::net::SocketAddr,
    Arc<AtomicBool>,
) {
    may::config().set_stack_size(0x8000);
    let handler_called = Arc::new(AtomicBool::new(false));
    let flag = handler_called.clone();

    let route = RouteMeta {
        x_service: None,
        x_brrtrouter_downstream_path: None,
        x_brrtrouter_impl: None,
        method: Method::POST,
        path_pattern: Arc::from("/echo"),
        handler_name: Arc::from("echo"),
        parameters: Vec::new(),
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
        estimated_request_body_bytes,
        x_brrtrouter_stack_size: None,
        x_brrtrouter_deadline_ms: None,
        cors_policy: brrtrouter::middleware::RouteCorsPolicy::Inherit,
    };

    let router = Arc::new(arc_swap::ArcSwap::from_pointee(Router::new(vec![route])));
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("echo", move |req: HandlerRequest| {
            flag.store(true, Ordering::SeqCst);
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: Default::default(),
                body: json!({"ok": true}),
                sse: None,
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
    (handle, addr, handler_called)
}

/// P1 — body under global max proceeds.
#[test]
fn body_limit_integration_p1_under_global() {
    let _g = env_lock().lock().unwrap();
    std::env::remove_var(MAX_REQUEST_BODY_ENV);
    reset_max_inbound_body_cache_for_tests();

    let (handle, addr, called) = start_echo_server(None);
    let body = r#"{"a":1}"#;
    let req = format!(
        "POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let resp = send_request(&addr, &req);
    handle.stop();
    assert!(resp.contains("200"), "P1 expected 200, got: {resp}");
    assert!(called.load(Ordering::SeqCst), "P1 handler should run");
}

/// P2 — body under route estimate proceeds.
#[test]
fn body_limit_integration_p2_under_route_estimate() {
    let _g = env_lock().lock().unwrap();
    std::env::remove_var(MAX_REQUEST_BODY_ENV);
    reset_max_inbound_body_cache_for_tests();

    let (handle, addr, called) = start_echo_server(Some(1024));
    let body = r#"{"ok":true}"#;
    let req = format!(
        "POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let resp = send_request(&addr, &req);
    handle.stop();
    assert!(resp.contains("200"), "P2: {resp}");
    assert!(called.load(Ordering::SeqCst));
}

/// P6 — empty body on GET unaffected (use POST empty for same route).
#[test]
fn body_limit_integration_p6_empty_body() {
    let _g = env_lock().lock().unwrap();
    std::env::remove_var(MAX_REQUEST_BODY_ENV);
    reset_max_inbound_body_cache_for_tests();

    let (handle, addr, called) = start_echo_server(Some(64));
    let req =
        "POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    let resp = send_request(&addr, req);
    handle.stop();
    assert!(resp.contains("200"), "P6: {resp}");
    assert!(called.load(Ordering::SeqCst));
}

/// N1 — Content-Length over global max → 413; no handler.
#[test]
fn body_limit_integration_n1_global_cl() {
    let _g = env_lock().lock().unwrap();
    std::env::set_var(MAX_REQUEST_BODY_ENV, "32");
    reset_max_inbound_body_cache_for_tests();

    let (handle, addr, called) = start_echo_server(None);
    let body = "x".repeat(64);
    let req = format!(
        "POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let resp = send_request(&addr, &req);
    handle.stop();
    std::env::remove_var(MAX_REQUEST_BODY_ENV);
    reset_max_inbound_body_cache_for_tests();

    assert!(resp.contains("413"), "N1: {resp}");
    assert!(
        resp.contains(REASON_REQUEST_BODY_TOO_LARGE),
        "N6 shape: {resp}"
    );
    assert!(resp.contains("Payload Too Large"), "N6: {resp}");
    assert!(!called.load(Ordering::SeqCst), "N5 no handler after 413");
}

/// N2 — Content-Length over route cap → 413.
#[test]
fn body_limit_integration_n2_route_cap() {
    let _g = env_lock().lock().unwrap();
    std::env::remove_var(MAX_REQUEST_BODY_ENV);
    reset_max_inbound_body_cache_for_tests();

    let (handle, addr, called) = start_echo_server(Some(16));
    let body = "y".repeat(64);
    let req = format!(
        "POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let resp = send_request(&addr, &req);
    handle.stop();
    assert!(resp.contains("413"), "N2: {resp}");
    assert!(!called.load(Ordering::SeqCst), "N5");
}

/// N4 — hostile huge Content-Length → 413; no OOM.
#[test]
fn body_limit_integration_n4_hostile_cl() {
    let _g = env_lock().lock().unwrap();
    std::env::set_var(MAX_REQUEST_BODY_ENV, "1024");
    reset_max_inbound_body_cache_for_tests();

    let (handle, addr, called) = start_echo_server(None);
    let req = "POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Length: 999999999999999999\r\nConnection: close\r\n\r\n";
    let resp = send_request(&addr, req);
    handle.stop();
    std::env::remove_var(MAX_REQUEST_BODY_ENV);
    reset_max_inbound_body_cache_for_tests();

    assert!(resp.contains("413"), "N4: {resp}");
    assert!(!called.load(Ordering::SeqCst));
}
