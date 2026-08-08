//! Story 13.6 — handler / request deadlines → 504 (integration).
#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

use brrtrouter::dispatcher::{
    deadline_exceeded_response, Dispatcher, HandlerRequest, HandlerResponse, HeaderVec,
    REASON_HANDLER_DEADLINE_EXCEEDED,
};
use brrtrouter::router::RouteMatch;
use brrtrouter::spec::RouteMeta;
use http::Method;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn route_meta(handler: &str, deadline_ms: Option<u64>) -> RouteMeta {
    RouteMeta {
        x_service: None,
        x_brrtrouter_downstream_path: None,
        x_brrtrouter_impl: None,
        method: Method::GET,
        path_pattern: Arc::from("/ping"),
        handler_name: Arc::from(handler),
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
        estimated_request_body_bytes: None,
        x_brrtrouter_stack_size: None,
        x_brrtrouter_deadline_ms: deadline_ms,
        cors_policy: brrtrouter::middleware::RouteCorsPolicy::Inherit,
    }
}

fn route_match(meta: RouteMeta) -> RouteMatch {
    let handler_name = meta.handler_name.to_string();
    RouteMatch {
        route: Arc::new(meta),
        path_params: Default::default(),
        handler_name,
        query_params: Default::default(),
    }
}

fn ensure_may() {
    // Need at least one may worker OS thread so handlers progress while the
    // test thread blocks in `recv_timeout`.
    may::config().set_stack_size(0x8000);
    may::config().set_workers(2);
}

/// P1 — fast handler under deadline returns 200.
#[test]
fn handler_deadline_p1_fast_under_limit() {
    ensure_may();
    let mut dispatcher = Dispatcher::new();
    dispatcher.set_handler_deadline_ms(Some(500));
    unsafe {
        dispatcher.register_handler("fast", |req: HandlerRequest| {
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: Default::default(),
                body: json!({"ok": true}),
            });
        });
    }
    let resp = dispatcher
        .dispatch(
            route_match(route_meta("fast", None)),
            None,
            HeaderVec::new(),
            HeaderVec::new(),
        )
        .expect("response");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body["ok"], true);
}

/// P2 / N3 / N4 — slow handler past deadline → 504 (not 200, not hang).
#[test]
fn handler_deadline_p2_slow_returns_504() {
    ensure_may();
    let mut dispatcher = Dispatcher::new();
    dispatcher.set_handler_deadline_ms(Some(50));
    unsafe {
        dispatcher.register_handler("slow", |req: HandlerRequest| {
            thread::sleep(Duration::from_millis(300));
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: Default::default(),
                body: json!({"late": true}),
            });
        });
    }
    let started = std::time::Instant::now();
    let resp = dispatcher
        .dispatch(
            route_match(route_meta("slow", None)),
            None,
            HeaderVec::new(),
            HeaderVec::new(),
        )
        .expect("response");
    let elapsed = started.elapsed();
    assert_eq!(resp.status, 504, "N4: timeout must not return 200");
    assert_eq!(resp.body["reason"], REASON_HANDLER_DEADLINE_EXCEEDED);
    assert!(
        elapsed < Duration::from_millis(250),
        "N3: must not wait for slow handler (elapsed {elapsed:?})"
    );
}

/// P3 — timeout callback (metrics sink) fires once.
#[test]
fn handler_deadline_p3_metric_callback() {
    ensure_may();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_cb = hits.clone();
    let mut dispatcher = Dispatcher::new();
    dispatcher.set_handler_deadline_ms(Some(40));
    dispatcher.set_on_deadline_timeout(Some(Arc::new(move || {
        hits_cb.fetch_add(1, Ordering::SeqCst);
    })));
    unsafe {
        dispatcher.register_handler("slow_metric", |req: HandlerRequest| {
            thread::sleep(Duration::from_millis(200));
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: Default::default(),
                body: json!({}),
            });
        });
    }
    let resp = dispatcher
        .dispatch(
            route_match(route_meta("slow_metric", None)),
            None,
            HeaderVec::new(),
            HeaderVec::new(),
        )
        .expect("response");
    assert_eq!(resp.status, 504);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

/// P4 — disabled deadline: slow handler still returns 200.
#[test]
fn handler_deadline_p4_disabled_waits() {
    ensure_may();
    let mut dispatcher = Dispatcher::new();
    dispatcher.set_handler_deadline_ms(None);
    unsafe {
        dispatcher.register_handler("slow_ok", |req: HandlerRequest| {
            thread::sleep(Duration::from_millis(80));
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: Default::default(),
                body: json!({"ok": true}),
            });
        });
    }
    let resp = dispatcher
        .dispatch(
            route_match(route_meta("slow_ok", None)),
            None,
            HeaderVec::new(),
            HeaderVec::new(),
        )
        .expect("response");
    assert_eq!(resp.status, 200);
}

/// P6 — problem JSON shape matches shared deadline helper.
#[test]
fn handler_deadline_p6_problem_shape() {
    let expected = deadline_exceeded_response();
    ensure_may();
    let mut dispatcher = Dispatcher::new();
    dispatcher.set_handler_deadline_ms(Some(30));
    unsafe {
        dispatcher.register_handler("shape", |req: HandlerRequest| {
            thread::sleep(Duration::from_millis(200));
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: Default::default(),
                body: json!({}),
            });
        });
    }
    let resp = dispatcher
        .dispatch(
            route_match(route_meta("shape", None)),
            None,
            HeaderVec::new(),
            HeaderVec::new(),
        )
        .expect("response");
    assert_eq!(resp.status, expected.status);
    assert_eq!(resp.body["status"], 504);
    assert_eq!(resp.body["reason"], REASON_HANDLER_DEADLINE_EXCEEDED);
    assert!(!resp.body.to_string().contains("secret"));
}

/// Route-only override enables deadline when global is off.
#[test]
fn handler_deadline_route_override_alone() {
    ensure_may();
    let mut dispatcher = Dispatcher::new();
    dispatcher.set_handler_deadline_ms(None);
    unsafe {
        dispatcher.register_handler("route_slow", |req: HandlerRequest| {
            thread::sleep(Duration::from_millis(200));
            let _ = req.reply_tx.send(HandlerResponse {
                status: 200,
                headers: Default::default(),
                body: json!({}),
            });
        });
    }
    let resp = dispatcher
        .dispatch(
            route_match(route_meta("route_slow", Some(40))),
            None,
            HeaderVec::new(),
            HeaderVec::new(),
        )
        .expect("response");
    assert_eq!(resp.status, 504);
}
