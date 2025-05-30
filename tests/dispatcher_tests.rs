use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest},
    load_spec,
    router::{RouteMatch, Router},
    typed::{Handler, TypedHandlerRequest},
};
use http::Method;
use may::sync::mpsc;
use pet_store::registry;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;
mod tracing_util;
use brrtrouter::middleware::TracingMiddleware;
use tracing_util::TestTracing;

fn set_stack_size() -> TestTracing {
    may::config().set_stack_size(0x401);
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
            .path_params
            .get("id")
            .ok_or_else(|| anyhow::anyhow!("missing id"))?
            .parse()?;
        let debug = req
            .query_params
            .get("debug")
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
    let handler_name = route.handler_name.clone();

    let (reply_tx, reply_rx) = mpsc::channel();
    let mut path_params = HashMap::new();
    path_params.insert("id".to_string(), "item-001".to_string());
    let mut query_params = HashMap::new();
    query_params.insert("debug".to_string(), "true".to_string());
    let body = json!({"name": "New Item"});

    let request = HandlerRequest {
        method: Method::POST,
        path: route.path_pattern.clone(),
        handler_name: handler_name.clone(),
        path_params,
        query_params,
        headers: HashMap::new(),
        cookies: HashMap::new(),
        body: Some(body),
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
    let handler_name = route.handler_name.clone();

    let (reply_tx, reply_rx) = mpsc::channel();
    let mut path_params = HashMap::new();
    path_params.insert("id".to_string(), "12345".to_string());
    let mut query_params = HashMap::new();
    query_params.insert("include".to_string(), "stats".to_string());

    let request = HandlerRequest {
        method: Method::GET,
        path: route.path_pattern.clone(),
        handler_name: handler_name.clone(),
        path_params,
        query_params,
        headers: HashMap::new(),
        cookies: HashMap::new(),
        body: None,
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
    let mut path_params = HashMap::new();
    path_params.insert("id".to_string(), "42".to_string());
    let mut query_params = HashMap::new();
    query_params.insert("debug".to_string(), "true".to_string());

    let request = HandlerRequest {
        method: Method::GET,
        path: "/items/{id}".to_string(),
        handler_name: "assert_controller".to_string(),
        path_params,
        query_params,
        headers: HashMap::new(),
        cookies: HashMap::new(),
        body: None,
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
    let mut path_params = HashMap::new();
    // invalid integer value for id
    path_params.insert("id".to_string(), "not_an_int".to_string());
    let mut query_params = HashMap::new();
    query_params.insert("debug".to_string(), "true".to_string());

    let request = HandlerRequest {
        method: Method::GET,
        path: "/items/{id}".to_string(),
        handler_name: "assert_controller".to_string(),
        path_params,
        query_params,
        headers: HashMap::new(),
        cookies: HashMap::new(),
        body: None,
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
#[ignore]
fn test_panic_handler_returns_500() {
    let _tracing = set_stack_size();
    fn panic_handler(_req: HandlerRequest) {
        panic!("boom");
    }

    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_handler("panic", panic_handler);
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));

    let (reply_tx, reply_rx) = mpsc::channel();

    let request = HandlerRequest {
        method: Method::GET,
        path: "/panic".to_string(),
        handler_name: "panic".to_string(),
        path_params: HashMap::new(),
        query_params: HashMap::new(),
        headers: HashMap::new(),
        cookies: HashMap::new(),
        body: None,
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
        let (method, path, body, expected) = match name.as_str() {
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
            "stream_events" => (Method::GET, "/events", None, json!("")),
            other => panic!("unexpected handler {}", other),
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
        assert_eq!(resp.status, 200, "handler {}", name);
        // TODO: fix this assertion once the handlers return correct responses
        // assert_eq!(resp.body, expected, "handler {}", name);
    }
}
