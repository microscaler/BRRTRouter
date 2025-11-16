use super::Router;
use crate::spec::RouteMeta;
use http::Method;

// Helper function to create a basic RouteMeta for testing
fn create_route_meta(method: Method, path: &str, handler: &str) -> RouteMeta {
    use std::collections::HashMap;
    use std::path::PathBuf;

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
    }
}

#[test]
fn test_root_path() {
    let (re, params) = Router::path_to_regex("/");
    assert!(re.is_match("/"));
    assert!(params.is_empty());
}

#[test]
fn test_parameterized_path() {
    let (re, params) = Router::path_to_regex("/items/{id}");
    assert!(re.is_match("/items/123"));
    assert_eq!(params, vec!["id"]);
}

#[test]
fn test_nested_path() {
    let (re, params) = Router::path_to_regex("/a/{b}/c");
    assert!(re.is_match("/a/1/c"));
    assert_eq!(params, vec!["b"]);
}

#[test]
fn test_multiple_parameters() {
    let (re, params) = Router::path_to_regex("/users/{user_id}/posts/{post_id}");
    assert!(re.is_match("/users/123/posts/456"));
    assert_eq!(params, vec!["user_id", "post_id"]);
}

#[test]
fn test_path_regex_no_match() {
    let (re, _) = Router::path_to_regex("/items/{id}");
    assert!(!re.is_match("/items"));
    assert!(!re.is_match("/items/123/extra"));
    assert!(!re.is_match("/other/123"));
}

#[test]
fn test_complex_path_patterns() {
    let (re, params) = Router::path_to_regex("/api/v1/users/{user_id}/settings/{setting_name}");
    assert!(re.is_match("/api/v1/users/42/settings/theme"));
    assert_eq!(params, vec!["user_id", "setting_name"]);
    assert!(!re.is_match("/api/v1/users/42/settings"));
    assert!(!re.is_match("/api/v2/users/42/settings/theme"));
}

#[test]
fn test_router_new_empty_routes() {
    let router = Router::new(vec![]);
    assert!(router.route(Method::GET, "/").is_none());
}

#[test]
fn test_router_new_single_route() {
    let routes = vec![create_route_meta(Method::GET, "/health", "health_check")];
    let router = Router::new(routes);

    let route_match = router.route(Method::GET, "/health").unwrap();
    assert_eq!(route_match.handler_name, "health_check");
    assert!(route_match.path_params.is_empty());
}

#[test]
fn test_router_route_with_parameters() {
    let routes = vec![
        create_route_meta(Method::GET, "/users/{id}", "get_user"),
        create_route_meta(Method::POST, "/users", "create_user"),
    ];
    let router = Router::new(routes);

    // Test parameterized route
    let route_match = router.route(Method::GET, "/users/123").unwrap();
    assert_eq!(route_match.handler_name, "get_user");
    assert_eq!(route_match.path_params.get("id"), Some(&"123".to_string()));

    // Test non-parameterized route
    let route_match = router.route(Method::POST, "/users").unwrap();
    assert_eq!(route_match.handler_name, "create_user");
    assert!(route_match.path_params.is_empty());
}

#[test]
fn test_router_method_filtering() {
    let routes = vec![
        create_route_meta(Method::GET, "/items", "get_items"),
        create_route_meta(Method::POST, "/items", "create_item"),
    ];
    let router = Router::new(routes);

    // Test different methods on same path
    let get_match = router.route(Method::GET, "/items").unwrap();
    assert_eq!(get_match.handler_name, "get_items");

    let post_match = router.route(Method::POST, "/items").unwrap();
    assert_eq!(post_match.handler_name, "create_item");

    // Test unsupported method
    assert!(router.route(Method::PUT, "/items").is_none());
}

#[test]
fn test_router_no_match() {
    let routes = vec![create_route_meta(Method::GET, "/users/{id}", "get_user")];
    let router = Router::new(routes);

    // Test non-matching path
    assert!(router.route(Method::GET, "/posts/123").is_none());

    // Test non-matching method
    assert!(router.route(Method::POST, "/users/123").is_none());

    // Test malformed path
    assert!(router.route(Method::GET, "/users").is_none());
}

#[test]
fn test_router_path_priority() {
    // Routes should be sorted by path length (longest first)
    let routes = vec![
        create_route_meta(Method::GET, "/users", "list_users"),
        create_route_meta(Method::GET, "/users/{id}", "get_user"),
        create_route_meta(Method::GET, "/users/{id}/posts", "get_user_posts"),
    ];
    let router = Router::new(routes);

    // Test that more specific routes match first
    let match1 = router.route(Method::GET, "/users/123/posts").unwrap();
    assert_eq!(match1.handler_name, "get_user_posts");

    let match2 = router.route(Method::GET, "/users/123").unwrap();
    assert_eq!(match2.handler_name, "get_user");

    let match3 = router.route(Method::GET, "/users").unwrap();
    assert_eq!(match3.handler_name, "list_users");
}

