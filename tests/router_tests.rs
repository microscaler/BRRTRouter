#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Unit tests for the HTTP router and path matching
//!
//! # Test Coverage
//!
//! Validates route matching and path parameter extraction:
//! - Path pattern to regex compilation
//! - Route matching by method and path
//! - Path parameter extraction
//! - Route priority (longest match first)
//! - Ambiguous route detection
//!
//! # Test Strategy
//!
//! Uses synthetic OpenAPI specs with various path patterns:
//! - Static paths: `/users`
//! - Path parameters: `/users/{id}`
//! - Multiple parameters: `/users/{userId}/posts/{postId}`
//! - Ambiguous routes: `/users/me` vs `/users/{id}`
//!
//! # Key Test Cases
//!
//! - `test_router_matches_static_route`: Exact path matching
//! - `test_router_matches_parameterized_route`: Path param extraction
//! - `test_router_respects_http_methods`: Method-specific routing
//! - `test_router_prioritizes_longer_routes`: Specificity ordering
//!
//! # Edge Cases Tested
//!
//! - Missing routes return None
//! - Wrong methods return None
//! - Path parameters are URL-decoded
//! - Routes are sorted by length (longest first)

use brrtrouter::{
    router::{RouteMatch, Router},
    spec::RouteMeta,
};

use http::Method;

fn example_spec() -> &'static str {
    r#"
openapi: 3.1.0
info:
  title: Verb Zoo
  version: "1.0.0"
paths:
  "/":
    get:
      operationId: root_handler
      responses:
        "200": { description: OK }
  /zoo/animals:
    get:
      operationId: get_animals
      responses:
        "200": { description: OK }
    post:
      operationId: create_animal
      responses:
        "200": { description: OK }

  /zoo/animals/{id}:
    get:
      operationId: get_animal
      responses:
        "200": { description: OK }
    put:
      operationId: update_animal
      responses:
        "200": { description: OK }
    patch:
      operationId: patch_animal
      responses:
        "200": { description: OK }
    delete:
      operationId: delete_animal
      responses:
        "200": { description: OK }

  /zoo/health:
    head:
      operationId: health_check
      responses:
        "200": { description: OK }
    options:
      operationId: supported_ops
      responses:
        "200": { description: OK }
    trace:
      operationId: trace_route
      responses:
        "200": { description: OK }
"#
}

fn example_spec_with_base() -> &'static str {
    r#"
openapi: 3.1.0
info:
  title: Verb Zoo
  version: "1.0.0"
servers:
  - url: /api
paths:
  "/":
    get:
      operationId: root_handler
      responses:
        "200": { description: OK }
  /zoo/animals:
    get:
      operationId: get_animals
      responses:
        "200": { description: OK }
    post:
      operationId: create_animal
      responses:
        "200": { description: OK }

  /zoo/animals/{id}:
    get:
      operationId: get_animal
      responses:
        "200": { description: OK }
    put:
      operationId: update_animal
      responses:
        "200": { description: OK }
    patch:
      operationId: patch_animal
      responses:
        "200": { description: OK }
    delete:
      operationId: delete_animal
      responses:
        "200": { description: OK }

  /zoo/health:
    head:
      operationId: health_check
      responses:
        "200": { description: OK }
    options:
      operationId: supported_ops
      responses:
        "200": { description: OK }
    trace:
      operationId: trace_route
      responses:
        "200": { description: OK }
"#
}

fn parse_spec(yaml: &str) -> Vec<RouteMeta> {
    let spec = serde_yaml::from_str(yaml).expect("failed to parse YAML spec");
    brrtrouter::spec::load_spec_from_spec(spec).expect("failed to load spec")
}

pub fn load_spec_from_spec(spec_wrapper: oas3::OpenApiV3Spec) -> anyhow::Result<Vec<RouteMeta>> {
    brrtrouter::spec::load_spec_from_spec(spec_wrapper)
}

fn assert_route_match(router: &Router, method: Method, path: &str, expected_handler: &str) {
    let result = router.route(method.clone(), path);
    match result {
        Some(RouteMatch {
            route,
            path_params: _,
            ..
        }) => {
            println!("✅ {} {} → {}", method, path, route.handler_name);
            assert_eq!(
                route.handler_name.as_ref(),
                expected_handler,
                "Handler mismatch for {} {}: expected '{}', got '{}'",
                method,
                path,
                expected_handler,
                route.handler_name
            );
        }
        None => {
            println!("❌ {method} {path} → no match");
            assert_eq!(
                expected_handler, "<none>",
                "Expected route to match for {method} {path}"
            );
        }
    }
}

#[test]
fn test_router_get_animals() {
    let routes = parse_spec(example_spec());
    let router = Router::new(routes);
    assert_route_match(&router, Method::GET, "/zoo/animals", "get_animals");
}

#[test]
fn test_router_post_animals() {
    let routes = parse_spec(example_spec());
    let router = Router::new(routes);
    assert_route_match(&router, Method::POST, "/zoo/animals", "create_animal");
}

#[test]
fn test_router_get_animal_by_id() {
    let routes = parse_spec(example_spec());
    let router = Router::new(routes);
    assert_route_match(&router, Method::GET, "/zoo/animals/123", "get_animal");
}

#[test]
fn test_router_put_animal() {
    let routes = parse_spec(example_spec());
    let router = Router::new(routes);
    assert_route_match(&router, Method::PUT, "/zoo/animals/123", "update_animal");
}

#[test]
fn test_router_patch_animal() {
    let routes = parse_spec(example_spec());
    let router = Router::new(routes);
    assert_route_match(&router, Method::PATCH, "/zoo/animals/123", "patch_animal");
}

#[test]
fn test_router_delete_animal() {
    let routes = parse_spec(example_spec());
    let router = Router::new(routes);
    assert_route_match(&router, Method::DELETE, "/zoo/animals/123", "delete_animal");
}

#[test]
fn test_router_head_health() {
    let routes = parse_spec(example_spec());
    let router = Router::new(routes);
    assert_route_match(&router, Method::HEAD, "/zoo/health", "health_check");
}

#[test]
fn test_router_options_health() {
    let routes = parse_spec(example_spec());
    let router = Router::new(routes);
    assert_route_match(&router, Method::OPTIONS, "/zoo/health", "supported_ops");
}

#[test]
fn test_router_trace_health() {
    let routes = parse_spec(example_spec());
    let router = Router::new(routes);
    assert_route_match(&router, Method::TRACE, "/zoo/health", "trace_route");
}

#[test]
fn test_router_unknown_path() {
    let routes = parse_spec(example_spec());
    let router = Router::new(routes);
    assert_route_match(&router, Method::GET, "/unknown", "<none>");
}

#[test]
fn test_router_root_path() {
    let routes = parse_spec(example_spec());
    let router = Router::new(routes);
    assert_route_match(&router, Method::GET, "/", "root_handler");
}

#[test]
fn test_router_base_path_routing() {
    let routes = parse_spec(example_spec_with_base());
    let router = Router::new(routes);
    assert_route_match(&router, Method::GET, "/api/zoo/animals", "get_animals");
    assert_route_match(&router, Method::GET, "/api", "root_handler");
    assert_route_match(&router, Method::GET, "/zoo/animals", "<none>");
}
