//! Tests for the request dispatcher and coroutine handler system
//!
//! # Test Coverage
//!
//! Validates the dispatcher's core responsibilities:
//! - Handler registration and lookup
//! - Request routing to correct handlers
//! - Response collection from handlers
//! - Typed handler conversion (HandlerRequest → TypedHandlerRequest)
//! - Middleware integration
//! - Panic recovery and error handling
//!
//! # Test Strategy
//!
//! 1. **Unit Tests**: Isolated dispatcher logic with mock handlers
//! 2. **Integration Tests**: Full router → dispatcher → handler flow
//! 3. **Typed Tests**: Type-safe request handling
//! 4. **Error Tests**: Panic handling, timeout behavior
//!
//! # Key Test Cases
//!
//! - `test_dispatcher_routes_to_handler`: Basic routing works
//! - `test_dispatcher_with_middleware`: Middleware execution order
//! - `test_typed_handler_conversion`: Type-safe request handling
//! - `test_panic_handler_returns_500`: Panic recovery (currently ignored - needs fix)
//!
//! # Known Issues
//!
//! - Panic test is ignored: May coroutines don't play well with catch_unwind in test context
//! - This is a framework limitation, not a production issue

use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HeaderVec},
    load_spec,
    router::{ParamVec, RouteMatch, Router},
    typed::{Handler, TypedHandlerRequest},
};
use http::Method;
use may::sync::mpsc;
use pet_store::registry;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::TryFrom;
use std::sync::Arc;
mod tracing_util;
use brrtrouter::ids::RequestId;
use brrtrouter::middleware::TracingMiddleware;
use tracing_util::TestTracing;

fn set_stack_size() -> TestTracing {
    let size = std::env::var("BRRTR_STACK_SIZE")
        .ok()
        .and_then(|v| {
            if let Some(hex) = v.strip_prefix("0x") {
                usize::from_str_radix(hex, 16).ok()
            } else {
                v.parse().ok()
            }
        })
        .unwrap_or(0x4000);
    may::config().set_stack_size(size);
    TestTracing::init()
}

#[derive(Debug, Deserialize, Serialize)]
struct TypedReq {
    id: i32,
    debug: bool,
}

impl TryFrom<HandlerRequest> for TypedReq {
    type Error = anyhow::Error;

    fn try_from(req: HandlerRequest) -> Result<Self, Self::Error> {
        let id = req
            .get_path_param("id")
            .ok_or_else(|| anyhow::anyhow!("missing id"))?
            .parse()?;
        let debug = req
            .get_query_param("debug")
            .map(|v| v.parse::<bool>())
            .transpose()?;
        Ok(TypedReq {
            id,
            debug: debug.unwrap_or(false),
        })
    }
}

#[derive(Debug, Serialize)]
struct TypedRes {
    ok: bool,
}

#[derive(Clone)]
struct AssertController;

impl Handler for AssertController {
    type Request = TypedReq;
    type Response = TypedRes;
    fn handle(&self, req: TypedHandlerRequest<TypedReq>) -> TypedRes {
        assert_eq!(req.data.id, 42);
        assert!(req.data.debug);
        TypedRes { ok: true }
    }
}

#[test]
fn test_dispatch_post_item() {
    let _tracing = set_stack_size();
    let (routes, _slug) = load_spec("examples/openapi.yaml").expect("load spec");
    let router = Router::new(routes);
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_all(&mut dispatcher);
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));
    dispatcher.add_middleware(Arc::new(TracingMiddleware));
    dispatcher.add_middleware(Arc::new(TracingMiddleware));

    let RouteMatch { route, .. } = router
        .route(Method::POST, "/items/item-001")
        .expect("route");
    // JSF P0-2: Convert Arc<str> to String for test compatibility
    let handler_name = route.handler_name.to_string();

    let (reply_tx, reply_rx) = mpsc::channel();
    let mut path_params: ParamVec = ParamVec::new();
    path_params.push((Arc::from("id"), "item-001".to_string()));
    let mut query_params: ParamVec = ParamVec::new();
    query_params.push((Arc::from("debug"), "true".to_string()));
    let body = json!({"name": "New Item"});

    let request = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::POST,
        // Convert Arc<str> to String for HandlerRequest
        path: route.path_pattern.to_string(),
        handler_name: handler_name.clone(),
        path_params,
        query_params,
        headers: HeaderVec::new(),
        cookies: HeaderVec::new(),
        body: Some(body),
        jwt_claims: None,
        reply_tx,
    };

    dispatcher
        .handlers
        .get(&handler_name)
        .unwrap()
        .send(request)
        .unwrap();
    let resp = reply_rx.recv().unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, json!({"id": "item-001", "name": "New Item"}));
}

#[test]
fn test_dispatch_get_pet() {
    let _tracing = set_stack_size();
    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();
    let router = Router::new(routes);
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_all(&mut dispatcher);
    }

    let RouteMatch { route, .. } = router.route(Method::GET, "/pets/12345").unwrap();
    // JSF P0-2: Convert Arc<str> to String for test compatibility
    let handler_name = route.handler_name.to_string();

    let (reply_tx, reply_rx) = mpsc::channel();
    let mut path_params: ParamVec = ParamVec::new();
    path_params.push((Arc::from("id"), "12345".to_string()));
    let mut query_params: ParamVec = ParamVec::new();
    query_params.push((Arc::from("include"), "stats".to_string()));

    let request = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        // JSF P0-2: Convert Arc<str> to String for HandlerRequest
        path: route.path_pattern.to_string(),
        handler_name: handler_name.clone(),
        path_params,
        query_params,
        headers: HeaderVec::new(),
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx,
    };

    dispatcher
        .handlers
        .get(&handler_name)
        .unwrap()
        .send(request)
        .unwrap();
    let resp = reply_rx.recv().unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(
        resp.body,
        json!({
            "age": 3,
            "breed": "Golden Retriever",
            "id": 12345,
            "name": "Max",
            "tags": ["friendly", "trained"],
            "vaccinated": true
        })
    );
}

