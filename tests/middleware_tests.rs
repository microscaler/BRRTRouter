#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HandlerResponse, HeaderVec},
    load_spec,
    middleware::Middleware,
    middleware::{AuthMiddleware, CorsMiddleware, MetricsMiddleware, RouteCorsConfig, RouteCorsPolicy},
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
    let mw = CorsMiddleware::new_legacy(
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
    let mw = CorsMiddleware::permissive();

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
    let mw = CorsMiddleware::permissive();

    let req = create_test_request(Method::GET, "/api/data", HeaderVec::new());
    let result = mw.before(&req);

    // Should return None for non-OPTIONS requests
    assert!(result.is_none());
}

#[test]
fn test_cors_options_without_preflight_header() {
    // BUG FIX TEST: OPTIONS request without Access-Control-Request-Method should not return 403
    // Per CORS spec: Missing Access-Control-Request-Method means it's a regular OPTIONS request,
    // not a preflight. Regular OPTIONS requests should proceed to handler or return 200/204.
    let mw = CorsMiddleware::permissive();

    // Test 1: OPTIONS with Origin but no Access-Control-Request-Method (regular OPTIONS, not preflight)
    let mut headers1 = HeaderVec::new();
    headers1.push((Arc::from("origin"), "https://example.com".to_string()));
    // Note: No access-control-request-method header
    let req1 = create_test_request(Method::OPTIONS, "/api/data", headers1);
    let result1 = mw.before(&req1);
    
    // Should return None (proceed to handler) - not 403
    // This is the bug fix: regular OPTIONS requests should not be rejected
    assert!(
        result1.is_none(),
        "BUG FIX: OPTIONS without preflight header should proceed to handler, not return 403"
    );
    
    // Test 2: OPTIONS with Origin and Access-Control-Request-Method (preflight)
    let mut headers2 = HeaderVec::new();
    headers2.push((Arc::from("origin"), "https://example.com".to_string()));
    headers2.push((Arc::from("access-control-request-method"), "GET".to_string()));
    let req2 = create_test_request(Method::OPTIONS, "/api/data", headers2);
    let result2 = mw.before(&req2);
    
    // Should return preflight response (200 with CORS headers)
    assert!(
        result2.is_some(),
        "OPTIONS with preflight header should return preflight response"
    );
    let resp2 = result2.unwrap();
    assert_eq!(resp2.status, 200);
    assert_eq!(resp2.get_header("access-control-allow-origin"), Some("*"));
    
    // Test 3: OPTIONS without Origin (not a CORS request)
    // When there's no Origin header, it's not a CORS request
    // The code returns 200 OK without CORS headers (correct behavior)
    let req3 = create_test_request(Method::OPTIONS, "/api/data", HeaderVec::new());
    let result3 = mw.before(&req3);
    
    // Should return 200 OK without CORS headers (not a CORS request)
    assert!(result3.is_some(), "OPTIONS without Origin should return 200 OK");
    let resp3 = result3.unwrap();
    assert_eq!(resp3.status, 200);
    assert_eq!(resp3.get_header("access-control-allow-origin"), None, "No CORS headers for non-CORS request");
}

#[test]
fn test_cors_multiple_origins() {
    let mw = CorsMiddleware::new_legacy(
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
    let mw = CorsMiddleware::new_legacy(
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
    let mw = CorsMiddleware::new_legacy(
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
    let mw = CorsMiddleware::permissive();

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
    let cors = CorsMiddleware::permissive();

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
    let cors = CorsMiddleware::permissive();

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
    let cors = CorsMiddleware::new_legacy(
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
    let cors = CorsMiddleware::permissive();

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

#[test]
fn test_cors_builder_basic() {
    use brrtrouter::middleware::CorsMiddlewareBuilder;
    use http::Method;

    let cors = CorsMiddlewareBuilder::new()
        .allowed_origins(&["https://example.com"])
        .allowed_methods(&[Method::GET, Method::POST])
        .allowed_headers(&["Content-Type"])
        .build()
        .expect("Valid CORS configuration");

    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);

    cors.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(resp.get_header("access-control-allow-origin"), Some("https://example.com"));
}

#[test]
fn test_cors_builder_with_credentials() {
    use brrtrouter::middleware::CorsMiddlewareBuilder;
    use http::Method;

    let cors = CorsMiddlewareBuilder::new()
        .allowed_origins(&["https://example.com"])
        .allowed_methods(&[Method::GET])
        .allow_credentials(true)
        .build()
        .expect("Valid CORS configuration");

    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);

    cors.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(resp.get_header("access-control-allow-credentials"), Some("true"));
}

#[test]
fn test_cors_builder_wildcard_with_credentials_error() {
    use brrtrouter::middleware::{CorsConfigError, CorsMiddlewareBuilder};
    use http::Method;

    let result = CorsMiddlewareBuilder::new()
        .allowed_origins(&["*"])
        .allowed_methods(&[Method::GET])
        .allow_credentials(true)
        .build();

    assert!(result.is_err());
    match result {
        Err(CorsConfigError::WildcardWithCredentials) => {}
        _ => panic!("Expected WildcardWithCredentials error"),
    }
}

#[test]
fn test_route_cors_config_empty_origins_with_credentials_panic() {
    // BUG FIX TEST: RouteCorsConfig::with_origins should panic when credentials enabled with empty origins
    // This matches the validation in CorsMiddlewareBuilder::build()
    use brrtrouter::middleware::RouteCorsConfig;
    
    let mut route_config = RouteCorsConfig::default();
    route_config.allow_credentials = true;
    
    // Should panic when called with empty origins and credentials enabled
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        route_config.with_origins(&[]);
    }));
    
    assert!(
        result.is_err(),
        "RouteCorsConfig::with_origins should panic when credentials enabled with empty origins"
    );
}

