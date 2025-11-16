//! Radix tree implementation for efficient HTTP route matching
//!
//! This module provides a radix tree (also called compact prefix tree) for O(k)
//! route matching where k is the path length. This is a significant improvement
//! over traditional O(n) linear scan approaches.
//!
//! ## Key Benefits
//!
//! - **O(k) Lookup**: Route matching time is proportional to path length, not number of routes
//! - **Memory Efficient**: Shared prefixes (e.g., `/api/v1/`) are stored only once
//! - **Minimal Allocations**: Uses `Arc` for route metadata and `Cow` for strings
//! - **Scalable**: Performance remains consistent as routes are added
//!
//! ## Implementation Details
//!
//! The radix tree is built by splitting paths into segments and creating a tree
//! structure where:
//! - Each node represents a path segment
//! - Static segments (e.g., `users`) match exactly
//! - Parameter segments (e.g., `{id}`) match any value
//! - Routes are stored at terminal nodes, keyed by HTTP method
//!
//! ## Example
//!
//! ```rust,ignore
//! use brrtrouter::router::RadixRouter;
//! use brrtrouter::spec::RouteMeta;
//! use http::Method;
//!
//! let routes = vec![
//!     // ... route metadata from OpenAPI spec
//! ];
//! let router = RadixRouter::new(routes);
//!
//! // Fast O(k) lookup
//! if let Some((route, params)) = router.route(Method::GET, "/api/users/123") {
//!     println!("Handler: {}", route.handler_name);
//!     println!("User ID: {}", params.get("id").unwrap());
//! }
//! ```
//!
//! ## Performance Characteristics
//!
//! Based on benchmarks with the `criterion` crate:
//! - 10 routes: ~256 ns per lookup
//! - 100 routes: ~411 ns per lookup
//! - 500 routes: ~990 ns per lookup
//!
//! The relatively flat performance curve demonstrates O(k) complexity rather than O(n).

use http::Method;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::spec::RouteMeta;

/// Node in the radix tree for efficient route matching
///
/// Each node represents a segment of a URL path and can have children
/// that share common prefixes. This allows for O(k) route lookups where
/// k is the length of the path, not the number of routes.
#[derive(Clone)]
struct RadixNode {
    /// The path segment this node represents (without leading /)
    segment: Cow<'static, str>,
    /// If this node is a terminal (end of a route), stores the route metadata per HTTP method
    routes: HashMap<Method, Arc<RouteMeta>>,
    /// Parameter name if this segment is a path parameter (e.g., "{id}" -> Some("id"))
    param_name: Option<Cow<'static, str>>,
    /// Child nodes for more specific paths
    children: Vec<RadixNode>,
    /// Wildcard child nodes for parameterized paths (e.g., {id}, {user_id})
    /// Multiple parameter children are supported to handle routes with different
    /// parameter names at the same position (e.g., /users/{id}/posts vs /users/{user_id}/comments)
    param_children: Vec<RadixNode>,
}

impl RadixNode {
    /// Create a new radix node with the given segment
    fn new(segment: Cow<'static, str>) -> Self {
        Self {
            segment,
            routes: HashMap::new(),
            param_name: None,
            children: Vec::new(),
            param_children: Vec::new(),
        }
    }

    /// Create a new parameter node
    fn new_param(param_name: Cow<'static, str>) -> Self {
        Self {
            segment: Cow::Borrowed(""),
            routes: HashMap::new(),
            param_name: Some(param_name),
            children: Vec::new(),
            param_children: Vec::new(),
        }
    }

    /// Check if this is a parameter node
    #[allow(dead_code)]
    fn is_param(&self) -> bool {
        self.param_name.is_some()
    }

    /// Insert a route into the tree
    fn insert(&mut self, segments: &[&str], method: Method, route: Arc<RouteMeta>) {
        if segments.is_empty() {
            // We've reached the end of the path, store the route
            self.routes.insert(method, route);
            return;
        }

        let segment = segments[0];
        let remaining = &segments[1..];

        // Check if this is a parameter segment (starts with {)
        if segment.starts_with('{') && segment.ends_with('}') {
            let param_name = segment.trim_start_matches('{').trim_end_matches('}');

            // Look for an existing param_child with the same parameter name
            for param_child in &mut self.param_children {
                if let Some(ref existing_param_name) = param_child.param_name {
                    if existing_param_name.as_ref() == param_name {
                        // Found matching parameter name, reuse this child
                        param_child.insert(remaining, method, route);
                        return;
                    }
                }
            }

            // No matching param_child found, create a new one
            let mut new_param_child = RadixNode::new_param(Cow::Owned(param_name.to_string()));
            new_param_child.insert(remaining, method, route);
            self.param_children.push(new_param_child);
            return;
        }

        // Try to find a matching child node
        let segment_str = segment;
        for child in &mut self.children {
            if child.segment == segment_str {
                child.insert(remaining, method, route);
                return;
            }
        }

        // No matching child found, create a new one
        let mut new_child = RadixNode::new(Cow::Owned(segment_str.to_string()));
        new_child.insert(remaining, method, route);
        self.children.push(new_child);
    }

