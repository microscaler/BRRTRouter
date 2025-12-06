//! Integration tests for automatic metrics path pre-registration
//!
//! These tests verify that when MetricsMiddleware is set on AppService,
//! all paths from the OpenAPI spec are automatically pre-registered.

use brrtrouter::middleware::MetricsMiddleware;
use brrtrouter::router::Router;
use brrtrouter::server::AppService;
use brrtrouter::spec::RouteMeta;
use http::Method;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Helper function to create a basic RouteMeta for testing
fn create_route_meta(method: Method, path: &str, handler: &str) -> RouteMeta {
    RouteMeta {
        method,
        path_pattern: Arc::from(path),
        handler_name: Arc::from(handler),
        base_path: String::new(),
        parameters: Vec::new(),
        request_schema: None,
        request_body_required: false,
        response_schema: None,
        example: None,
        responses: HashMap::new(),
        security: Vec::new(),
        example_name: "test_example".to_string(),
        project_slug: "test_project".to_string(),
        output_dir: PathBuf::from("test_output"),
        sse: false,
        estimated_request_body_bytes: None,
        x_brrtrouter_stack_size: None,
    }
}

#[test]
fn test_metrics_middleware_auto_preregisters_paths() {
    // Create a router with several routes
    let routes = vec![
        create_route_meta(Method::GET, "/users", "list_users"),
        create_route_meta(Method::GET, "/users/{id}", "get_user"),
        create_route_meta(Method::POST, "/users", "create_user"),
        create_route_meta(Method::GET, "/posts/{id}", "get_post"),
    ];

    let router = Router::new(routes);
    let router = Arc::new(RwLock::new(router));

    // Create an empty dispatcher (we won't actually use it)
    let dispatcher = Arc::new(RwLock::new(brrtrouter::dispatcher::Dispatcher::new()));

    // Create the service
    let mut service = AppService::new(
        router,
        dispatcher,
        HashMap::new(),
        PathBuf::from("test_spec.yaml"),
        None,
        None,
    );

    // Create metrics middleware
    let metrics = Arc::new(MetricsMiddleware::new());

    // Set the metrics middleware - this should automatically pre-register all paths
    service.set_metrics_middleware(metrics.clone());

    // Verify that all paths were pre-registered by checking path_stats
    let stats = metrics.path_stats();

    // Should have 3 unique registered paths (GET /users and POST /users share the same path)
    // Metrics are collected per-path, not per-method
    assert_eq!(stats.len(), 3, "All unique paths should be pre-registered");

    // Verify specific paths exist
    assert!(stats.contains_key("/users"), "Should contain /users");
    assert!(
        stats.contains_key("/users/{id}"),
        "Should contain /users/{{id}}"
    );
    assert!(
        stats.contains_key("/posts/{id}"),
        "Should contain /posts/{{id}}"
    );

    // All should have zero counts initially (only pre-registered, not used)
    for (path, (count, _, _, _)) in stats.iter() {
        assert_eq!(*count, 0, "Path {} should have zero count initially", path);
    }
}

#[test]
fn test_metrics_middleware_preregistration_with_base_path() {
    // Create routes with a base path
    let mut route1 = create_route_meta(Method::GET, "/users", "list_users");
    route1.base_path = "/api/v1".to_string();

    let mut route2 = create_route_meta(Method::GET, "/posts", "list_posts");
    route2.base_path = "/api/v1".to_string();

    let routes = vec![route1, route2];

    let router = Router::new(routes);
    let router = Arc::new(RwLock::new(router));

    let dispatcher = Arc::new(RwLock::new(brrtrouter::dispatcher::Dispatcher::new()));

    let mut service = AppService::new(
        router,
        dispatcher,
        HashMap::new(),
        PathBuf::from("test_spec.yaml"),
        None,
        None,
    );

    let metrics = Arc::new(MetricsMiddleware::new());
    service.set_metrics_middleware(metrics.clone());

    let stats = metrics.path_stats();

    // Should have full paths including base path
    assert_eq!(stats.len(), 2);
    assert!(
        stats.contains_key("/api/v1/users"),
        "Should contain full path with base"
    );
    assert!(
        stats.contains_key("/api/v1/posts"),
        "Should contain full path with base"
    );
}

#[test]
fn test_metrics_middleware_preregisters_parameterized_paths() {
    // Test that parameterized paths are correctly pre-registered
    let routes = vec![
        create_route_meta(Method::GET, "/users/{id}", "get_user"),
        create_route_meta(Method::GET, "/users/{id}/posts/{post_id}", "get_user_post"),
    ];

    let router = Router::new(routes);
    let router = Arc::new(RwLock::new(router));

    let dispatcher = Arc::new(RwLock::new(brrtrouter::dispatcher::Dispatcher::new()));

    let mut service = AppService::new(
        router,
        dispatcher,
        HashMap::new(),
        PathBuf::from("test_spec.yaml"),
        None,
        None,
    );

    let metrics = Arc::new(MetricsMiddleware::new());
    service.set_metrics_middleware(metrics.clone());

    // Verify parameterized paths are pre-registered with their parameter placeholders
    let stats = metrics.path_stats();
    assert_eq!(stats.len(), 2);
    assert!(
        stats.contains_key("/users/{id}"),
        "Should contain parameterized path"
    );
    assert!(
        stats.contains_key("/users/{id}/posts/{post_id}"),
        "Should contain nested parameterized path"
    );

    // Verify they have zero counts initially
    assert_eq!(stats.get("/users/{id}").unwrap().0, 0);
    assert_eq!(stats.get("/users/{id}/posts/{post_id}").unwrap().0, 0);
}

#[test]
fn test_metrics_middleware_empty_router() {
    // Test with empty router (no routes)
    let router = Router::new(vec![]);
    let router = Arc::new(RwLock::new(router));

    let dispatcher = Arc::new(RwLock::new(brrtrouter::dispatcher::Dispatcher::new()));

    let mut service = AppService::new(
        router,
        dispatcher,
        HashMap::new(),
        PathBuf::from("test_spec.yaml"),
        None,
        None,
    );

    let metrics = Arc::new(MetricsMiddleware::new());
    service.set_metrics_middleware(metrics.clone());

    // Should have no pre-registered paths
    let stats = metrics.path_stats();
    assert_eq!(stats.len(), 0);
}