#[test]
fn test_route_cors_config_empty_origins_without_credentials_ok() {
    // Test that empty origins without credentials is allowed
    use brrtrouter::middleware::RouteCorsConfig;
    
    let mut route_config = RouteCorsConfig::default();
    route_config.allow_credentials = false;
    
    // Should not panic when credentials are disabled
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        route_config.with_origins(&[]);
    }));
    
    assert!(
        result.is_ok(),
        "RouteCorsConfig::with_origins should allow empty origins when credentials are disabled"
    );
}

#[test]
fn test_cors_builder_empty_origins_with_credentials_error() {
    use brrtrouter::middleware::{CorsConfigError, CorsMiddlewareBuilder};
    use http::Method;

    // Test case 1: No origins specified, credentials enabled
    let result = CorsMiddlewareBuilder::new()
        .allowed_methods(&[Method::GET])
        .allow_credentials(true)
        .build();

    assert!(result.is_err());
    match result {
        Err(CorsConfigError::EmptyOriginsWithCredentials) => {}
        _ => panic!("Expected EmptyOriginsWithCredentials error"),
    }

    // Test case 2: Explicitly empty origins list, credentials enabled
    let result = CorsMiddlewareBuilder::new()
        .allowed_origins(&[])
        .allowed_methods(&[Method::GET])
        .allow_credentials(true)
        .build();

    assert!(result.is_err());
    match result {
        Err(CorsConfigError::EmptyOriginsWithCredentials) => {}
        _ => panic!("Expected EmptyOriginsWithCredentials error"),
    }
}

#[test]
fn test_cors_builder_expose_headers() {
    use brrtrouter::middleware::CorsMiddlewareBuilder;
    use http::Method;

    let cors = CorsMiddlewareBuilder::new()
        .allowed_origins(&["https://example.com"])
        .allowed_methods(&[Method::GET])
        .expose_headers(&["X-Total-Count", "X-Page-Number"])
        .build()
        .expect("Valid CORS configuration");

    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);

    cors.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(
        resp.get_header("access-control-expose-headers"),
        Some("X-Total-Count, X-Page-Number")
    );
}

#[test]
fn test_cors_builder_max_age() {
    use brrtrouter::middleware::CorsMiddlewareBuilder;
    use http::Method;

    let cors = CorsMiddlewareBuilder::new()
        .allowed_origins(&["https://example.com"])
        .allowed_methods(&[Method::GET, Method::POST])
        .max_age(3600)
        .build()
        .expect("Valid CORS configuration");

    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    headers.push((Arc::from("access-control-request-method"), "GET".to_string()));
    let req = create_test_request(Method::OPTIONS, "/", headers);

    let resp = cors.before(&req).expect("Should return preflight response");
    assert_eq!(resp.get_header("access-control-max-age"), Some("3600"));
}

#[test]
fn test_cors_regex_pattern_matching() {
    use brrtrouter::middleware::CorsMiddleware;
    use http::Method;

    // Test regex pattern matching - allow all *.example.com subdomains
    let cors = CorsMiddleware::with_regex_patterns(
        vec![r"^https://.*\.example\.com$".to_string()],
        vec!["Content-Type".to_string()],
        vec![Method::GET],
        false,
        vec![],
        None,
    );

    // Test matching origin
    let mut headers_match = HeaderVec::new();
    headers_match.push((Arc::from("origin"), "https://api.example.com".to_string()));
    let req_match = create_test_request(Method::GET, "/", headers_match);
    let mut resp_match = create_test_response(200);
    cors.after(&req_match, &mut resp_match, Duration::from_millis(0));
    assert_eq!(
        resp_match.get_header("access-control-allow-origin"),
        Some("https://api.example.com")
    );

    // Test non-matching origin
    let mut headers_no_match = HeaderVec::new();
    headers_no_match.push((Arc::from("origin"), "https://evil.com".to_string()));
    let req_no_match = create_test_request(Method::GET, "/", headers_no_match);
    let mut resp_no_match = create_test_response(200);
    cors.after(&req_no_match, &mut resp_no_match, Duration::from_millis(0));
    assert_eq!(resp_no_match.get_header("access-control-allow-origin"), None);
}