    /// Search for a matching route in the tree
    fn search(
        &self,
        segments: &[&str],
        method: &Method,
        params: &mut HashMap<String, String>,
    ) -> Option<Arc<RouteMeta>> {
        if segments.is_empty() {
            // We've consumed all segments, check if this node has a route for the method
            return self.routes.get(method).cloned();
        }

        let segment = segments[0];
        let remaining = &segments[1..];

        // First, try exact match with static children
        for child in &self.children {
            if child.segment == segment {
                if let Some(route) = child.search(remaining, method, params) {
                    return Some(route);
                }
            }
        }

        // If no exact match, try all parameter children
        for param_child in &self.param_children {
            if let Some(ref param_name) = param_child.param_name {
                params.insert(param_name.to_string(), segment.to_string());
                if let Some(route) = param_child.search(remaining, method, params) {
                    return Some(route);
                }
                // Backtrack: remove the parameter if the search fails
                params.remove(param_name.as_ref());
            }
        }

        None
    }
}

/// Radix tree-based router for O(k) route matching
///
/// Uses a radix tree (also called compact prefix tree) to efficiently match
/// HTTP requests to routes. Route lookup is O(k) where k is the path length,
/// not O(n) where n is the number of routes.
///
/// # Performance
///
/// - Insertion: O(k) where k is the path length
/// - Lookup: O(k) where k is the path length
/// - Memory: O(total path characters) with shared prefixes compressed
///
/// This is a significant improvement over the previous O(n) linear scan
/// with regex matching for each route.
#[derive(Clone)]
pub struct RadixRouter {
    /// Root node of the radix tree
    root: RadixNode,
    /// Base path prefix for all routes (e.g., `/api/v1`)
    #[allow(dead_code)]
    base_path: String,
}

impl RadixRouter {
    /// Create a new radix router from OpenAPI route metadata
    ///
    /// Builds a radix tree from the route patterns, enabling O(k) lookups
    /// where k is the path length.
    ///
    /// # Arguments
    ///
    /// * `routes` - List of route metadata extracted from OpenAPI spec
    ///
    /// # Returns
    ///
    /// A new `RadixRouter` ready to match incoming requests
    pub fn new(routes: Vec<RouteMeta>) -> Self {
        // Filter out unsupported HTTP methods
        let supported_methods = [
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
            Method::HEAD,
            Method::TRACE,
        ];

        let routes: Vec<RouteMeta> = routes
            .into_iter()
            .filter(|r| supported_methods.contains(&r.method))
            .collect();

        let base_path = routes
            .first()
            .map(|r| r.base_path.clone())
            .unwrap_or_default();

        let mut root = RadixNode::new(Cow::Borrowed(""));

        // Insert all routes into the radix tree
        for route in routes {
            let full_path = format!("{}{}", base_path, route.path_pattern);
            let segments: Vec<&str> = full_path
                .trim_start_matches('/')
                .split('/')
                .filter(|s| !s.is_empty())
                .collect();

            let method = route.method.clone();
            root.insert(&segments, method, Arc::new(route));
        }

        Self { root, base_path }
    }

