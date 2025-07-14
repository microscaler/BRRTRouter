use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HandlerResponse},
    load_spec,
    middleware::Middleware,
    middleware::{CorsMiddleware, MetricsMiddleware},
    router::Router,
};
use http::Method;
use may::sync::mpsc;
use pet_store::registry;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

mod tracing_util;
use brrtrouter::middleware::TracingMiddleware;
use tracing_util::TestTracing;

#[test]
fn test_metrics_middleware_counts() {
    let _tracing = TestTracing::init();
    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    let metrics = Arc::new(MetricsMiddleware::new());
    dispatcher.add_middleware(metrics.clone());
    dispatcher.add_middleware(Arc::new(TracingMiddleware));

    let route_match = router.route(Method::GET, "/pets/12345").unwrap();
    let resp = dispatcher
        .dispatch(route_match, None, HashMap::new(), HashMap::new())
        .unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(metrics.request_count(), 1);
    assert!(metrics.average_latency().as_nanos() > 0);
}

#[test]
fn test_metrics_stack_usage() {
    // set an odd stack size so may prints usage information
    may::config().set_stack_size(0x801);
    let mut tracing = TestTracing::init();

    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    let metrics = Arc::new(MetricsMiddleware::new());
    dispatcher.add_middleware(metrics.clone());
    dispatcher.add_middleware(Arc::new(TracingMiddleware));

    let route_match = router.route(Method::GET, "/pets/12345").unwrap();
    let resp = dispatcher
        .dispatch(route_match, None, HashMap::new(), HashMap::new())
        .unwrap();
    assert_eq!(resp.status, 200);
    let (size, used) = metrics.stack_usage();
    assert_eq!(size, 0x801);
    assert!(used >= 0);
    // tracing.wait_for_span("get_pet");
}

#[test]
fn test_cors_custom_headers() {
    let mw = CorsMiddleware::new(
        vec!["https://example.com".into()],
        vec!["X-Token".into()],
        vec![Method::GET, Method::POST],
    );

    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let req = HandlerRequest {
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
        Some(&"https://example.com".to_string())
    );
    assert_eq!(
        resp.headers.get("Access-Control-Allow-Headers"),
        Some(&"X-Token".to_string())
    );
    assert_eq!(
        resp.headers.get("Access-Control-Allow-Methods"),
        Some(&"GET, POST".to_string())
    );
}

#[test]
fn test_cors_preflight_response() {
    let mw = CorsMiddleware::default();

    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    let req = HandlerRequest {
        method: Method::OPTIONS,
        path: "/".into(),
        handler_name: "test".into(),
        path_params: HashMap::new(),
        query_params: HashMap::new(),
        headers: HashMap::new(),
        cookies: HashMap::new(),
        body: None,
        reply_tx: tx,
    };
    let mut resp = mw.before(&req).expect("should return response");
    assert_eq!(resp.status, 204);
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