#[test]
fn test_cors_builder_regex_patterns() {
    use brrtrouter::middleware::CorsMiddlewareBuilder;
    use http::Method;

    let cors = CorsMiddlewareBuilder::new()
        .allowed_origins_regex(&[r"^https://.*\.example\.com$", r"^https://api\.example\.org$"])
        .allowed_methods(&[Method::GET])
        .build()
        .expect("Valid CORS configuration");

    // Test matching origin
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://www.example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);
    cors.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(
        resp.get_header("access-control-allow-origin"),
        Some("https://www.example.com")
    );
}

#[test]
fn test_cors_custom_validator() {
    use brrtrouter::middleware::CorsMiddleware;
    use http::Method;

    // Test custom validator - allow origins ending with .example.com
    let cors = CorsMiddleware::with_custom_validator(
        |origin: &str| origin.ends_with(".example.com"),
        vec!["Content-Type".to_string()],
        vec![Method::GET],
        false,
        vec![],
        None,
    );

    // Test matching origin
    let mut headers_match = HeaderVec::new();
    headers_match.push((Arc::from("origin"), "https://api.example.com".to_string()));
    let req_match = create_test_request(Method::GET, "/", headers_match);
    let mut resp_match = create_test_response(200);
    cors.after(&req_match, &mut resp_match, Duration::from_millis(0));
    assert_eq!(
        resp_match.get_header("access-control-allow-origin"),
        Some("https://api.example.com")
    );

    // Test non-matching origin
    let mut headers_no_match = HeaderVec::new();
    headers_no_match.push((Arc::from("origin"), "https://evil.com".to_string()));
    let req_no_match = create_test_request(Method::GET, "/", headers_no_match);
    let mut resp_no_match = create_test_response(200);
    cors.after(&req_no_match, &mut resp_no_match, Duration::from_millis(0));
    assert_eq!(resp_no_match.get_header("access-control-allow-origin"), None);
}

#[test]
fn test_cors_builder_custom_validator() {
    use brrtrouter::middleware::CorsMiddlewareBuilder;
    use http::Method;

    let cors = CorsMiddlewareBuilder::new()
        .allowed_origins_custom(|origin| origin.contains("example") && origin.starts_with("https://"))
        .allowed_methods(&[Method::GET])
        .build()
        .expect("Valid CORS configuration");

    // Test matching origin
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);
    cors.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(
        resp.get_header("access-control-allow-origin"),
        Some("https://example.com")
    );
}

#[test]
fn test_cors_credentials_support() {
    let mw = CorsMiddleware::new(
        vec!["https://example.com".into()],
        vec!["Content-Type".into()],
        vec![Method::GET, Method::POST],
        true, // allow credentials
        vec![], // no exposed headers
        None, // no max age
    );

    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);

    mw.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(resp.get_header("access-control-allow-credentials"), Some("true"));
    assert_eq!(resp.get_header("access-control-allow-origin"), Some("https://example.com"));
}

#[test]
#[should_panic(expected = "Cannot use wildcard origin")]
fn test_cors_credentials_with_wildcard_panics() {
    // This should panic because wildcard + credentials is invalid
    let _mw = CorsMiddleware::new(
        vec!["*".into()],
        vec!["Content-Type".into()],
        vec![Method::GET],
        true, // allow credentials - INVALID with wildcard
        vec![],
        None,
    );
}

#[test]
fn test_cors_exposed_headers() {
    let mw = CorsMiddleware::new(
        vec!["https://example.com".into()],
        vec!["Content-Type".into()],
        vec![Method::GET],
        false,
        vec!["X-Total-Count".into(), "X-Page-Number".into()],
        None,
    );

    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);

    mw.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(
        resp.get_header("access-control-expose-headers"),
        Some("X-Total-Count, X-Page-Number")
    );
}

#[test]
fn test_cors_preflight_max_age() {
    let mw = CorsMiddleware::new(
        vec!["https://example.com".into()],
        vec!["Content-Type".into()],
        vec![Method::GET, Method::POST],
        false,
        vec![],
        Some(3600), // 1 hour cache
    );

    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    headers.push((Arc::from("access-control-request-method"), "GET".to_string()));
    let req = create_test_request(Method::OPTIONS, "/", headers);

    let resp = mw.before(&req).expect("should return preflight response");
    assert_eq!(resp.get_header("access-control-max-age"), Some("3600"));
    assert_eq!(resp.get_header("access-control-allow-origin"), Some("https://example.com"));
}

#[test]
fn test_cors_secure_default() {
    let mw = CorsMiddleware::default();

    // Default should have empty origins (secure)
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);

    mw.after(&req, &mut resp, Duration::from_millis(0));
    // With empty origins, no CORS headers should be added
    assert_eq!(resp.get_header("access-control-allow-origin"), None);
}

#[test]
fn test_cors_permissive_for_development() {
    let mw = CorsMiddleware::permissive();

    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    let req = create_test_request(Method::GET, "/", headers);
    let mut resp = create_test_response(200);

    mw.after(&req, &mut resp, Duration::from_millis(0));
    // Permissive should allow all origins
    assert_eq!(resp.get_header("access-control-allow-origin"), Some("*"));
}