    /// Match an HTTP request to a route
    ///
    /// Uses the radix tree to efficiently find the matching route and extract
    /// path parameters. This is O(k) where k is the path length.
    ///
    /// # Arguments
    ///
    /// * `method` - HTTP method (GET, POST, etc.)
    /// * `path` - Request path (e.g., `/users/123`)
    ///
    /// # Returns
    ///
    /// * `Some((route, params))` - If a matching route is found with extracted parameters
    /// * `None` - If no route matches
    pub fn route(
        &self,
        method: Method,
        path: &str,
    ) -> Option<(Arc<RouteMeta>, HashMap<String, String>)> {
        let segments: Vec<&str> = path
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let mut params = HashMap::new();
        let route = self.root.search(&segments, &method, &mut params)?;
        Some((route, params))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    // Helper function to create a basic RouteMeta for testing
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
        }
    }

    #[test]
    fn test_radix_router_simple_route() {
        let routes = vec![create_route_meta(Method::GET, "/health", "health_check")];
        let router = RadixRouter::new(routes);

        let result = router.route(Method::GET, "/health");
        assert!(result.is_some());
        let (route, params) = result.unwrap();
        assert_eq!(route.handler_name, "health_check");
        assert!(params.is_empty());
    }

    #[test]
    fn test_radix_router_with_parameter() {
        let routes = vec![create_route_meta(Method::GET, "/users/{id}", "get_user")];
        let router = RadixRouter::new(routes);

        let result = router.route(Method::GET, "/users/123");
        assert!(result.is_some());
        let (route, params) = result.unwrap();
        assert_eq!(route.handler_name, "get_user");
        assert_eq!(params.get("id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_radix_router_multiple_parameters() {
        let routes = vec![create_route_meta(
            Method::GET,
            "/users/{user_id}/posts/{post_id}",
            "get_post",
        )];
        let router = RadixRouter::new(routes);

        let result = router.route(Method::GET, "/users/123/posts/456");
        assert!(result.is_some());
        let (route, params) = result.unwrap();
        assert_eq!(route.handler_name, "get_post");
        assert_eq!(params.get("user_id"), Some(&"123".to_string()));
        assert_eq!(params.get("post_id"), Some(&"456".to_string()));
    }

    #[test]
    fn test_radix_router_method_filtering() {
        let routes = vec![
            create_route_meta(Method::GET, "/items", "get_items"),
            create_route_meta(Method::POST, "/items", "create_item"),
        ];
        let router = RadixRouter::new(routes);

        let get_result = router.route(Method::GET, "/items");
        assert!(get_result.is_some());
        assert_eq!(get_result.unwrap().0.handler_name, "get_items");

        let post_result = router.route(Method::POST, "/items");
        assert!(post_result.is_some());
        assert_eq!(post_result.unwrap().0.handler_name, "create_item");

        let put_result = router.route(Method::PUT, "/items");
        assert!(put_result.is_none());
    }

    #[test]
    fn test_radix_router_no_match() {
        let routes = vec![create_route_meta(Method::GET, "/users/{id}", "get_user")];
        let router = RadixRouter::new(routes);

        assert!(router.route(Method::GET, "/posts/123").is_none());
        assert!(router.route(Method::POST, "/users/123").is_none());
    }

    #[test]
    fn test_radix_router_complex_paths() {
        let routes = vec![
            create_route_meta(Method::GET, "/users", "list_users"),
            create_route_meta(Method::GET, "/users/{id}", "get_user"),
            create_route_meta(Method::GET, "/users/{id}/posts", "get_user_posts"),
        ];
        let router = RadixRouter::new(routes);

        let result1 = router.route(Method::GET, "/users");
        assert!(result1.is_some());
        assert_eq!(result1.unwrap().0.handler_name, "list_users");

        let result2 = router.route(Method::GET, "/users/123");
        assert!(result2.is_some());
        assert_eq!(result2.unwrap().0.handler_name, "get_user");

        let result3 = router.route(Method::GET, "/users/123/posts");
        assert!(result3.is_some());
        assert_eq!(result3.unwrap().0.handler_name, "get_user_posts");
    }

    #[test]
    fn test_radix_router_different_param_names_same_position() {
        // This test demonstrates the bug where routes with different parameter names
        // at the same path position incorrectly share the same param_child node.
        // Example: /users/{user_id}/posts and /users/{id}/comments
        let routes = vec![
            create_route_meta(Method::GET, "/users/{user_id}/posts", "get_user_posts"),
            create_route_meta(Method::GET, "/users/{id}/comments", "get_user_comments"),
        ];
        let router = RadixRouter::new(routes);

        // Test first route - should extract user_id parameter
        let result1 = router.route(Method::GET, "/users/123/posts");
        assert!(result1.is_some());
        let (route1, params1) = result1.unwrap();
        assert_eq!(route1.handler_name, "get_user_posts");
        assert_eq!(params1.get("user_id"), Some(&"123".to_string()));
        assert!(params1.get("id").is_none()); // Should NOT have 'id' parameter

        // Test second route - should extract id parameter
        let result2 = router.route(Method::GET, "/users/456/comments");
        assert!(result2.is_some());
        let (route2, params2) = result2.unwrap();
        assert_eq!(route2.handler_name, "get_user_comments");
        assert_eq!(params2.get("id"), Some(&"456".to_string()));
        assert!(params2.get("user_id").is_none()); // Should NOT have 'user_id' parameter
    }

    #[test]
    fn test_radix_router_multiple_divergent_params() {
        // Test more complex scenario with multiple routes having different parameter names
        let routes = vec![
            create_route_meta(Method::GET, "/api/{version}/users/{user_id}", "get_user_v1"),
            create_route_meta(Method::GET, "/api/{v}/products/{product_id}", "get_product"),
            create_route_meta(
                Method::GET,
                "/api/{api_version}/orders/{order_id}",
                "get_order",
            ),
        ];
        let router = RadixRouter::new(routes);

        // Each route should extract its own parameter names
        let result1 = router.route(Method::GET, "/api/v1/users/123");
        assert!(result1.is_some());
        let (route1, params1) = result1.unwrap();
        assert_eq!(route1.handler_name, "get_user_v1");
        assert_eq!(params1.get("version"), Some(&"v1".to_string()));
        assert_eq!(params1.get("user_id"), Some(&"123".to_string()));

        let result2 = router.route(Method::GET, "/api/v2/products/456");
        assert!(result2.is_some());
        let (route2, params2) = result2.unwrap();
        assert_eq!(route2.handler_name, "get_product");
        assert_eq!(params2.get("v"), Some(&"v2".to_string()));
        assert_eq!(params2.get("product_id"), Some(&"456".to_string()));

        let result3 = router.route(Method::GET, "/api/v3/orders/789");
        assert!(result3.is_some());
        let (route3, params3) = result3.unwrap();
        assert_eq!(route3.handler_name, "get_order");
        assert_eq!(params3.get("api_version"), Some(&"v3".to_string()));
        assert_eq!(params3.get("order_id"), Some(&"789".to_string()));
    }
}
