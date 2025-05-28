use brrtrouter::{load_spec, router::{Router, RouteMatch}, dispatcher::{Dispatcher, HandlerRequest}, typed::{Handler, TypedHandlerRequest}, spec::ParameterMeta};
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
                    location: "Path".to_string(),
                    required: true,
                    schema: Some(json!({"type": "integer"})),
                },
                ParameterMeta {
                    name: "debug".to_string(),
                    location: "Query".to_string(),
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
