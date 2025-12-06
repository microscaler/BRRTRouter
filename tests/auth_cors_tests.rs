use brrtrouter::dispatcher::{HandlerRequest, HandlerResponse, HeaderVec};
use brrtrouter::ids::RequestId;
use brrtrouter::middleware::{AuthMiddleware, CorsMiddleware, Middleware};
use brrtrouter::router::ParamVec;
use http::Method;
use may::sync::mpsc;
use smallvec::smallvec;
use std::time::Duration;

#[test]
fn test_auth_middleware_allows_valid_token() {
    let mw = AuthMiddleware::new("secret".into());
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let headers: HeaderVec = smallvec![("authorization".to_string(), "secret".to_string())];
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/".into(),
        handler_name: "test".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        reply_tx: tx,
    };
    assert!(mw.before(&req).is_none());
}

#[test]
fn test_auth_middleware_blocks_invalid_token() {
    let mw = AuthMiddleware::new("secret".into());
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let req = HandlerRequest {
        request_id: RequestId::new(),
        method: Method::GET,
        path: "/".into(),
        handler_name: "test".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers: HeaderVec::new(),
        cookies: HeaderVec::new(),
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
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers: HeaderVec::new(),
        cookies: HeaderVec::new(),
        body: None,
        reply_tx: tx,
    };
    let mut resp = HandlerResponse::new(200, HeaderVec::new(), serde_json::Value::Null);
    mw.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(resp.get_header("Access-Control-Allow-Origin"), Some("*"));
    assert_eq!(
        resp.get_header("Access-Control-Allow-Headers"),
        Some("Content-Type, Authorization")
    );
    assert_eq!(
        resp.get_header("Access-Control-Allow-Methods"),
        Some("GET, POST, PUT, DELETE, OPTIONS")
    );
}
