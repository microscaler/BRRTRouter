#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Unit-level `AuthMiddleware` + `CorsMiddleware` behavior. For OpenAPI global security + raw TCP
//! HTTP, see `tests/cors_http_conformance_tests.rs` (API key), `tests/cors_http_security_schemes_tests.rs`
//! (Bearer / cookie), and `docs/CORS_IMPLEMENTATION_AUDIT.md` §3.

use brrtrouter::dispatcher::{HandlerRequest, HandlerResponse, HeaderVec};
use brrtrouter::ids::RequestId;
use brrtrouter::middleware::{AuthMiddleware, CorsMiddleware, Middleware};
use brrtrouter::router::ParamVec;
use http::Method;
use may::sync::mpsc;
use smallvec::smallvec;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_auth_middleware_allows_valid_token() {
    let mw = AuthMiddleware::new("secret".into());
    let (__tx_raw, _rx) = mpsc::channel::<HandlerResponse>();
    let tx = brrtrouter::dispatcher::HandlerReplySender::channel(__tx_raw);
    // JSF P2: HeaderVec now uses Arc<str> for keys
    let headers: HeaderVec = smallvec![(Arc::from("authorization"), "secret".to_string())];
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
        jwt_claims: None,
        reply_tx: tx,
        queue_guard: None,
    };
    assert!(mw.before(&req).is_none());
}

#[test]
fn test_auth_middleware_blocks_invalid_token() {
    let mw = AuthMiddleware::new("secret".into());
    let (__tx_raw, _rx) = mpsc::channel::<HandlerResponse>();
    let tx = brrtrouter::dispatcher::HandlerReplySender::channel(__tx_raw);
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
        jwt_claims: None,
        reply_tx: tx,
        queue_guard: None,
    };
    let resp = mw.before(&req).expect("should produce response");
    assert_eq!(resp.status, 401);
    assert_eq!(resp.body["error"], "Unauthorized");
}

#[test]
fn test_cors_middleware_sets_headers() {
    let mw = CorsMiddleware::permissive();
    let (__tx_raw, _rx) = mpsc::channel::<HandlerResponse>();
    let tx = brrtrouter::dispatcher::HandlerReplySender::channel(__tx_raw);
    // Add Origin header for CORS validation
    let headers: HeaderVec = smallvec![(Arc::from("origin"), "https://example.com".to_string())];
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
        jwt_claims: None,
        reply_tx: tx,
        queue_guard: None,
    };
    let mut resp = HandlerResponse::new(200, HeaderVec::new(), serde_json::Value::Null);
    mw.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(resp.get_header("access-control-allow-origin"), Some("*"));
    assert_eq!(
        resp.get_header("access-control-allow-headers"),
        Some("Content-Type, Authorization")
    );
    assert_eq!(
        resp.get_header("access-control-allow-methods"),
        Some("GET, POST, PUT, DELETE, OPTIONS")
    );
    assert_eq!(resp.get_header("vary"), Some("Origin"));
}
