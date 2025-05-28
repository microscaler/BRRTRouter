use brrtrouter::{load_spec, router::{Router, RouteMatch}, dispatcher::{Dispatcher, HandlerRequest}, typed::{Handler, TypedHandlerRequest}, spec::{ParameterMeta, ParameterLocation}};
use http::Method;
use may::sync::mpsc;
use serde_json::json;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use pet_store::registry;

#[derive(Debug, Deserialize, Serialize)]
struct TypedReq {
    id: i32,
    debug: bool,
}

#[derive(Debug, Serialize)]
struct TypedRes {
    ok: bool,
}

struct AssertController;

impl Handler<TypedReq, TypedRes> for AssertController {
    fn handle(&self, req: TypedHandlerRequest<TypedReq>) -> TypedRes {
        assert_eq!(req.data.id, 42);
        assert!(req.data.debug);
        TypedRes { ok: true }
    }
}

#[test]
fn test_dispatch_post_item() {
    let (routes, _slug) = load_spec("examples/openapi.yaml").expect("load spec");
    let router = Router::new(routes);
    let mut dispatcher = Dispatcher::new();
    unsafe { registry::register_all(&mut dispatcher); }

    let RouteMatch { route, .. } = router.route(Method::POST, "/items/item-001").expect("route");
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
        body: Some(body),
        reply_tx,
    };

    dispatcher.handlers.get(&handler_name).unwrap().send(request).unwrap();
    let resp = reply_rx.recv().unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, json!({"id": "item-001", "name": "New Item"}));
}

#[test]
fn test_dispatch_get_pet() {
    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();
    let router = Router::new(routes);
    let mut dispatcher = Dispatcher::new();
    unsafe { registry::register_all(&mut dispatcher); }

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
        body: None,
        reply_tx,
    };

    dispatcher.handlers.get(&handler_name).unwrap().send(request).unwrap();
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
    let mut dispatcher = Dispatcher::new();
    unsafe {
        dispatcher.register_typed(
            "assert_controller",
            AssertController,
            vec![
                ParameterMeta {
                    name: "id".to_string(),
                    location: ParameterLocation::Path,
                    required: true,
                    schema: Some(json!({"type": "integer"})),
                },
                ParameterMeta {
                    name: "debug".to_string(),
                    location: ParameterLocation::Query,
                    required: false,
                    schema: Some(json!({"type": "boolean"})),
                },
            ],
        );
    }

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
fn test_dispatch_all_registry_handlers() {
    let (routes, _slug) = load_spec("examples/openapi.yaml").expect("load spec");
    let router = Router::new(routes);
    let mut dispatcher = Dispatcher::new();
    unsafe { registry::register_all(&mut dispatcher); }

    let handlers: Vec<String> = dispatcher.handlers.keys().cloned().collect();
    for name in handlers {
        let (method, path, body, expected) = match name.as_str() {
            "admin_settings" => (
                Method::GET,
                "/admin/settings",
                None,
                json!({"feature_flags": null}),
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
                json!({"users": [{"id": "", "name": ""}, {"id": "", "name": ""}]}),
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
                json!({"items": [{"body": "", "id": "", "title": ""}]}),
            ),
            "get_post" => (
                Method::GET,
                "/users/abc-123/posts/post1",
                None,
                json!({"body": "Welcome to the blog", "id": "post1", "title": "Intro"}),
            ),
            other => panic!("unexpected handler {}", other),
        };

        let route_match = router.route(method.clone(), path).expect("route match");
        assert_eq!(route_match.handler_name, name);
        let resp = dispatcher
            .dispatch(route_match, body.clone())
            .expect("dispatch");
        assert_eq!(resp.status, 200, "handler {}", name);
        assert_eq!(resp.body, expected, "handler {}", name);
    }
}
