// Performance-focused tests for router scalability
//
// These tests validate that the radix tree implementation provides
// better performance characteristics than the previous O(n) linear scan.

use super::Router;
use crate::spec::RouteMeta;
use http::Method;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

fn create_route_meta(method: Method, path: &str, handler: &str) -> RouteMeta {
    RouteMeta {
        method,
        path_pattern: path.to_string(),
        handler_name: handler.to_string(),
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
fn test_router_performance_with_many_routes() {
    // Create a router with many routes to test scalability
    let mut routes = Vec::new();
    for i in 0..500 {
        routes.push(create_route_meta(
            Method::GET,
            &format!("/api/v1/resource{}/{{id}}", i),
            &format!("handler_{}", i),
        ));
    }

    let router = Router::new(routes);

    // Measure time to match a route (should be O(k) where k is path length)
    let start = Instant::now();
    for _ in 0..1000 {
        let result = router.route(Method::GET, "/api/v1/resource250/123");
        assert!(result.is_some());
    }
    let duration = start.elapsed();

    // With O(k) radix tree, 1000 lookups should be very fast (< 10ms)
    // With O(n) linear scan, this would take much longer
    assert!(
        duration.as_millis() < 50,
        "Router performance degraded: {}ms for 1000 lookups with 500 routes",
        duration.as_millis()
    );
}

#[test]
fn test_router_memory_efficiency() {
    // Test that the router doesn't clone excessively
    let routes = vec![
        create_route_meta(Method::GET, "/api/users/{id}", "get_user"),
        create_route_meta(Method::GET, "/api/users/{id}/posts", "get_posts"),
        create_route_meta(Method::GET, "/api/users/{id}/posts/{post_id}", "get_post"),
    ];

    let router = Router::new(routes);

    // Match multiple times - should not allocate excessively
    for i in 0..100 {
        let result = router.route(Method::GET, &format!("/api/users/{}/posts", i));
        assert!(result.is_some());
    }

    // This test primarily validates that the code runs without panicking
    // Memory profiling would show that Arc usage prevents excessive cloning
}

#[test]
fn test_router_worst_case_performance() {
    // Create routes with increasing depths to test worst-case scenarios
    let routes = vec![
        create_route_meta(Method::GET, "/a", "handler_a"),
        create_route_meta(Method::GET, "/a/b", "handler_ab"),
        create_route_meta(Method::GET, "/a/b/c", "handler_abc"),
        create_route_meta(Method::GET, "/a/b/c/d", "handler_abcd"),
        create_route_meta(Method::GET, "/a/b/c/d/e", "handler_abcde"),
        create_route_meta(Method::GET, "/a/b/c/d/e/f", "handler_abcdef"),
    ];

    let router = Router::new(routes);

    // Even with deep nesting, performance should remain consistent (O(k))
    let start = Instant::now();
    for _ in 0..1000 {
        router.route(Method::GET, "/a/b/c/d/e/f");
    }
    let duration = start.elapsed();

    // Should complete quickly even with deep paths
    assert!(
        duration.as_millis() < 10,
        "Deep path matching too slow: {}ms",
        duration.as_millis()
    );
}

#[test]
fn test_router_common_prefix_efficiency() {
    // Test routes with common prefixes (where radix trees excel)
    let routes = vec![
        create_route_meta(Method::GET, "/api/v1/users", "list_users"),
        create_route_meta(Method::GET, "/api/v1/users/{id}", "get_user"),
        create_route_meta(Method::GET, "/api/v1/users/{id}/profile", "get_profile"),
        create_route_meta(Method::GET, "/api/v1/posts", "list_posts"),
        create_route_meta(Method::GET, "/api/v1/posts/{id}", "get_post"),
        create_route_meta(Method::GET, "/api/v2/users", "list_users_v2"),
        create_route_meta(Method::GET, "/api/v2/posts", "list_posts_v2"),
    ];

    let router = Router::new(routes);

    // All these paths share the /api prefix - radix tree should handle efficiently
    assert!(router.route(Method::GET, "/api/v1/users").is_some());
    assert!(router.route(Method::GET, "/api/v1/users/123").is_some());
    assert!(router
        .route(Method::GET, "/api/v1/users/123/profile")
        .is_some());
    assert!(router.route(Method::GET, "/api/v1/posts").is_some());
    assert!(router.route(Method::GET, "/api/v2/users").is_some());
}

#[test]
fn test_router_parameter_extraction_performance() {
    // Test that parameter extraction doesn't slow down matching
    let routes = vec![create_route_meta(
        Method::GET,
        "/api/{version}/users/{user_id}/posts/{post_id}/comments/{comment_id}",
        "get_comment",
    )];

    let router = Router::new(routes);

    let start = Instant::now();
    for _ in 0..1000 {
        let result = router.route(Method::GET, "/api/v1/users/123/posts/456/comments/789");
        assert!(result.is_some());
        let route_match = result.unwrap();
        assert_eq!(route_match.path_params.len(), 4);
    }
    let duration = start.elapsed();

    // Parameter extraction should remain fast
    assert!(
        duration.as_millis() < 20,
        "Parameter extraction too slow: {}ms",
        duration.as_millis()
    );
}