#[test]
#[should_panic(expected = "Cannot use wildcard origin")]
fn test_cors_route_wildcard_with_credentials_panic() {
    // P1: Test that route config with allowCredentials: true cannot use wildcard origins
    let mut route_config = RouteCorsConfig::default();
    route_config.allow_credentials = true;
    // This should panic because wildcard + credentials is invalid per CORS spec
    let _ = route_config.with_origins(&["*"]);
}

#[test]
fn test_cors_same_origin_port_comparison() {
    // Test that is_same_origin correctly compares ports
    // This fixes the bug where different ports were treated as same-origin
    let mw = CorsMiddleware::permissive();

    // Test 1: Same hostname, different ports - should be cross-origin (CORS headers added)
    // BUG FIX: Host: localhost (no port, server on default port 80), Origin: http://localhost:8080
    // These are different origins per browser same-origin policy (ports differ: 80 vs 8080)
    // The bug was that is_same_origin() would normalize Host to port 80 and compare,
    // but it should detect that Origin has explicit non-default port and treat as different
    let mut headers1 = HeaderVec::new();
    headers1.push((Arc::from("host"), "localhost".to_string()));
    headers1.push((Arc::from("origin"), "http://localhost:8080".to_string()));
    let req1 = create_test_request(Method::GET, "/api/data", headers1);
    let mut resp1 = create_test_response(200);
    mw.after(&req1, &mut resp1, Duration::from_millis(0));
    // Should have CORS headers because ports differ (80 vs 8080)
    // This is the critical bug fix: explicit port 8080 != implicit default port 80
    assert_eq!(
        resp1.get_header("access-control-allow-origin"),
        Some("*"),
        "BUG FIX: Host with no port (default 80) vs Origin with explicit port 8080 should be cross-origin"
    );

    // Test 2: Same hostname, same port (explicit) - should be same-origin (no CORS headers)
    // Host: localhost:80, Origin: http://localhost:80
    let mut headers2 = HeaderVec::new();
    headers2.push((Arc::from("host"), "localhost:80".to_string()));
    headers2.push((Arc::from("origin"), "http://localhost:80".to_string()));
    let req2 = create_test_request(Method::GET, "/api/data", headers2);
    let mut resp2 = create_test_response(200);
    mw.after(&req2, &mut resp2, Duration::from_millis(0));
    // Should NOT have CORS headers because same origin (hostname + port match)
    assert_eq!(
        resp2.get_header("access-control-allow-origin"),
        None,
        "Same hostname and port should be treated as same-origin"
    );

    // Test 3: Same hostname, default port (implicit) - should be same-origin (no CORS headers)
    // Host: localhost (default 80), Origin: http://localhost (default 80)
    let mut headers3 = HeaderVec::new();
    headers3.push((Arc::from("host"), "localhost".to_string()));
    headers3.push((Arc::from("origin"), "http://localhost".to_string()));
    let req3 = create_test_request(Method::GET, "/api/data", headers3);
    let mut resp3 = create_test_response(200);
    mw.after(&req3, &mut resp3, Duration::from_millis(0));
    // Should NOT have CORS headers because same origin (both use default port 80)
    assert_eq!(
        resp3.get_header("access-control-allow-origin"),
        None,
        "Same hostname with implicit default ports should be treated as same-origin"
    );

    // Test 4: Same hostname, default port (implicit in Host, explicit in Origin) - should be same-origin
    // Host: localhost (default 80), Origin: http://localhost:80
    let mut headers4 = HeaderVec::new();
    headers4.push((Arc::from("host"), "localhost".to_string()));
    headers4.push((Arc::from("origin"), "http://localhost:80".to_string()));
    let req4 = create_test_request(Method::GET, "/api/data", headers4);
    let mut resp4 = create_test_response(200);
    mw.after(&req4, &mut resp4, Duration::from_millis(0));
    // Should NOT have CORS headers because both resolve to port 80
    assert_eq!(
        resp4.get_header("access-control-allow-origin"),
        None,
        "Same hostname with default port (implicit vs explicit) should be treated as same-origin"
    );

    // Test 5: HTTPS with default port 443
    // Host: example.com (default 443), Origin: https://example.com (default 443)
    let mut headers5 = HeaderVec::new();
    headers5.push((Arc::from("host"), "example.com".to_string()));
    headers5.push((Arc::from("origin"), "https://example.com".to_string()));
    let req5 = create_test_request(Method::GET, "/api/data", headers5);
    let mut resp5 = create_test_response(200);
    mw.after(&req5, &mut resp5, Duration::from_millis(0));
    // Should NOT have CORS headers because same origin (both use default port 443)
    assert_eq!(
        resp5.get_header("access-control-allow-origin"),
        None,
        "HTTPS with default port 443 should be treated as same-origin"
    );

    // Test 6: HTTPS with different ports - should be cross-origin
    // Host: example.com:443 (default), Origin: https://example.com:8443
    let mut headers6 = HeaderVec::new();
    headers6.push((Arc::from("host"), "example.com".to_string()));
    headers6.push((Arc::from("origin"), "https://example.com:8443".to_string()));
    let req6 = create_test_request(Method::GET, "/api/data", headers6);
    let mut resp6 = create_test_response(200);
    mw.after(&req6, &mut resp6, Duration::from_millis(0));
    // Should have CORS headers because ports differ (443 vs 8443)
    assert_eq!(
        resp6.get_header("access-control-allow-origin"),
        Some("*"),
        "Different HTTPS ports should be treated as cross-origin"
    );
}

