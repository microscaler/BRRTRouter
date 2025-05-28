use brrtrouter::{dispatcher::{HandlerRequest, HandlerResponse}, typed::TypedHandlerRequest, spec::{ParameterMeta, ParameterLocation}};
use http::Method;
use may::sync::mpsc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use brrtrouter::typed::TypedHandlerFor;

#[derive(Debug, Deserialize, Serialize)]
struct Req {
    id: i32,
    active: bool,
}

#[test]
fn test_from_handler_non_string_params() {
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let mut path_params = HashMap::new();
    path_params.insert("id".to_string(), "42".to_string());
    let mut query_params = HashMap::new();
    query_params.insert("active".to_string(), "true".to_string());

    let req = HandlerRequest {
        method: Method::GET,
        path: "/items/42".to_string(),
        handler_name: "test".to_string(),
        path_params: path_params.clone(),
        query_params: query_params.clone(),
        body: None,
        reply_tx: tx,
    };

    let params = vec![
        ParameterMeta { name: "id".to_string(), location: ParameterLocation::Path, required: true, schema: Some(json!({"type": "integer"})) },
        ParameterMeta { name: "active".to_string(), location: ParameterLocation::Query, required: false, schema: Some(json!({"type": "boolean"})) },
    ];

    let typed = TypedHandlerRequest::<Req>::from_handler(req, &params).expect("conversion failed");
    assert_eq!(typed.data.id, 42);
    assert!(typed.data.active);
}
