use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HandlerResponse, HeaderVec},
    load_spec,
    middleware::Middleware,
    middleware::{AuthMiddleware, CorsMiddleware, MetricsMiddleware},
    router::{ParamVec, Router},
};
use http::Method;
use may::sync::mpsc;
use pet_store::registry;
use smallvec::smallvec;
use std::sync::Arc;
use std::time::Duration;

mod tracing_util;
use brrtrouter::middleware::TracingMiddleware;
use tracing_util::TestTracing;

// Helper function to create a test HandlerRequest
fn create_test_request(method: Method, path: &str, headers: HeaderVec) -> HandlerRequest {
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method,
        path: path.to_string(),
        handler_name: "test_handler".to_string(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: tx,
    }
}

// Helper function to create a test HandlerResponse
fn create_test_response(status: u16) -> HandlerResponse {
    HandlerResponse::new(status, HeaderVec::new(), serde_json::Value::Null)
}

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
        .dispatch(route_match, None, HeaderVec::new(), HeaderVec::new())
        .unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(metrics.request_count(), 1);
    assert!(metrics.average_latency().as_nanos() > 0);
}

#[test]
fn test_metrics_stack_usage() {
    // set an odd stack size so may prints usage information
    may::config().set_stack_size(0x801);
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
        .dispatch(route_match, None, HeaderVec::new(), HeaderVec::new())
        .unwrap();
    assert_eq!(resp.status, 200);
    let (size, _used) = metrics.stack_usage();
    assert_eq!(size, 0x801);
    // used is always >= 0 since it's usize, no need to assert
    // tracing.wait_for_span("get_pet");
}

#[test]
fn test_metrics_middleware_multiple_requests() {
    let _tracing = TestTracing::init();
    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    let metrics = Arc::new(MetricsMiddleware::new());
    dispatcher.add_middleware(metrics.clone());

    // Make multiple requests
    for i in 0..5 {
        let route_match = router.route(Method::GET, "/pets/12345").unwrap();
        let resp = dispatcher
            .dispatch(route_match, None, HeaderVec::new(), HeaderVec::new())
            .unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(metrics.request_count(), i + 1);
    }

    // Check that average latency is calculated correctly
    assert!(metrics.average_latency().as_nanos() > 0);
}

#[test]
fn test_metrics_middleware_zero_requests() {
    let metrics = MetricsMiddleware::new();

    // Initially no requests
    assert_eq!(metrics.request_count(), 0);
    assert_eq!(metrics.average_latency(), Duration::from_nanos(0));

    // Stack usage should have defaults (both are usize, always >= 0)
    let (_size, _used) = metrics.stack_usage();
    // No need to assert >= 0 for usize values
}

#[test]
fn test_auth_middleware_valid_token() {
    let auth = AuthMiddleware::new("Bearer valid-token".to_string());

    // JSF P2: HeaderVec now uses Arc<str> for keys
    let headers: HeaderVec =
        smallvec![(Arc::from("authorization"), "Bearer valid-token".to_string())];

    let req = create_test_request(Method::GET, "/protected", headers);
    let result = auth.before(&req);

    // Should return None (allow request to proceed)
    assert!(result.is_none());
}

#[test]
fn test_auth_middleware_invalid_token() {
    let auth = AuthMiddleware::new("Bearer valid-token".to_string());

    // JSF P2: HeaderVec now uses Arc<str> for keys
    let headers: HeaderVec = smallvec![(
        Arc::from("authorization"),
        "Bearer invalid-token".to_string()
    )];

    let req = create_test_request(Method::GET, "/protected", headers);
    let result = auth.before(&req);

    // Should return 401 Unauthorized
    assert!(result.is_some());
    let response = result.unwrap();
    assert_eq!(response.status, 401);
    assert_eq!(
        response.body,
        serde_json::json!({ "error": "Unauthorized" })
    );
}

#[test]
fn test_auth_middleware_missing_token() {
    let auth = AuthMiddleware::new("Bearer valid-token".to_string());

    let headers = HeaderVec::new(); // No authorization header
    let req = create_test_request(Method::GET, "/protected", headers);
    let result = auth.before(&req);

    // Should return 401 Unauthorized
    assert!(result.is_some());
    let response = result.unwrap();
    assert_eq!(response.status, 401);
    assert_eq!(
        response.body,
        serde_json::json!({ "error": "Unauthorized" })
    );
}

#[test]
fn test_auth_middleware_case_insensitive_header() {
    let auth = AuthMiddleware::new("Bearer valid-token".to_string());

    // HTTP headers are case-insensitive per RFC 7230
    // "Authorization" (capital A) should match when looking for "authorization" (lowercase)
    // JSF P2: HeaderVec now uses Arc<str> for keys
    let headers: HeaderVec =
        smallvec![(Arc::from("Authorization"), "Bearer valid-token".to_string())];

    let req = create_test_request(Method::GET, "/protected", headers);
    let result = auth.before(&req);

    // Should succeed (return None) because header lookup is case-insensitive per RFC 7230
    assert!(
        result.is_none(),
        "Header lookup should be case-insensitive per RFC 7230"
    );
}

