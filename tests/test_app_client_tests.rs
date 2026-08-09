//! Epic 13.10 — TestApp / RequestBuilder contract tests (P1–P4, N1–N3, N5).
//!
//! Integration tests (not `#[cfg(test)]` in the lib): mirrors the
//! `server_tests::CustomServerTestFixture` AppService shape so may stack
//! sizing matches known-good in-process servers.
#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

mod tracing_util;

use arc_swap::ArcSwap;
use brrtrouter::dispatcher::{Dispatcher, HandlerRequest, HandlerResponse, HeaderVec};
use brrtrouter::middleware::TracingMiddleware;
use brrtrouter::router::Router;
use brrtrouter::server::AppService;
use brrtrouter::spec::RouteMeta;
use brrtrouter::test_support::{TestApp, TestAppError};
use http::Method;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing_util::TestTracing;

/// Serialize AppService servers in this binary under parallel `cargo test`.
static HTTP_TEST_LOCK: Mutex<()> = Mutex::new(());

fn route(method: Method, path: &str, handler: &str, request_schema: Option<Value>) -> RouteMeta {
    RouteMeta {
        x_service: None,
        x_brrtrouter_downstream_path: None,
        x_brrtrouter_impl: None,
        method,
        path_pattern: Arc::from(path),
        handler_name: Arc::from(handler),
        parameters: Vec::new(),
        request_body_required: request_schema.is_some(),
        request_content_types: if request_schema.is_some() {
            vec!["application/json".into()]
        } else {
            Vec::new()
        },
        request_schema,
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
        x_brrtrouter_deadline_ms: None,
        cors_policy: brrtrouter::middleware::RouteCorsPolicy::Inherit,
    }
}

fn with_app(
    routes: Vec<RouteMeta>,
    register: impl FnOnce(&mut Dispatcher),
    f: impl FnOnce(&TestApp),
) {
    let _lock = HTTP_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    may::config().set_stack_size(0x8000);
    let _tracing = TestTracing::init();
    let router = Arc::new(ArcSwap::from_pointee(Router::new(routes)));
    let mut dispatcher = Dispatcher::new();
    register(&mut dispatcher);
    dispatcher.add_middleware(Arc::new(TracingMiddleware));
    let service = AppService::new(
        router,
        Arc::new(ArcSwap::from_pointee(dispatcher)),
        HashMap::new(),
        PathBuf::from("examples/openapi.yaml"),
        Some(PathBuf::from("examples/pet_store/static_site")),
        Some(PathBuf::from("examples/pet_store/doc")),
    );
    let app = TestApp::from_service(service).expect("test app starts");
    f(&app);
}

#[test]
fn p1_get_known_route_returns_200_and_body() {
    with_app(
        vec![route(Method::GET, "/hello", "hello", None)],
        |d| {
            // SAFETY: test-only closure handler
            unsafe {
                d.register_handler("hello", |req: HandlerRequest| {
                    let _ = req.reply_tx.send(HandlerResponse {
                        status: 200,
                        headers: HeaderVec::new(),
                        body: json!({"ok": true}),
                        sse: None,
                    });
                });
            }
        },
        |app| {
            let res = app.get("/hello").send().unwrap();
            assert_eq!(res.status, 200);
            assert_eq!(res.json_value().unwrap()["ok"], true);
        },
    );
}

#[test]
fn p2_post_json_round_trip() {
    let schema = json!({
        "type": "object",
        "properties": { "name": { "type": "string" } },
        "required": ["name"]
    });
    with_app(
        vec![route(Method::POST, "/echo", "echo", Some(schema))],
        |d| unsafe {
            d.register_handler("echo", |req: HandlerRequest| {
                let body = req.body.clone().unwrap_or(Value::Null);
                let _ = req.reply_tx.send(HandlerResponse {
                    status: 200,
                    headers: HeaderVec::new(),
                    body,
                    sse: None,
                });
            });
        },
        |app| {
            let res = app
                .post("/echo")
                .json(&json!({"name": "widget"}))
                .unwrap()
                .send()
                .unwrap();
            assert_eq!(res.status, 200);
            assert_eq!(res.json_value().unwrap()["name"], "widget");
        },
    );
}