#[test]
fn test_cors_x_cors_false_disables_cors() {
    // Test that x-cors: false actually disables CORS (no CORS headers)
    use brrtrouter::middleware::CorsMiddlewareBuilder;
    use std::collections::HashMap;

    // Create global CORS middleware with permissive settings
    let global_cors = CorsMiddlewareBuilder::new()
        .allowed_origins(&["*"])
        .allowed_headers(&["Content-Type", "Authorization"])
        .allowed_methods(&[Method::GET, Method::POST])
        .build()
        .unwrap();

    // Build route policies map
    let mut route_policies = HashMap::new();
    route_policies.insert(
        "internal_handler".to_string(),
        RouteCorsPolicy::Disabled,
    );

    // Create CORS middleware with route-specific disabled policy
    let cors = CorsMiddleware::with_route_policies(global_cors, route_policies);

    // Test OPTIONS request (preflight) - should return 200 without CORS headers
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://evil.com".to_string()));
    headers.push((
        Arc::from("access-control-request-method"),
        "GET".to_string(),
    ));
    let req = HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::OPTIONS,
        path: "/api/internal".to_string(),
        handler_name: "internal_handler".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: mpsc::channel::<HandlerResponse>().0,
    };

    let resp = cors.before(&req).expect("Should return response for OPTIONS");
    assert_eq!(resp.status, 200, "OPTIONS should return 200 even when CORS is disabled");
    // Critical: No CORS headers should be present
    assert_eq!(
        resp.get_header("access-control-allow-origin"),
        None,
        "x-cors: false should prevent CORS headers in preflight response"
    );
    assert_eq!(
        resp.get_header("access-control-allow-methods"),
        None,
        "x-cors: false should prevent CORS headers in preflight response"
    );

    // Test GET request - should not add CORS headers in after()
    let mut headers2 = HeaderVec::new();
    headers2.push((Arc::from("origin"), "https://evil.com".to_string()));
    let req2 = HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::GET,
        path: "/api/internal".to_string(),
        handler_name: "internal_handler".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers: headers2,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: mpsc::channel::<HandlerResponse>().0,
    };

    // before() should not short-circuit (CORS disabled, so no validation)
    assert!(
        cors.before(&req2).is_none(),
        "before() should not short-circuit when CORS is disabled"
    );

    // after() should not add CORS headers
    let mut resp2 = HandlerResponse::new(200, HeaderVec::new(), serde_json::Value::Null);
    cors.after(&req2, &mut resp2, Duration::from_millis(0));
    assert_eq!(
        resp2.get_header("access-control-allow-origin"),
        None,
        "x-cors: false should prevent CORS headers in response"
    );
    assert_eq!(
        resp2.get_header("access-control-allow-methods"),
        None,
        "x-cors: false should prevent CORS headers in response"
    );
}

#[test]
fn test_cors_x_cors_inherit_uses_global_config() {
    // Test that x-cors: "inherit" uses global CORS config
    use brrtrouter::middleware::CorsMiddlewareBuilder;
    use std::collections::HashMap;

    // Create global CORS middleware with specific origin
    let global_cors = CorsMiddlewareBuilder::new()
        .allowed_origins(&["https://example.com"])
        .allowed_headers(&["Content-Type", "Authorization"])
        .allowed_methods(&[Method::GET, Method::POST])
        .allow_credentials(true)
        .build()
        .unwrap();

    // Build route policies map (Inherit policies are not stored, so map is empty)
    let route_policies = HashMap::new();

    // Create CORS middleware (Inherit is default, so no route-specific policy needed)
    let cors = CorsMiddleware::with_route_policies(global_cors, route_policies);

    // Test OPTIONS request with valid origin - should return 200 with CORS headers
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));
    headers.push((
        Arc::from("access-control-request-method"),
        "GET".to_string(),
    ));
    let req = HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::OPTIONS,
        path: "/api/public".to_string(),
        handler_name: "public_handler".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: mpsc::channel::<HandlerResponse>().0,
    };

    let resp = cors.before(&req).expect("Should return response for OPTIONS");
    assert_eq!(resp.status, 200, "OPTIONS should return 200 for valid preflight");
    // Should have CORS headers from global config
    assert_eq!(
        resp.get_header("access-control-allow-origin"),
        Some("https://example.com"),
        "x-cors: 'inherit' should use global CORS config"
    );
    assert_eq!(
        resp.get_header("access-control-allow-credentials"),
        Some("true"),
        "x-cors: 'inherit' should use global credentials setting"
    );

    // Test GET request - should add CORS headers
    let mut headers2 = HeaderVec::new();
    headers2.push((Arc::from("origin"), "https://example.com".to_string()));
    let req2 = HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::GET,
        path: "/api/public".to_string(),
        handler_name: "public_handler".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers: headers2,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: mpsc::channel::<HandlerResponse>().0,
    };

    let mut resp2 = HandlerResponse::new(200, HeaderVec::new(), serde_json::Value::Null);
    cors.after(&req2, &mut resp2, Duration::from_millis(0));
    assert_eq!(
        resp2.get_header("access-control-allow-origin"),
        Some("https://example.com"),
        "x-cors: 'inherit' should use global CORS config in response"
    );
    assert_eq!(
        resp2.get_header("access-control-allow-credentials"),
        Some("true"),
        "x-cors: 'inherit' should use global credentials setting in response"
    );
}