#[test]
fn test_cors_custom_headers() {
    let mw = CorsMiddleware::new(
        vec!["https://example.com".into()],
        vec!["X-Token".into()],
        vec![Method::GET, Method::POST],
    );

    // Create request with Origin header (cross-origin)
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);

    mw.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(
        resp.get_header("access-control-allow-origin"),
        Some("https://example.com")
    );
    assert_eq!(
        resp.get_header("access-control-allow-headers"),
        Some("X-Token")
    );
    assert_eq!(
        resp.get_header("access-control-allow-methods"),
        Some("GET, POST")
    );
    // Vary header should be present
    assert_eq!(resp.get_header("vary"), Some("Origin"));
}

#[test]
fn test_cors_preflight_response() {
    let mw = CorsMiddleware::default();

    // Create preflight request with Origin and requested method/headers
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    headers.push((Arc::from("access-control-request-method"), "GET".to_string()));
    headers.push((Arc::from("access-control-request-headers"), "Content-Type".to_string()));
    let req = create_test_request(Method::OPTIONS, "/", headers);
    
    let resp = mw.before(&req).expect("should return response");
    assert_eq!(resp.status, 200); // Preflight returns 200 with CORS headers
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

#[test]
fn test_cors_non_preflight_request() {
    let mw = CorsMiddleware::default();

    let req = create_test_request(Method::GET, "/api/data", HeaderVec::new());
    let result = mw.before(&req);

    // Should return None for non-OPTIONS requests
    assert!(result.is_none());
}

#[test]
fn test_cors_multiple_origins() {
    let mw = CorsMiddleware::new(
        vec![
            "https://example.com".into(),
            "https://api.example.com".into(),
        ],
        vec!["Content-Type".into()],
        vec![Method::GET],
    );

    // Test with first origin
    let mut headers1 = HeaderVec::new();
    headers1.push((Arc::from("origin"), "https://example.com".to_string()));
    let req1 = create_test_request(Method::GET, "/", headers1);
    let mut resp1 = create_test_response(200);
    mw.after(&req1, &mut resp1, Duration::from_millis(0));
    // CORS spec: only one origin per response (the matching one)
    assert_eq!(
        resp1.get_header("access-control-allow-origin"),
        Some("https://example.com")
    );

    // Test with second origin
    let mut headers2 = HeaderVec::new();
    headers2.push((Arc::from("origin"), "https://api.example.com".to_string()));
    let req2 = create_test_request(Method::GET, "/", headers2);
    let mut resp2 = create_test_response(200);
    mw.after(&req2, &mut resp2, Duration::from_millis(0));
    assert_eq!(
        resp2.get_header("access-control-allow-origin"),
        Some("https://api.example.com")
    );
}

#[test]
fn test_cors_multiple_headers() {
    let mw = CorsMiddleware::new(
        vec!["*".into()],
        vec![
            "Content-Type".into(),
            "Authorization".into(),
            "X-Custom".into(),
        ],
        vec![Method::GET],
    );

    // Create request with Origin header (cross-origin)
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);

    mw.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(
        resp.get_header("access-control-allow-headers"),
        Some("Content-Type, Authorization, X-Custom")
    );
}

#[test]
fn test_cors_multiple_methods() {
    let mw = CorsMiddleware::new(
        vec!["*".into()],
        vec!["Content-Type".into()],
        vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
        ],
    );

    // Create request with Origin header (cross-origin)
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);

    mw.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(
        resp.get_header("access-control-allow-methods"),
        Some("GET, POST, PUT, DELETE, PATCH")
    );
}

#[test]
fn test_cors_default_configuration() {
    let mw = CorsMiddleware::default();

    // Create request with Origin header (cross-origin)
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);

    mw.after(&req, &mut resp, Duration::from_millis(0));

    // Check default values
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

#[test]
fn test_middleware_combination_auth_and_cors() {
    let auth = AuthMiddleware::new("Bearer valid-token".to_string());
    let cors = CorsMiddleware::default();

    // JSF P2: HeaderVec now uses Arc<str> for keys
    let mut headers: HeaderVec =
        smallvec![(Arc::from("authorization"), "Bearer valid-token".to_string())];
    // Add Origin header for CORS
    headers.push((Arc::from("origin"), "https://example.com".to_string()));

    let req = create_test_request(Method::GET, "/protected", headers);
    let mut resp = create_test_response(200);

    // Test auth middleware first
    let auth_result = auth.before(&req);
    assert!(auth_result.is_none()); // Should allow request

    // Test CORS middleware after
    let cors_result = cors.before(&req);
    assert!(cors_result.is_none()); // Should not interfere with non-OPTIONS (origin is valid)

    // Apply CORS headers
    cors.after(&req, &mut resp, Duration::from_millis(10));
    assert!(resp.get_header("access-control-allow-origin").is_some());
}