#[test]
fn p3_custom_header_forwarded() {
    with_app(
        vec![route(Method::GET, "/h", "h", None)],
        |d| unsafe {
            d.register_handler("h", |req: HandlerRequest| {
                let v = req
                    .headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("x-trace"))
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_default();
                let _ = req.reply_tx.send(HandlerResponse {
                    status: 200,
                    headers: HeaderVec::new(),
                    body: json!({"trace": v}),
                    sse: None,
                });
            });
        },
        |app| {
            let res = app.get("/h").header("X-Trace", "abc").send().unwrap();
            assert_eq!(res.status, 200);
            assert_eq!(res.json_value().unwrap()["trace"], "abc");
        },
    );
}

#[test]
fn p4_cookie_set_observed() {
    with_app(
        vec![route(Method::GET, "/c", "c", None)],
        |d| unsafe {
            d.register_handler("c", |req: HandlerRequest| {
                let v = req.get_cookie("sid").unwrap_or("").to_string();
                let _ = req.reply_tx.send(HandlerResponse {
                    status: 200,
                    headers: HeaderVec::new(),
                    body: json!({"sid": v}),
                    sse: None,
                });
            });
        },
        |app| {
            let res = app.get("/c").cookie("sid", "s3cr3t").send().unwrap();
            assert_eq!(res.status, 200);
            assert_eq!(res.json_value().unwrap()["sid"], "s3cr3t");
        },
    );
}

#[test]
fn n1_unknown_path_is_404() {
    with_app(
        vec![route(Method::GET, "/only", "only", None)],
        |d| unsafe {
            d.register_handler("only", |req: HandlerRequest| {
                let _ = req.reply_tx.send(HandlerResponse {
                    status: 200,
                    headers: HeaderVec::new(),
                    body: json!({}),
                    sse: None,
                });
            });
        },
        |app| {
            let res = app.get("/missing").send().unwrap();
            assert_eq!(res.status, 404);
        },
    );
}

#[test]
fn n2_invalid_json_body_is_client_error() {
    let schema = json!({
        "type": "object",
        "properties": { "name": { "type": "string" } },
        "required": ["name"]
    });
    with_app(
        vec![route(Method::POST, "/echo", "echo", Some(schema))],
        |d| unsafe {
            d.register_handler("echo", |req: HandlerRequest| {
                let _ = req.reply_tx.send(HandlerResponse {
                    status: 200,
                    headers: HeaderVec::new(),
                    body: json!({}),
                    sse: None,
                });
            });
        },
        |app| {
            // Type error (name must be string) — same fixture as server_tests body validation.
            let res = app
                .post("/echo")
                .json(&json!({"name": 123}))
                .unwrap()
                .send()
                .unwrap();
            assert!(
                (400..500).contains(&res.status),
                "expected 4xx, got {}",
                res.status
            );
        },
    );
}

#[test]
fn n3_empty_path_is_err() {
    with_app(
        vec![route(Method::GET, "/x", "x", None)],
        |d| unsafe {
            d.register_handler("x", |req: HandlerRequest| {
                let _ = req.reply_tx.send(HandlerResponse {
                    status: 200,
                    headers: HeaderVec::new(),
                    body: json!({}),
                    sse: None,
                });
            });
        },
        |app| {
            let err = app.get("").send().unwrap_err();
            assert!(matches!(err, TestAppError::EmptyPath));
        },
    );
}

#[test]
fn n5_authorization_redacted_in_debug() {
    with_app(
        vec![route(Method::GET, "/x", "x", None)],
        |_| {},
        |app| {
            let builder = app.get("/x").header("Authorization", "Bearer secret-token");
            let dbg = format!("{builder:?}");
            assert!(dbg.contains("<redacted>"));
            assert!(!dbg.contains("secret-token"));
        },
    );
}
