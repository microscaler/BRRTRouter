use brrtrouter::{
    dispatcher::{Dispatcher, HandlerRequest, HandlerResponse},
    load_spec,
    middleware::{Middleware, TracingMiddleware},
    router::Router,
};
use http::Method;
use pet_store::registry;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
mod tracing_util;
use tracing_util::TestTracing;

// Helper function to create a valid HandlerRequest for testing
fn create_test_request(
    method: Method,
    path: &str,
    handler_name: &str,
    body: Option<Value>,
) -> HandlerRequest {
    let (reply_tx, _reply_rx) = may::sync::mpsc::channel();
    HandlerRequest {
        method,
        path: path.to_string(),
        handler_name: handler_name.to_string(),
        path_params: HashMap::new(),
        query_params: HashMap::new(),
        headers: HashMap::new(),
        cookies: HashMap::new(),
        body,
        reply_tx,
    }
}

// Helper function to create a valid HandlerResponse for testing
fn create_test_response(status: u16, body: Value) -> HandlerResponse {
    HandlerResponse {
        status,
        headers: HashMap::new(),
        body,
    }
}

#[test]
fn test_tracing_middleware_emits_spans() {
    let mut tracing = TestTracing::init();

    let (routes, _slug) = load_spec("examples/openapi.yaml").unwrap();
    let router = Router::new(routes.clone());
    let mut dispatcher = Dispatcher::new();
    unsafe {
        registry::register_from_spec(&mut dispatcher, &routes);
    }
    dispatcher.add_middleware(Arc::new(TracingMiddleware));

    // Use a simpler endpoint that doesn't require special validation parameters
    let route_match = router.route(Method::GET, "/users/abc-123").unwrap();
    let resp = dispatcher
        .dispatch(route_match, None, Default::default(), Default::default())
        .unwrap();
    assert_eq!(resp.status, 200);

    // Force flush the tracer provider to ensure spans are exported
    tracing.force_flush();

    // Give a moment for spans to be exported
    std::thread::sleep(std::time::Duration::from_millis(100));

    let spans = tracing.spans();
    assert!(!spans.is_empty());
}

#[test]
fn test_tracing_middleware_before_method() {
    let middleware = TracingMiddleware;
    let request = create_test_request(Method::GET, "/test/path", "test_handler", None);

    // before() should not return early response - always returns None
    let result = middleware.before(&request);
    assert!(result.is_none());
}

#[test]
fn test_tracing_middleware_after_method() {
    let middleware = TracingMiddleware;
    let request = create_test_request(
        Method::POST,
        "/api/endpoint",
        "api_handler",
        Some(json!({"test": "data"})),
    );

    let mut response = create_test_response(201, json!({"id": 123, "status": "created"}));
    let latency = Duration::from_millis(150);

    // after() should not modify the response, just log it
    let original_status = response.status;
    let original_body = response.body.clone();

    middleware.after(&request, &mut response, latency);

    assert_eq!(response.status, original_status);
    assert_eq!(response.body, original_body);
}

#[test]
fn test_tracing_middleware_with_different_http_methods() {
    let middleware = TracingMiddleware;
    let methods = vec![
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::PATCH,
    ];

    for method in methods {
        let body = if method == Method::GET {
            None
        } else {
            Some(json!({}))
        };
        let request = create_test_request(
            method.clone(),
            &format!("/api/{}", method.as_str().to_lowercase()),
            &format!("{}_handler", method.as_str().to_lowercase()),
            body,
        );

        let mut response = create_test_response(200, json!({"method": method.as_str()}));

        // Test before()
        assert!(middleware.before(&request).is_none());

        // Test after()
        middleware.after(&request, &mut response, Duration::from_millis(50));

        // Response should be unchanged
        assert_eq!(response.status, 200);
    }
}

#[test]
fn test_tracing_middleware_with_error_responses() {
    let middleware = TracingMiddleware;
    let error_statuses = vec![400, 401, 403, 404, 500, 502, 503];

    for status in error_statuses {
        let request = create_test_request(Method::GET, "/error/endpoint", "error_handler", None);
        let mut response =
            create_test_response(status, json!({"error": format!("Error {}", status)}));

        middleware.after(&request, &mut response, Duration::from_millis(25));

        // Status should remain unchanged
        assert_eq!(response.status, status);
    }
}

#[test]
fn test_tracing_middleware_with_various_latencies() {
    let middleware = TracingMiddleware;
    let latencies = vec![
        Duration::from_nanos(500),  // Very fast
        Duration::from_micros(100), // Fast
        Duration::from_millis(1),   // Quick
        Duration::from_millis(50),  // Normal
        Duration::from_millis(500), // Slow
        Duration::from_secs(1),     // Very slow
        Duration::from_secs(5),     // Extremely slow
    ];

    for latency in latencies {
        let request = create_test_request(Method::GET, "/latency/test", "latency_handler", None);
        let mut response = create_test_response(200, json!({"latency_ms": latency.as_millis()}));

        middleware.after(&request, &mut response, latency);

        // Response should be unchanged regardless of latency
        assert_eq!(response.status, 200);
    }
}

#[test]
fn test_tracing_middleware_with_complex_paths() {
    let middleware = TracingMiddleware;
    let complex_paths = vec![
        "/",
        "/simple",
        "/api/v1/users/123",
        "/api/v2/users/{id}/posts/{post_id}",
        "/very/deep/nested/path/with/many/segments",
        "/path/with/query?param=value",
        "/path/with/special-chars_and.dots",
        "/unicode/path/测试路径",
    ];

    for path in complex_paths {
        let request = create_test_request(Method::GET, path, "complex_path_handler", None);
        let mut response = create_test_response(200, json!({"path": path}));

        // Test both before and after
        assert!(middleware.before(&request).is_none());
        middleware.after(&request, &mut response, Duration::from_millis(10));

        assert_eq!(response.status, 200);
    }
}

#[test]
fn test_tracing_middleware_with_request_parameters() {
    let middleware = TracingMiddleware;

    // Create request with custom parameters
    let (reply_tx, _reply_rx) = may::sync::mpsc::channel();
    let mut path_params = HashMap::new();
    path_params.insert("user_id".to_string(), "12345".to_string());
    path_params.insert("post_id".to_string(), "67890".to_string());

    let mut query_params = HashMap::new();
    query_params.insert("include".to_string(), "owner".to_string());
    query_params.insert("format".to_string(), "json".to_string());

    let mut headers = HashMap::new();
    headers.insert("content-type".to_string(), "application/json".to_string());
    headers.insert("x-request-id".to_string(), "test-123".to_string());

    let request = HandlerRequest {
        method: Method::GET,
        path: "/users/{user_id}/posts/{post_id}".to_string(),
        handler_name: "get_user_post".to_string(),
        path_params,
        query_params,
        headers,
        cookies: HashMap::new(),
        body: None,
        reply_tx,
    };

    let mut response = create_test_response(200, json!({"user_id": "12345", "post_id": "67890"}));

    assert!(middleware.before(&request).is_none());
    middleware.after(&request, &mut response, Duration::from_millis(75));

    assert_eq!(response.status, 200);
}

#[test]
fn test_tracing_middleware_zero_latency() {
    let middleware = TracingMiddleware;
    let request = create_test_request(Method::GET, "/instant", "instant_handler", None);
    let mut response = create_test_response(200, json!({"instant": true}));

    // Test with exactly zero latency
    middleware.after(&request, &mut response, Duration::from_nanos(0));

    assert_eq!(response.status, 200);
}