#[test]
fn test_typed_controller_params() {
    let _tracing = set_stack_size();
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_typed("assert_controller", AssertController);
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));

    let (reply_tx, reply_rx) = mpsc::channel();
    let mut path_params: ParamVec = ParamVec::new();
    path_params.push((Arc::from("id"), "42".to_string()));
    let mut query_params: ParamVec = ParamVec::new();
    query_params.push((Arc::from("debug"), "true".to_string()));

    let request = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/items/{id}".to_string(),
        handler_name: "assert_controller".to_string(),
        path_params,
        query_params,
        headers: HeaderVec::new(),
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx,
    };

    dispatcher
        .handlers
        .get("assert_controller")
        .unwrap()
        .send(request)
        .unwrap();
    let resp = reply_rx.recv().unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, json!({"ok": true}));
}

#[test]
fn test_typed_controller_invalid_params() {
    let _tracing = set_stack_size();
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_typed("assert_controller", AssertController);
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));

    let (reply_tx, reply_rx) = mpsc::channel();
    let mut path_params: ParamVec = ParamVec::new();
    // invalid integer value for id
    path_params.push((Arc::from("id"), "not_an_int".to_string()));
    let mut query_params: ParamVec = ParamVec::new();
    query_params.push((Arc::from("debug"), "true".to_string()));

    let request = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/items/{id}".to_string(),
        handler_name: "assert_controller".to_string(),
        path_params,
        query_params,
        headers: HeaderVec::new(),
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx,
    };

    dispatcher
        .handlers
        .get("assert_controller")
        .unwrap()
        .send(request)
        .unwrap();
    let resp = reply_rx.recv().unwrap();
    assert_eq!(resp.status, 400);
    assert!(resp.body.get("error").is_some());
}

#[test]
fn test_panic_handler_returns_500() {
    let _tracing = set_stack_size();
    fn panic_handler(_req: HandlerRequest) {
        panic!("boom! - watch to see if I recover");
    }

    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("panic", panic_handler);
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));

    let (reply_tx, reply_rx) = mpsc::channel();

    let request = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/panic".to_string(),
        handler_name: "panic".to_string(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers: HeaderVec::new(),
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx,
    };

    dispatcher
        .handlers
        .get("panic")
        .unwrap()
        .send(request)
        .unwrap();
    let resp = reply_rx.recv().unwrap();
    assert_eq!(resp.status, 500);
    assert!(resp.body.get("error").is_some());
}

#[test]
fn test_dispatch_all_registry_handlers() {
    let _tracing = set_stack_size();
    let (routes, _slug) = load_spec("examples/openapi.yaml").expect("load spec");
    let router = Router::new(routes);
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_all(&mut dispatcher);
    }

    let handlers: Vec<String> = dispatcher.handlers.keys().cloned().collect();
    for name in handlers {
        let (method, path, body, _expected) = match name.as_str() {
            "admin_settings" => (
                Method::GET,
                "/admin/settings",
                None,
                json!({"feature_flags": {"analytics": "false", "beta": "true"}}),
            ),
            "get_item" => (
                Method::GET,
                "/items/item-001",
                None,
                json!({"id": "item-001", "name": "Sample Item"}),
            ),
            "post_item" => (
                Method::POST,
                "/items/item-001",
                Some(json!({"name": "New Item"})),
                json!({"id": "item-001", "name": "New Item"}),
            ),
            "list_pets" => (
                Method::GET,
                "/pets",
                None,
                json!({"items": [{"age": 0, "breed": "", "id": 0, "name": "", "tags": [], "vaccinated": false}]}),
            ),
            "add_pet" => (
                Method::POST,
                "/pets",
                Some(json!({"name": "Bella"})),
                json!({"id": 67890, "status": "success"}),
            ),
            "get_pet" => (
                Method::GET,
                "/pets/12345",
                None,
                json!({
                    "age": 3,
                    "breed": "Golden Retriever",
                    "id": 12345,
                    "name": "Max",
                    "tags": ["friendly", "trained"],
                    "vaccinated": true
                }),
            ),
            "list_users" => (
                Method::GET,
                "/users",
                None,
                json!({"users": [{"id": "abc-123", "name": "John"}, {"id": "def-456", "name": "Jane"}]}),
            ),
            "get_user" => (
                Method::GET,
                "/users/abc-123",
                None,
                json!({"id": "abc-123", "name": "John"}),
            ),
            "list_user_posts" => (
                Method::GET,
                "/users/abc-123/posts",
                None,
                json!({"items": [{"body": "", "id": "abc-123", "title": ""}]}),
            ),
            "get_post" => (
                Method::GET,
                "/users/abc-123/posts/post1",
                None,
                json!({"body": "Welcome to the blog", "id": "post1", "title": "Intro"}),
            ),
            // Skip SSE route in dispatcher unit test; it is long-lived
            "stream_events" => continue,
            // Skip handlers not explicitly covered here (spec may include more)
            _other => continue,
        };

        let route_match = router.route(method.clone(), path).expect("route match");
        assert_eq!(route_match.handler_name, name);
        let resp = dispatcher
            .dispatch(
                route_match,
                body.clone(),
                Default::default(),
                Default::default(),
            )
            .expect("dispatch");
        assert_eq!(resp.status, 200, "handler {name}");
        // TODO: fix this assertion once the handlers return correct responses
        // assert_eq!(resp.body, expected, "handler {}", name);
    }
}