#[test]
fn test_router_supported_methods() {
    let methods = vec![
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::PATCH,
        Method::OPTIONS,
        Method::HEAD,
    ];

    let routes: Vec<RouteMeta> = methods
        .into_iter()
        .map(|m| create_route_meta(m, "/test", "test_handler"))
        .collect();

    let router = Router::new(routes);

    // All supported methods should work
    assert!(router.route(Method::GET, "/test").is_some());
    assert!(router.route(Method::POST, "/test").is_some());
    assert!(router.route(Method::PUT, "/test").is_some());
    assert!(router.route(Method::DELETE, "/test").is_some());
    assert!(router.route(Method::PATCH, "/test").is_some());
    assert!(router.route(Method::OPTIONS, "/test").is_some());
    assert!(router.route(Method::HEAD, "/test").is_some());
}

#[test]
fn test_router_unsupported_methods_filtered() {
    // Currently TRACE is supported but could be filtered in the future
    let routes = vec![
        create_route_meta(Method::GET, "/test", "get_handler"),
        create_route_meta(Method::TRACE, "/test", "trace_handler"),
    ];

    let router = Router::new(routes);

    // GET should work
    assert!(router.route(Method::GET, "/test").is_some());

    // TRACE is currently supported (though the comment suggests it might be filtered)
    assert!(router.route(Method::TRACE, "/test").is_some());
}

#[test]
fn test_router_complex_parameter_extraction() {
    let routes = vec![create_route_meta(
        Method::GET,
        "/api/v1/users/{user_id}/posts/{post_id}/comments/{comment_id}",
        "get_comment",
    )];
    let router = Router::new(routes);

    let route_match = router
        .route(Method::GET, "/api/v1/users/123/posts/456/comments/789")
        .unwrap();
    assert_eq!(route_match.handler_name, "get_comment");
    assert_eq!(
        route_match.path_params.get("user_id"),
        Some(&"123".to_string())
    );
    assert_eq!(
        route_match.path_params.get("post_id"),
        Some(&"456".to_string())
    );
    assert_eq!(
        route_match.path_params.get("comment_id"),
        Some(&"789".to_string())
    );
}

#[test]
fn test_router_edge_case_paths() {
    // Test various edge cases in path patterns
    let routes = vec![
        create_route_meta(Method::GET, "/", "root"),
        create_route_meta(Method::GET, "/a", "single_char"),
        create_route_meta(
            Method::GET,
            "/very/long/path/with/many/segments",
            "long_path",
        ),
    ];
    let router = Router::new(routes);

    assert!(router.route(Method::GET, "/").is_some());
    assert!(router.route(Method::GET, "/a").is_some());
    assert!(router
        .route(Method::GET, "/very/long/path/with/many/segments")
        .is_some());
}

#[test]
fn test_route_match_structure() {
    let routes = vec![create_route_meta(Method::GET, "/users/{id}", "get_user")];
    let router = Router::new(routes);

    let route_match = router.route(Method::GET, "/users/123").unwrap();

    // Test RouteMatch structure
    assert_eq!(route_match.handler_name, "get_user");
    assert_eq!(route_match.path_params.get("id"), Some(&"123".to_string()));
    assert!(route_match.query_params.is_empty());
    assert_eq!(route_match.route.method, Method::GET);
    assert_eq!(route_match.route.path_pattern, "/users/{id}");
}

#[test]
fn test_router_different_param_names_same_position() {
    // Test that routes with different parameter names at the same position
    // correctly extract their respective parameters
    let routes = vec![
        create_route_meta(Method::GET, "/users/{user_id}/posts", "get_user_posts"),
        create_route_meta(Method::GET, "/users/{id}/comments", "get_user_comments"),
        create_route_meta(Method::GET, "/users/{uid}/settings", "get_user_settings"),
    ];
    let router = Router::new(routes);

    // Test first route - should extract user_id parameter
    let match1 = router.route(Method::GET, "/users/123/posts").unwrap();
    assert_eq!(match1.handler_name, "get_user_posts");
    assert_eq!(match1.path_params.get("user_id"), Some(&"123".to_string()));
    assert!(match1.path_params.get("id").is_none());
    assert!(match1.path_params.get("uid").is_none());

    // Test second route - should extract id parameter
    let match2 = router.route(Method::GET, "/users/456/comments").unwrap();
    assert_eq!(match2.handler_name, "get_user_comments");
    assert_eq!(match2.path_params.get("id"), Some(&"456".to_string()));
    assert!(match2.path_params.get("user_id").is_none());
    assert!(match2.path_params.get("uid").is_none());

    // Test third route - should extract uid parameter
    let match3 = router.route(Method::GET, "/users/789/settings").unwrap();
    assert_eq!(match3.handler_name, "get_user_settings");
    assert_eq!(match3.path_params.get("uid"), Some(&"789".to_string()));
    assert!(match3.path_params.get("id").is_none());
    assert!(match3.path_params.get("user_id").is_none());
}
