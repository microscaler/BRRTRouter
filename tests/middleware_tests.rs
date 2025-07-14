use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HandlerResponse},
    load_spec,
    middleware::Middleware,
    middleware::{AuthMiddleware, CorsMiddleware, MetricsMiddleware},
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

// Helper function to create a test HandlerRequest
fn create_test_request(method: Method, path: &str, headers: HashMap<String, String>) -> HandlerRequest {
    let (tx, _rx) = mpsc::channel::<HandlerResponse>();
    HandlerRequest {
        method,
        path: path.to_string(),
        handler_name: "test_handler".to_string(),
        path_params: HashMap::new(),
        query_params: HashMap::new(),
        headers,
        cookies: HashMap::new(),
        body: None,
        reply_tx: tx,
    }
}

// Helper function to create a test HandlerResponse
fn create_test_response(status: u16) -> HandlerResponse {
    HandlerResponse {
        status,
        headers: HashMap::new(),
        body: serde_json::Value::Null,
    }
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
            .dispatch(route_match, None, HashMap::new(), HashMap::new())
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
    
    // Stack usage should have defaults
    let (size, used) = metrics.stack_usage();
    assert!(size >= 0);
    assert!(used >= 0);
}

#[test]
fn test_auth_middleware_valid_token() {
    let auth = AuthMiddleware::new("Bearer valid-token".to_string());
    
    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), "Bearer valid-token".to_string());
    
    let req = create_test_request(Method::GET, "/protected", headers);
    let result = auth.before(&req);
    
    // Should return None (allow request to proceed)
    assert!(result.is_none());
}

#[test]
fn test_auth_middleware_invalid_token() {
    let auth = AuthMiddleware::new("Bearer valid-token".to_string());
    
    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), "Bearer invalid-token".to_string());
    
    let req = create_test_request(Method::GET, "/protected", headers);
    let result = auth.before(&req);
    
    // Should return 401 Unauthorized
    assert!(result.is_some());
    let response = result.unwrap();
    assert_eq!(response.status, 401);
    assert_eq!(response.body, serde_json::json!({ "error": "Unauthorized" }));
}

#[test]
fn test_auth_middleware_missing_token() {
    let auth = AuthMiddleware::new("Bearer valid-token".to_string());
    
    let headers = HashMap::new(); // No authorization header
    let req = create_test_request(Method::GET, "/protected", headers);
    let result = auth.before(&req);
    
    // Should return 401 Unauthorized
    assert!(result.is_some());
    let response = result.unwrap();
    assert_eq!(response.status, 401);
    assert_eq!(response.body, serde_json::json!({ "error": "Unauthorized" }));
}

#[test]
fn test_auth_middleware_case_insensitive_header() {
    let auth = AuthMiddleware::new("Bearer valid-token".to_string());
    
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer valid-token".to_string()); // Capital A
    
    let req = create_test_request(Method::GET, "/protected", headers);
    let result = auth.before(&req);
    
    // Should still fail because the middleware expects lowercase
    assert!(result.is_some());
    let response = result.unwrap();
    assert_eq!(response.status, 401);
}