#[test]
fn test_middleware_combination_auth_failure_with_cors() {
    let auth = AuthMiddleware::new("Bearer valid-token".to_string());
    let cors = CorsMiddleware::default();

    // Add Origin header for CORS (no auth header - will fail auth)
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/protected", headers);

    // Test auth middleware first - should fail
    let auth_result = auth.before(&req);
    assert!(auth_result.is_some());

    let mut resp = auth_result.unwrap();
    assert_eq!(resp.status, 401);

    // Even on auth failure, CORS headers should be applied if origin is valid
    cors.after(&req, &mut resp, Duration::from_millis(10));
    assert!(resp.get_header("access-control-allow-origin").is_some());
}

#[test]
fn test_middleware_latency_tracking() {
    let metrics = MetricsMiddleware::new();

    let req = create_test_request(Method::GET, "/test", HeaderVec::new());
    let mut resp = create_test_response(200);

    // Simulate request processing
    metrics.before(&req);

    let test_latency = Duration::from_millis(100);
    metrics.after(&req, &mut resp, test_latency);

    assert_eq!(metrics.request_count(), 1);
    assert_eq!(metrics.average_latency(), test_latency);
}

#[test]
fn test_middleware_latency_averaging() {
    let metrics = MetricsMiddleware::new();

    let req = create_test_request(Method::GET, "/test", HeaderVec::new());
    let mut resp = create_test_response(200);

    // Process multiple requests with different latencies
    let latencies = vec![
        Duration::from_millis(100),
        Duration::from_millis(200),
        Duration::from_millis(300),
    ];

    for latency in &latencies {
        metrics.before(&req);
        metrics.after(&req, &mut resp, *latency);
    }

    assert_eq!(metrics.request_count(), 3);

    // Average should be 200ms
    let avg = metrics.average_latency();
    assert_eq!(avg, Duration::from_millis(200));
}

#[test]
fn test_middleware_after_method_called() {
    let auth = AuthMiddleware::new("Bearer token".to_string());

    let req = create_test_request(Method::GET, "/test", HeaderVec::new());
    let mut resp = create_test_response(200);

    // The after method should not modify the response for AuthMiddleware
    auth.after(&req, &mut resp, Duration::from_millis(10));

    // Response should remain unchanged
    assert_eq!(resp.status, 200);
    assert!(resp.headers.is_empty());
}

#[test]
fn test_middleware_edge_case_empty_headers() {
    let cors = CorsMiddleware::new(
        vec![], // Empty origins
        vec![], // Empty headers
        vec![], // Empty methods
    );

    // Add Origin header for CORS validation
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);

    cors.after(&req, &mut resp, Duration::from_millis(0));

    // With empty origins list, origin validation fails, so no CORS headers should be added
    // This is the correct behavior - empty origins means no origins are allowed
    assert_eq!(resp.get_header("access-control-allow-origin"), None);
    assert_eq!(resp.get_header("access-control-allow-headers"), None);
    assert_eq!(resp.get_header("access-control-allow-methods"), None);
}

#[test]
fn test_middleware_response_modification() {
    let cors = CorsMiddleware::default();

    // Add Origin header for CORS validation
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(404);

    // Add some existing headers
    resp.set_header("Content-Type", "application/json".to_string());

    cors.after(&req, &mut resp, Duration::from_millis(0));

    // Should preserve existing headers and add CORS headers
    assert_eq!(resp.get_header("Content-Type"), Some("application/json"));
    assert!(resp.get_header("access-control-allow-origin").is_some());
    assert!(resp.get_header("access-control-allow-headers").is_some());
    assert!(resp.get_header("access-control-allow-methods").is_some());
    assert_eq!(resp.get_header("vary"), Some("Origin"));
}

#[test]
fn test_middleware_thread_safety() {
    use std::sync::Arc;
    use std::thread;

    let metrics = Arc::new(MetricsMiddleware::new());
    let mut handles = vec![];

    // Spawn multiple threads to test thread safety
    for _ in 0..10 {
        let metrics_clone = Arc::clone(&metrics);
        let handle = thread::spawn(move || {
            let req = create_test_request(Method::GET, "/test", HeaderVec::new());
            let mut resp = create_test_response(200);

            metrics_clone.before(&req);
            metrics_clone.after(&req, &mut resp, Duration::from_millis(10));
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Should have processed 10 requests
    assert_eq!(metrics.request_count(), 10);
    assert_eq!(metrics.average_latency(), Duration::from_millis(10));
}