#[test]
fn test_cors_x_cors_false_vs_inherit_distinction() {
    // Critical test: Verify that x-cors: false and x-cors: "inherit" behave differently
    use brrtrouter::middleware::CorsMiddlewareBuilder;
    use std::collections::HashMap;

    // Create two routes: one with Disabled, one with Inherit
    let mut disabled_policies = HashMap::new();
    disabled_policies.insert("disabled_handler".to_string(), RouteCorsPolicy::Disabled);

    let inherit_policies = HashMap::new();
    // Inherit is default, so empty map means inherit

    // Create separate global CORS instances for each test
    let global_cors_disabled = CorsMiddlewareBuilder::new()
        .allowed_origins(&["*"])
        .allowed_headers(&["Content-Type"])
        .allowed_methods(&[Method::GET])
        .build()
        .unwrap();
    
    let global_cors_inherit = CorsMiddlewareBuilder::new()
        .allowed_origins(&["*"])
        .allowed_headers(&["Content-Type"])
        .allowed_methods(&[Method::GET])
        .build()
        .unwrap();

    let cors_disabled = CorsMiddleware::with_route_policies(global_cors_disabled, disabled_policies);
    let cors_inherit = CorsMiddleware::with_route_policies(global_cors_inherit, inherit_policies);

    // Test with same origin header
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("origin"), "https://example.com".to_string()));

    // Test disabled route - should NOT have CORS headers
    let req_disabled = HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::GET,
        path: "/api/disabled".to_string(),
        handler_name: "disabled_handler".into(),
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers: headers.clone(),
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: mpsc::channel::<HandlerResponse>().0,
    };

    let mut resp_disabled = HandlerResponse::new(200, HeaderVec::new(), serde_json::Value::Null);
    cors_disabled.after(&req_disabled, &mut resp_disabled, Duration::from_millis(0));
    assert_eq!(
        resp_disabled.get_header("access-control-allow-origin"),
        None,
        "x-cors: false should NOT add CORS headers"
    );

    // Test inherit route - SHOULD have CORS headers
    let req_inherit = HandlerRequest {
        request_id: brrtrouter::ids::RequestId::new(),
        method: Method::GET,
        path: "/api/inherit".to_string(),
        handler_name: "inherit_handler".into(), // Different handler, so uses global config
        path_params: ParamVec::new(),
        query_params: ParamVec::new(),
        headers,
        cookies: HeaderVec::new(),
        body: None,
        jwt_claims: None,
        reply_tx: mpsc::channel::<HandlerResponse>().0,
    };

    let mut resp_inherit = HandlerResponse::new(200, HeaderVec::new(), serde_json::Value::Null);
    cors_inherit.after(&req_inherit, &mut resp_inherit, Duration::from_millis(0));
    assert_eq!(
        resp_inherit.get_header("access-control-allow-origin"),
        Some("*"),
        "x-cors: 'inherit' SHOULD add CORS headers from global config"
    );

    // This is the critical distinction: disabled has NO headers, inherit HAS headers
    assert_ne!(
        resp_disabled.get_header("access-control-allow-origin"),
        resp_inherit.get_header("access-control-allow-origin"),
        "x-cors: false and x-cors: 'inherit' must behave differently"
    );
}

#[test]
fn test_extract_route_cors_config_false_vs_inherit() {
    // Test the extraction function directly
    use brrtrouter::middleware::extract_route_cors_config;
    use oas3::spec::Operation;
    use serde_json::json;

    // Test x-cors: false
    let mut op_false = Operation::default();
    op_false.extensions.insert(
        "x-cors".to_string(),
        json!(false),
    );
    let policy_false = extract_route_cors_config(&op_false);
    assert!(
        matches!(policy_false, RouteCorsPolicy::Disabled),
        "x-cors: false should return RouteCorsPolicy::Disabled"
    );

    // Test x-cors: "inherit"
    let mut op_inherit = Operation::default();
    op_inherit.extensions.insert(
        "x-cors".to_string(),
        json!("inherit"),
    );
    let policy_inherit = extract_route_cors_config(&op_inherit);
    assert!(
        matches!(policy_inherit, RouteCorsPolicy::Inherit),
        "x-cors: 'inherit' should return RouteCorsPolicy::Inherit"
    );

    // Test missing x-cors (should default to Inherit)
    let op_missing = Operation::default();
    let policy_missing = extract_route_cors_config(&op_missing);
    assert!(
        matches!(policy_missing, RouteCorsPolicy::Inherit),
        "Missing x-cors should default to RouteCorsPolicy::Inherit"
    );

    // Verify they are different
    assert_ne!(
        format!("{:?}", policy_false),
        format!("{:?}", policy_inherit),
        "x-cors: false and x-cors: 'inherit' must return different policies"
    );
}

