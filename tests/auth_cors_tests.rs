use brrtrouter::dispatcher::{HandlerRequest, HandlerResponse};
use brrtrouter::middleware::{AuthMiddleware, CorsMiddleware, Middleware};
use http::Method;
use may::sync::mpsc;
use std::collections::HashMap;
use std::time::Duration;
use brrtrouter::ids::RequestId;

#[test]
fn test_auth_middleware_allows_valid_token() {
    let mw = AuthMiddleware::new("secret".into());
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), "secret".to_string());
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/".into(),
        handler_name: "test".into(),
        path_params: HashMap::new(),
        query_params: HashMap::new(),
        headers,
        cookies: HashMap::new(),
        body: None,
        reply_tx: tx,
    };
    assert!(mw.before(&req).is_none());
}

#[test]
fn test_auth_middleware_blocks_invalid_token() {
    let mw = AuthMiddleware::new("secret".into());
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let headers = HashMap::new();
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/".into(),
        handler_name: "test".into(),
        path_params: HashMap::new(),
        query_params: HashMap::new(),
        headers,
        cookies: HashMap::new(),
        body: None,
        reply_tx: tx,
    };
    let resp = mw.before(&req).expect("should produce response");
    assert_eq!(resp.status, 401);
    assert_eq!(resp.body["error"], "Unauthorized");
}

#[test]
fn test_cors_middleware_sets_headers() {
    let mw = CorsMiddleware::default();
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/".into(),
        handler_name: "test".into(),
        path_params: HashMap::new(),
        query_params: HashMap::new(),
        headers: HashMap::new(),
        cookies: HashMap::new(),
        body: None,
        reply_tx: tx,
    };
    let mut resp = HandlerResponse {
        status: 200,
        headers: HashMap::new(),
        body: serde_json::Value::Null,
    };
    mw.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(
        resp.headers.get("Access-Control-Allow-Origin"),
        Some(&"*".to_string())
    );
    assert_eq!(
        resp.headers.get("Access-Control-Allow-Headers"),
        Some(&"Content-Type, Authorization".to_string())
    );
    assert_eq!(
        resp.headers.get("Access-Control-Allow-Methods"),
        Some(&"GET, POST, PUT, DELETE, OPTIONS".to_string())
    );
}