#[test]
fn test_cors_custom_headers() {
    let mw = CorsMiddleware::new(
        vec!["https://example.com".into()],
        vec!["X-Token".into()],
        vec![Method::GET, Method::POST],
    );

    let req = create_test_request(Method::GET, "/", HashMap::new());
    let mut resp = create_test_response(200);
    
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

    let req = create_test_request(Method::OPTIONS, "/", HashMap::new());
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

#[test]
fn test_cors_non_preflight_request() {
    let mw = CorsMiddleware::default();
    
    let req = create_test_request(Method::GET, "/api/data", HashMap::new());
    let result = mw.before(&req);
    
    // Should return None for non-OPTIONS requests
    assert!(result.is_none());
}

#[test]
fn test_cors_multiple_origins() {
    let mw = CorsMiddleware::new(
        vec!["https://example.com".into(), "https://api.example.com".into()],
        vec!["Content-Type".into()],
        vec![Method::GET],
    );

    let req = create_test_request(Method::GET, "/", HashMap::new());
    let mut resp = create_test_response(200);
    
    mw.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(
        resp.headers.get("Access-Control-Allow-Origin"),
        Some(&"https://example.com, https://api.example.com".to_string())
    );
}

#[test]
fn test_cors_multiple_headers() {
    let mw = CorsMiddleware::new(
        vec!["*".into()],
        vec!["Content-Type".into(), "Authorization".into(), "X-Custom".into()],
        vec![Method::GET],
    );

    let req = create_test_request(Method::GET, "/", HashMap::new());
    let mut resp = create_test_response(200);
    
    mw.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(
        resp.headers.get("Access-Control-Allow-Headers"),
        Some(&"Content-Type, Authorization, X-Custom".to_string())
    );
}

#[test]
fn test_cors_multiple_methods() {
    let mw = CorsMiddleware::new(
        vec!["*".into()],
        vec!["Content-Type".into()],
        vec![Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH],
    );

    let req = create_test_request(Method::GET, "/", HashMap::new());
    let mut resp = create_test_response(200);
    
    mw.after(&req, &mut resp, Duration::from_millis(0));
    assert_eq!(
        resp.headers.get("Access-Control-Allow-Methods"),
        Some(&"GET, POST, PUT, DELETE, PATCH".to_string())
    );
}

#[test]
fn test_cors_default_configuration() {
    let mw = CorsMiddleware::default();
    
    let req = create_test_request(Method::GET, "/", HashMap::new());
    let mut resp = create_test_response(200);
    
    mw.after(&req, &mut resp, Duration::from_millis(0));
    
    // Check default values
    assert_eq!(resp.headers.get("Access-Control-Allow-Origin"), Some(&"*".to_string()));
    assert_eq!(resp.headers.get("Access-Control-Allow-Headers"), Some(&"Content-Type, Authorization".to_string()));
    assert_eq!(resp.headers.get("Access-Control-Allow-Methods"), Some(&"GET, POST, PUT, DELETE, OPTIONS".to_string()));
}

#[test]
fn test_middleware_combination_auth_and_cors() {
    let auth = AuthMiddleware::new("Bearer valid-token".to_string());
    let cors = CorsMiddleware::default();
    
    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), "Bearer valid-token".to_string());
    
    let req = create_test_request(Method::GET, "/protected", headers);
    let mut resp = create_test_response(200);
    
    // Test auth middleware first
    let auth_result = auth.before(&req);
    assert!(auth_result.is_none()); // Should allow request
    
    // Test CORS middleware after
    let cors_result = cors.before(&req);
    assert!(cors_result.is_none()); // Should not interfere with non-OPTIONS
    
    // Apply CORS headers
    cors.after(&req, &mut resp, Duration::from_millis(10));
    assert!(resp.headers.contains_key("Access-Control-Allow-Origin"));
}

#[test]
fn test_middleware_combination_auth_failure_with_cors() {
    let auth = AuthMiddleware::new("Bearer valid-token".to_string());
    let cors = CorsMiddleware::default();
    
    let headers = HashMap::new(); // No auth header
    let req = create_test_request(Method::GET, "/protected", headers);
    
    // Test auth middleware first - should fail
    let auth_result = auth.before(&req);
    assert!(auth_result.is_some());
    
    let mut resp = auth_result.unwrap();
    assert_eq!(resp.status, 401);
    
    // Even on auth failure, CORS headers should be applied
    cors.after(&req, &mut resp, Duration::from_millis(10));
    assert!(resp.headers.contains_key("Access-Control-Allow-Origin"));
}

#[test]
fn test_middleware_latency_tracking() {
    let metrics = MetricsMiddleware::new();
    
    let req = create_test_request(Method::GET, "/test", HashMap::new());
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
    
    let req = create_test_request(Method::GET, "/test", HashMap::new());
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
    
    let req = create_test_request(Method::GET, "/test", HashMap::new());
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
    
    let req = create_test_request(Method::GET, "/", HashMap::new());
    let mut resp = create_test_response(200);
    
    cors.after(&req, &mut resp, Duration::from_millis(0));
    
    // Should set empty strings for the headers
    assert_eq!(resp.headers.get("Access-Control-Allow-Origin"), Some(&"".to_string()));
    assert_eq!(resp.headers.get("Access-Control-Allow-Headers"), Some(&"".to_string()));
    assert_eq!(resp.headers.get("Access-Control-Allow-Methods"), Some(&"".to_string()));
}

#[test]
fn test_middleware_response_modification() {
    let cors = CorsMiddleware::default();
    
    let req = create_test_request(Method::GET, "/", HashMap::new());
    let mut resp = create_test_response(404);
    
    // Add some existing headers
    resp.headers.insert("Content-Type".to_string(), "application/json".to_string());
    
    cors.after(&req, &mut resp, Duration::from_millis(0));
    
    // Should preserve existing headers and add CORS headers
    assert_eq!(resp.headers.get("Content-Type"), Some(&"application/json".to_string()));
    assert!(resp.headers.contains_key("Access-Control-Allow-Origin"));
    assert!(resp.headers.contains_key("Access-Control-Allow-Headers"));
    assert!(resp.headers.contains_key("Access-Control-Allow-Methods"));
}

#[test]
fn test_middleware_thread_safety() {
    use std::thread;
    use std::sync::Arc;
    
    let metrics = Arc::new(MetricsMiddleware::new());
    let mut handles = vec![];
    
    // Spawn multiple threads to test thread safety
    for _ in 0..10 {
        let metrics_clone = Arc::clone(&metrics);
        let handle = thread::spawn(move || {
            let req = create_test_request(Method::GET, "/test", HashMap::new());
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