#[test]
fn test_cors_ipv6_address_parsing() {
    // BUG FIX TEST: Verify IPv6 addresses are parsed correctly
    // IPv6 addresses like [::1]:8080 should be handled correctly
    // The bug was that find(':') would find the first colon inside ::1, not the port delimiter
    let cors = CorsMiddleware::permissive();
    
    // Test 1: IPv6 same-origin request
    let req1 = create_test_request(
        Method::GET,
        "/test",
        smallvec![
            (Arc::from("origin"), "http://[::1]:8080".to_string()),
            (Arc::from("host"), "[::1]:8080".to_string()),
        ],
    );
    let mut resp1 = create_test_response(200);
    cors.after(&req1, &mut resp1, Duration::from_millis(0));
    // Same origin - no CORS headers should be added
    assert_eq!(resp1.get_header("access-control-allow-origin"), None);
    
    // Test 2: IPv6 cross-origin request (different ports)
    let req2 = create_test_request(
        Method::GET,
        "/test",
        smallvec![
            (Arc::from("origin"), "http://[::1]:8080".to_string()),
            (Arc::from("host"), "[::1]:9090".to_string()),
        ],
    );
    let mut resp2 = create_test_response(200);
    cors.after(&req2, &mut resp2, Duration::from_millis(0));
    // Different ports - CORS headers should be added
    assert_eq!(resp2.get_header("access-control-allow-origin"), Some("*"));
    
    // Test 3: IPv6 without port (default port)
    let req3 = create_test_request(
        Method::GET,
        "/test",
        smallvec![
            (Arc::from("origin"), "http://[::1]".to_string()),
            (Arc::from("host"), "[::1]".to_string()),
        ],
    );
    let mut resp3 = create_test_response(200);
    cors.after(&req3, &mut resp3, Duration::from_millis(0));
    // Same origin (both use default port 80) - no CORS headers
    assert_eq!(resp3.get_header("access-control-allow-origin"), None);
    
    // Test 4: Malformed IPv6 (no closing bracket) - should be treated as invalid
    let req4 = create_test_request(
        Method::GET,
        "/test",
        smallvec![
            (Arc::from("origin"), "http://[::1:8080".to_string()),
            (Arc::from("host"), "[::1]:8080".to_string()),
        ],
    );
    let mut resp4 = create_test_response(200);
    cors.after(&req4, &mut resp4, Duration::from_millis(0));
    // Malformed origin - should be treated as cross-origin (CORS headers added)
    assert_eq!(resp4.get_header("access-control-allow-origin"), Some("*"));
}

#[test]
fn test_cors_malformed_port_parsing() {
    // BUG FIX TEST: Verify malformed ports are treated correctly
    // The bug was that unwrap_or(0) would convert parse failures to Some(0)
    // Now using .ok() to treat parse failures as None (default port)
    // This means malformed ports use default port, which is correct behavior
    let cors = CorsMiddleware::permissive();
    
    // Test 1: Malformed port in Origin (e.g., "abc") becomes None (default port 80)
    // Valid port 0 in Host is Some(0) (explicit port 0)
    // These are different: default port 80 vs explicit port 0
    let req1 = create_test_request(
        Method::GET,
        "/test",
        smallvec![
            (Arc::from("origin"), "http://example.com:abc".to_string()),
            (Arc::from("host"), "example.com:0".to_string()),
        ],
    );
    let mut resp1 = create_test_response(200);
    cors.after(&req1, &mut resp1, Duration::from_millis(0));
    // Malformed port (default 80) vs valid port 0 - different ports, CORS headers added
    assert_eq!(resp1.get_header("access-control-allow-origin"), Some("*"));
    
    // Test 2: Two different malformed ports both become None (default port)
    // Both use default port 80, so they match (same origin)
    let req2 = create_test_request(
        Method::GET,
        "/test",
        smallvec![
            (Arc::from("origin"), "http://example.com:abc".to_string()),
            (Arc::from("host"), "example.com:xyz".to_string()),
        ],
    );
    let mut resp2 = create_test_response(200);
    cors.after(&req2, &mut resp2, Duration::from_millis(0));
    // Both malformed ports use default port 80 - same origin, no CORS headers
    assert_eq!(resp2.get_header("access-control-allow-origin"), None);
    
    // Test 3: Valid port 0 should match valid port 0
    let req3 = create_test_request(
        Method::GET,
        "/test",
        smallvec![
            (Arc::from("origin"), "http://example.com:0".to_string()),
            (Arc::from("host"), "example.com:0".to_string()),
        ],
    );
    let mut resp3 = create_test_response(200);
    cors.after(&req3, &mut resp3, Duration::from_millis(0));
    // Valid port 0 matches valid port 0 - no CORS headers
    assert_eq!(resp3.get_header("access-control-allow-origin"), None);
    
    // Test 4: Malformed port (default 80) vs no port (default 80) - should match
    let req4 = create_test_request(
        Method::GET,
        "/test",
        smallvec![
            (Arc::from("origin"), "http://example.com:abc".to_string()),
            (Arc::from("host"), "example.com".to_string()),
        ],
    );
    let mut resp4 = create_test_response(200);
    cors.after(&req4, &mut resp4, Duration::from_millis(0));
    // Both use default port 80 - same origin, no CORS headers
    assert_eq!(resp4.get_header("access-control-allow-origin"), None);
}

#[test]
fn test_cors_same_origin_port_bug_fix() {
    // BUG FIX TEST: Verify that is_same_origin correctly handles port differences
    // when Host header has no port (default) and Origin has explicit non-default port
    use brrtrouter::middleware::CorsMiddleware;

    let mw = CorsMiddleware::permissive();

    // Test case from bug report:
    // Host: localhost (server on port 80, but Host header doesn't specify port)
    // Origin: http://localhost:8080 (explicit port 8080)
    // These are DIFFERENT origins per browser same-origin policy
    // Old bug: Would incorrectly treat as same-origin, skipping CORS headers
    // Fixed: Correctly detects different ports, adds CORS headers
    let mut headers = HeaderVec::new();
    headers.push((Arc::from("host"), "localhost".to_string()));
    headers.push((Arc::from("origin"), "http://localhost:8080".to_string()));
    let req = create_test_request(Method::GET, "/api/data", headers);
    
    // Verify is_same_origin returns false (different origins)
    // We can't call is_same_origin directly (it's private), so we test via after()
    let mut resp = create_test_response(200);
    mw.after(&req, &mut resp, Duration::from_millis(0));
    
    // Should have CORS headers because ports differ (implicit 80 vs explicit 8080)
    assert_eq!(
        resp.get_header("access-control-allow-origin"),
        Some("*"),
        "BUG FIX: Host 'localhost' (port 80) vs Origin 'http://localhost:8080' should be cross-origin"
    );

    // Additional test: Host with explicit default port vs Origin with no port
    // Host: localhost:80, Origin: http://localhost
    // These should be same-origin (both port 80)
    let mut headers2 = HeaderVec::new();
    headers2.push((Arc::from("host"), "localhost:80".to_string()));
    headers2.push((Arc::from("origin"), "http://localhost".to_string()));
    let req2 = create_test_request(Method::GET, "/api/data", headers2);
    let mut resp2 = create_test_response(200);
    mw.after(&req2, &mut resp2, Duration::from_millis(0));
    // Should NOT have CORS headers (same origin: both port 80)
    assert_eq!(
        resp2.get_header("access-control-allow-origin"),
        None,
        "Host 'localhost:80' vs Origin 'http://localhost' should be same-origin (both port 80)"
    );

    // Test: Host with explicit non-default port vs Origin with no port
    // Host: localhost:8080, Origin: http://localhost
    // These should be different origins (8080 vs 80)
    let mut headers3 = HeaderVec::new();
    headers3.push((Arc::from("host"), "localhost:8080".to_string()));
    headers3.push((Arc::from("origin"), "http://localhost".to_string()));
    let req3 = create_test_request(Method::GET, "/api/data", headers3);
    let mut resp3 = create_test_response(200);
    mw.after(&req3, &mut resp3, Duration::from_millis(0));
    // Should have CORS headers (different origins: 8080 vs 80)
    assert_eq!(
        resp3.get_header("access-control-allow-origin"),
        Some("*"),
        "Host 'localhost:8080' vs Origin 'http://localhost' should be cross-origin (8080 vs 80)"
    );

    // Test: Both have explicit ports, different values
    // Host: localhost:3000, Origin: http://localhost:8080
    // These should be different origins
    let mut headers4 = HeaderVec::new();
    headers4.push((Arc::from("host"), "localhost:3000".to_string()));
    headers4.push((Arc::from("origin"), "http://localhost:8080".to_string()));
    let req4 = create_test_request(Method::GET, "/api/data", headers4);
    let mut resp4 = create_test_response(200);
    mw.after(&req4, &mut resp4, Duration::from_millis(0));
    // Should have CORS headers (different ports: 3000 vs 8080)
    assert_eq!(
        resp4.get_header("access-control-allow-origin"),
        Some("*"),
        "Host 'localhost:3000' vs Origin 'http://localhost:8080' should be cross-origin"
    );

    // Test: Both have explicit ports, same values
    // Host: localhost:8080, Origin: http://localhost:8080
    // These should be same-origin
    let mut headers5 = HeaderVec::new();
    headers5.push((Arc::from("host"), "localhost:8080".to_string()));
    headers5.push((Arc::from("origin"), "http://localhost:8080".to_string()));
    let req5 = create_test_request(Method::GET, "/api/data", headers5);
    let mut resp5 = create_test_response(200);
    mw.after(&req5, &mut resp5, Duration::from_millis(0));
    // Should NOT have CORS headers (same origin: both port 8080)
    assert_eq!(
        resp5.get_header("access-control-allow-origin"),
        None,
        "Host 'localhost:8080' vs Origin 'http://localhost:8080' should be same-origin"
    );
}
