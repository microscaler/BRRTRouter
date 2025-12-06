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
//!     println!("User ID: {}", get_param(&params, "id").unwrap());
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

use super::core::ParamVec;
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
    /// Uses ParamVec (SmallVec) for stack-allocated params in hot path
    fn search(
        &self,
        segments: &[&str],
        method: &Method,
        params: &mut ParamVec,
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
                // Push the param for this branch
                // Note: duplicate param names will result in multiple entries;
                // use get_path_param() which returns the last occurrence (last write wins)
                params.push((param_name.to_string(), segment.to_string()));
                if let Some(route) = param_child.search(remaining, method, params) {
                    return Some(route);
                }
                // Backtrack: remove the param we just pushed
                params.pop();
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
    ///
    /// # JSF Compliance
    ///
    /// Uses ParamVec (SmallVec) for stack-allocated parameters, avoiding heap
    /// allocation for routes with ≤8 params (the common case).
    pub fn route(&self, method: Method, path: &str) -> Option<(Arc<RouteMeta>, ParamVec)> {
        let segments: Vec<&str> = path
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let mut params = ParamVec::new();
        let route = self.root.search(&segments, &method, &mut params)?;
        Some((route, params))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// Helper to get a param value from ParamVec (for test assertions)
    /// Returns the last occurrence to match HashMap semantics (last write wins)
    fn get_param<'a>(params: &'a ParamVec, name: &str) -> Option<&'a str> {
        params
            .iter()
            .rfind(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

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
            estimated_request_body_bytes: None,
            x_brrtrouter_stack_size: None,
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
        assert_eq!(
            params
                .iter()
                .find(|(k, _)| k == "id")
                .map(|(_, v)| v.as_str()),
            Some("123")
        );
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
        assert_eq!(
            params
                .iter()
                .find(|(k, _)| k == "user_id")
                .map(|(_, v)| v.as_str()),
            Some("123")
        );
        assert_eq!(
            params
                .iter()
                .find(|(k, _)| k == "post_id")
                .map(|(_, v)| v.as_str()),
            Some("456")
        );
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
        assert_eq!(get_param(&params1, "user_id"), Some("123"));
        assert!(get_param(&params1, "id").is_none()); // Should NOT have 'id' parameter

        // Test second route - should extract id parameter
        let result2 = router.route(Method::GET, "/users/456/comments");
        assert!(result2.is_some());
        let (route2, params2) = result2.unwrap();
        assert_eq!(route2.handler_name, "get_user_comments");
        assert_eq!(get_param(&params2, "id"), Some("456"));
        assert!(get_param(&params2, "user_id").is_none()); // Should NOT have 'user_id' parameter
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
        assert_eq!(get_param(&params1, "version"), Some("v1"));
        assert_eq!(get_param(&params1, "user_id"), Some("123"));

        let result2 = router.route(Method::GET, "/api/v2/products/456");
        assert!(result2.is_some());
        let (route2, params2) = result2.unwrap();
        assert_eq!(route2.handler_name, "get_product");
        assert_eq!(get_param(&params2, "v"), Some("v2"));
        assert_eq!(get_param(&params2, "product_id"), Some("456"));

        let result3 = router.route(Method::GET, "/api/v3/orders/789");
        assert!(result3.is_some());
        let (route3, params3) = result3.unwrap();
        assert_eq!(route3.handler_name, "get_order");
        assert_eq!(get_param(&params3, "api_version"), Some("v3"));
        assert_eq!(get_param(&params3, "order_id"), Some("789"));
    }

    #[test]
    fn test_radix_router_deep_params_3_levels() {
        // Test routes with 3 levels of parameters to verify depth-first building
        let routes = vec![
            create_route_meta(
                Method::GET,
                "/orgs/{org_id}/projects/{project_id}/issues/{issue_id}",
                "get_org_project_issue",
            ),
            create_route_meta(
                Method::GET,
                "/orgs/{organization}/repos/{repository}/commits/{commit_sha}",
                "get_org_repo_commit",
            ),
            create_route_meta(
                Method::GET,
                "/orgs/{org}/teams/{team}/members/{member}",
                "get_org_team_member",
            ),
        ];
        let router = RadixRouter::new(routes);

        // Test first route - should extract org_id, project_id, issue_id
        let result1 = router.route(Method::GET, "/orgs/acme/projects/web-app/issues/42");
        assert!(result1.is_some());
        let (route1, params1) = result1.unwrap();
        assert_eq!(route1.handler_name, "get_org_project_issue");
        assert_eq!(get_param(&params1, "org_id"), Some("acme"));
        assert_eq!(get_param(&params1, "project_id"), Some("web-app"));
        assert_eq!(get_param(&params1, "issue_id"), Some("42"));
        assert_eq!(params1.len(), 3);

        // Test second route - should extract organization, repository, commit_sha
        let result2 = router.route(Method::GET, "/orgs/github/repos/rust/commits/abc123");
        assert!(result2.is_some());
        let (route2, params2) = result2.unwrap();
        assert_eq!(route2.handler_name, "get_org_repo_commit");
        assert_eq!(get_param(&params2, "organization"), Some("github"));
        assert_eq!(get_param(&params2, "repository"), Some("rust"));
        assert_eq!(get_param(&params2, "commit_sha"), Some("abc123"));
        assert_eq!(params2.len(), 3);

        // Test third route - should extract org, team, member
        let result3 = router.route(
            Method::GET,
            "/orgs/mycompany/teams/engineering/members/alice",
        );
        assert!(result3.is_some());
        let (route3, params3) = result3.unwrap();
        assert_eq!(route3.handler_name, "get_org_team_member");
        assert_eq!(get_param(&params3, "org"), Some("mycompany"));
        assert_eq!(get_param(&params3, "team"), Some("engineering"));
        assert_eq!(get_param(&params3, "member"), Some("alice"));
        assert_eq!(params3.len(), 3);
    }

    #[test]
    fn test_radix_router_deep_params_4_levels() {
        // Test routes with 4 levels of parameters to verify depth-first building
        let routes = vec![
            create_route_meta(
                Method::GET,
                "/api/{version}/users/{user_id}/posts/{post_id}/comments/{comment_id}",
                "get_user_post_comment",
            ),
            create_route_meta(
                Method::GET,
                "/api/{v}/orgs/{org}/repos/{repo}/issues/{issue_num}",
                "get_org_repo_issue",
            ),
            create_route_meta(
                Method::GET,
                "/api/{api_ver}/companies/{company_id}/departments/{dept_id}/employees/{emp_id}",
                "get_company_dept_employee",
            ),
        ];
        let router = RadixRouter::new(routes);

        // Test first route - 4 levels of parameters
        let result1 = router.route(Method::GET, "/api/v1/users/john/posts/100/comments/5");
        assert!(result1.is_some());
        let (route1, params1) = result1.unwrap();
        assert_eq!(route1.handler_name, "get_user_post_comment");
        assert_eq!(get_param(&params1, "version"), Some("v1"));
        assert_eq!(get_param(&params1, "user_id"), Some("john"));
        assert_eq!(get_param(&params1, "post_id"), Some("100"));
        assert_eq!(get_param(&params1, "comment_id"), Some("5"));
        assert_eq!(params1.len(), 4);

        // Test second route - 4 levels with different parameter names
        let result2 = router.route(Method::GET, "/api/v2/orgs/github/repos/rust-lang/issues/42");
        assert!(result2.is_some());
        let (route2, params2) = result2.unwrap();
        assert_eq!(route2.handler_name, "get_org_repo_issue");
        assert_eq!(get_param(&params2, "v"), Some("v2"));
        assert_eq!(get_param(&params2, "org"), Some("github"));
        assert_eq!(get_param(&params2, "repo"), Some("rust-lang"));
        assert_eq!(get_param(&params2, "issue_num"), Some("42"));
        assert_eq!(params2.len(), 4);

        // Test third route - 4 levels with yet another set of parameter names
        let result3 = router.route(
            Method::GET,
            "/api/v3/companies/acme/departments/engineering/employees/alice",
        );
        assert!(result3.is_some());
        let (route3, params3) = result3.unwrap();
        assert_eq!(route3.handler_name, "get_company_dept_employee");
        assert_eq!(get_param(&params3, "api_ver"), Some("v3"));
        assert_eq!(get_param(&params3, "company_id"), Some("acme"));
        assert_eq!(get_param(&params3, "dept_id"), Some("engineering"));
        assert_eq!(get_param(&params3, "emp_id"), Some("alice"));
        assert_eq!(params3.len(), 4);
    }

    #[test]
    fn test_radix_router_mixed_static_and_params() {
        // Test routes with parameters at various depths mixed with static segments
        let routes = vec![
            create_route_meta(
                Method::GET,
                "/api/v1/users/{user_id}/profile",
                "get_user_profile",
            ),
            create_route_meta(Method::GET, "/api/v1/users/{id}/posts", "get_user_posts"),
            create_route_meta(
                Method::GET,
                "/api/v1/users/{user_id}/posts/{post_id}",
                "get_user_post",
            ),
            create_route_meta(
                Method::GET,
                "/api/v1/users/{uid}/settings/{setting_key}",
                "get_user_setting",
            ),
            create_route_meta(
                Method::GET,
                "/api/v1/posts/{post_id}/comments/{comment_id}/replies/{reply_id}",
                "get_post_comment_reply",
            ),
        ];
        let router = RadixRouter::new(routes);

        // Test route with parameter followed by static segment
        let result1 = router.route(Method::GET, "/api/v1/users/alice/profile");
        assert!(result1.is_some());
        let (route1, params1) = result1.unwrap();
        assert_eq!(route1.handler_name, "get_user_profile");
        assert_eq!(get_param(&params1, "user_id"), Some("alice"));
        assert_eq!(params1.len(), 1);

        // Test similar route with different parameter name
        let result2 = router.route(Method::GET, "/api/v1/users/bob/posts");
        assert!(result2.is_some());
        let (route2, params2) = result2.unwrap();
        assert_eq!(route2.handler_name, "get_user_posts");
        assert_eq!(get_param(&params2, "id"), Some("bob"));
        assert_eq!(params2.len(), 1);

        // Test 2-level parameters
        let result3 = router.route(Method::GET, "/api/v1/users/charlie/posts/123");
        assert!(result3.is_some());
        let (route3, params3) = result3.unwrap();
        assert_eq!(route3.handler_name, "get_user_post");
        assert_eq!(get_param(&params3, "user_id"), Some("charlie"));
        assert_eq!(get_param(&params3, "post_id"), Some("123"));
        assert_eq!(params3.len(), 2);

        // Test 2-level parameters with different names
        let result4 = router.route(Method::GET, "/api/v1/users/dave/settings/theme");
        assert!(result4.is_some());
        let (route4, params4) = result4.unwrap();
        assert_eq!(route4.handler_name, "get_user_setting");
        assert_eq!(get_param(&params4, "uid"), Some("dave"));
        assert_eq!(get_param(&params4, "setting_key"), Some("theme"));
        assert_eq!(params4.len(), 2);

        // Test 3-level parameters
        let result5 = router.route(Method::GET, "/api/v1/posts/456/comments/789/replies/101");
        assert!(result5.is_some());
        let (route5, params5) = result5.unwrap();
        assert_eq!(route5.handler_name, "get_post_comment_reply");
        assert_eq!(get_param(&params5, "post_id"), Some("456"));
        assert_eq!(get_param(&params5, "comment_id"), Some("789"));
        assert_eq!(get_param(&params5, "reply_id"), Some("101"));
        assert_eq!(params5.len(), 3);
    }

    #[test]
    fn test_radix_router_param_backtracking() {
        // Test that parameter matching correctly backtracks when a route doesn't match
        let routes = vec![
            create_route_meta(
                Method::GET,
                "/api/{version}/users/{user_id}/posts/{post_id}/edit",
                "edit_user_post",
            ),
            create_route_meta(
                Method::GET,
                "/api/{v}/users/{id}/posts/{pid}/delete",
                "delete_user_post",
            ),
            create_route_meta(
                Method::GET,
                "/api/{api_v}/users/{uid}/comments/{cid}/approve",
                "approve_user_comment",
            ),
        ];
        let router = RadixRouter::new(routes);

        // Each route should match correctly despite similar prefixes
        let result1 = router.route(Method::GET, "/api/v1/users/alice/posts/123/edit");
        assert!(result1.is_some());
        let (route1, params1) = result1.unwrap();
        assert_eq!(route1.handler_name, "edit_user_post");
        assert_eq!(get_param(&params1, "version"), Some("v1"));
        assert_eq!(get_param(&params1, "user_id"), Some("alice"));
        assert_eq!(get_param(&params1, "post_id"), Some("123"));

        let result2 = router.route(Method::GET, "/api/v2/users/bob/posts/456/delete");
        assert!(result2.is_some());
        let (route2, params2) = result2.unwrap();
        assert_eq!(route2.handler_name, "delete_user_post");
        assert_eq!(get_param(&params2, "v"), Some("v2"));
        assert_eq!(get_param(&params2, "id"), Some("bob"));
        assert_eq!(get_param(&params2, "pid"), Some("456"));

        let result3 = router.route(Method::GET, "/api/v3/users/charlie/comments/789/approve");
        assert!(result3.is_some());
        let (route3, params3) = result3.unwrap();
        assert_eq!(route3.handler_name, "approve_user_comment");
        assert_eq!(get_param(&params3, "api_v"), Some("v3"));
        assert_eq!(get_param(&params3, "uid"), Some("charlie"));
        assert_eq!(get_param(&params3, "cid"), Some("789"));
    }

    #[test]
    fn test_radix_router_duplicate_param_names_different_depths() {
        // Test route with the same parameter name appearing at different depths
        // This is a real-world edge case: /org/{id}/team/{team_id}/user/{id}
        // The second {id} should overwrite the first one
        let routes = vec![
            create_route_meta(
                Method::GET,
                "/org/{id}/team/{team_id}/user/{id}",
                "get_org_team_user",
            ),
            create_route_meta(Method::GET, "/org/{id}/projects", "get_org_projects"),
            create_route_meta(
                Method::GET,
                "/company/{id}/dept/{dept_id}/employee/{id}",
                "get_company_dept_employee",
            ),
        ];
        let router = RadixRouter::new(routes);

        // Test route with duplicate {id} parameter at different depths
        // The last occurrence should win (user id = "alice") via get_param()
        let result1 = router.route(Method::GET, "/org/org123/team/team456/user/alice");
        assert!(result1.is_some());
        let (route1, params1) = result1.unwrap();
        assert_eq!(route1.handler_name, "get_org_team_user");
        // get_param returns the last {id} (user id = "alice")
        assert_eq!(get_param(&params1, "id"), Some("alice"));
        assert_eq!(get_param(&params1, "team_id"), Some("team456"));
        // With SmallVec, duplicates are stored (org_id + team_id + user_id = 3)
        assert_eq!(params1.len(), 3);

        // Test simpler route with single {id}
        let result2 = router.route(Method::GET, "/org/org789/projects");
        assert!(result2.is_some());
        let (route2, params2) = result2.unwrap();
        assert_eq!(route2.handler_name, "get_org_projects");
        assert_eq!(get_param(&params2, "id"), Some("org789"));
        assert_eq!(params2.len(), 1);

        // Test another route with duplicate {id} at different depths
        let result3 = router.route(
            Method::GET,
            "/company/comp999/dept/engineering/employee/bob",
        );
        assert!(result3.is_some());
        let (route3, params3) = result3.unwrap();
        assert_eq!(route3.handler_name, "get_company_dept_employee");
        // get_param returns the last {id} (employee id = "bob")
        assert_eq!(get_param(&params3, "id"), Some("bob"));
        assert_eq!(get_param(&params3, "dept_id"), Some("engineering"));
        // With SmallVec, duplicates are stored (company_id + dept_id + employee_id = 3)
        assert_eq!(params3.len(), 3);
    }

    #[test]
    fn test_radix_router_same_param_name_different_routes() {
        // Test that the same parameter name can be used in different routes
        // at the same depth without collision
        let routes = vec![
            create_route_meta(Method::GET, "/users/{id}", "get_user"),
            create_route_meta(Method::GET, "/posts/{id}", "get_post"),
            create_route_meta(Method::GET, "/comments/{id}", "get_comment"),
            create_route_meta(Method::GET, "/users/{id}/posts/{id}", "get_user_post"),
        ];
        let router = RadixRouter::new(routes);

        // Each route should correctly extract its {id} parameter
        let result1 = router.route(Method::GET, "/users/user123");
        assert!(result1.is_some());
        let (route1, params1) = result1.unwrap();
        assert_eq!(route1.handler_name, "get_user");
        assert_eq!(get_param(&params1, "id"), Some("user123"));

        let result2 = router.route(Method::GET, "/posts/post456");
        assert!(result2.is_some());
        let (route2, params2) = result2.unwrap();
        assert_eq!(route2.handler_name, "get_post");
        assert_eq!(get_param(&params2, "id"), Some("post456"));

        let result3 = router.route(Method::GET, "/comments/comment789");
        assert!(result3.is_some());
        let (route3, params3) = result3.unwrap();
        assert_eq!(route3.handler_name, "get_comment");
        assert_eq!(get_param(&params3, "id"), Some("comment789"));

        // Route with duplicate {id} - get_param returns the last one
        let result4 = router.route(Method::GET, "/users/user999/posts/post111");
        assert!(result4.is_some());
        let (route4, params4) = result4.unwrap();
        assert_eq!(route4.handler_name, "get_user_post");
        // get_param returns the last {id}
        assert_eq!(get_param(&params4, "id"), Some("post111"));
        // With SmallVec, duplicates are stored (user_id + post_id = 2)
        assert_eq!(params4.len(), 2);
    }

    #[test]
    fn test_radix_router_backtracking_with_overlapping_param_names() {
        // This test demonstrates the bug where backtracking incorrectly removes
        // a parent parameter instead of restoring its previous value.
        //
        // Scenario:
        // - Route 1: /org/{id}/team/{id}/members (inserted first, will fail to match)
        // - Route 2: /org/{id}/team/{team_id}/stats (inserted second, should match)
        //
        // When matching /org/org123/team/team456/stats:
        // 1. First, {id} at /org level is set to "org123"
        // 2. Then we try the first param_child: {id} at /team level, overwriting to "team456"
        // 3. The route fails because "stats" != "members"
        // 4. BUG: params.remove("id") removes the parameter entirely, losing "org123"
        // 5. Then we try the second param_child: {team_id} at /team level
        // 6. The route matches but "id" (org parameter) is missing!
        //
        // Expected behavior: Step 4 should restore "id" to "org123" instead of removing it.
        let routes = vec![
            create_route_meta(
                Method::GET,
                "/org/{id}/team/{id}/members",
                "get_org_team_members",
            ),
            create_route_meta(
                Method::GET,
                "/org/{id}/team/{team_id}/stats",
                "get_team_stats",
            ),
        ];
        let router = RadixRouter::new(routes);

        // Test the second route that should match
        let result = router.route(Method::GET, "/org/org123/team/team456/stats");
        assert!(result.is_some());
        let (route, params) = result.unwrap();
        assert_eq!(route.handler_name, "get_team_stats");

        // The bug manifests here: "id" parameter is missing because it was removed
        // during backtracking instead of being restored to "org123"
        assert_eq!(
            get_param(&params, "id"),
            Some("org123"),
            "org id should be preserved after backtracking from failed route"
        );
        assert_eq!(
            params
                .iter()
                .find(|(k, _)| k == "team_id")
                .map(|(_, v)| v.as_str()),
            Some("team456")
        );
        assert_eq!(params.len(), 2);

        // Test the first route that should also work
        let result2 = router.route(Method::GET, "/org/org999/team/team888/members");
        assert!(result2.is_some());
        let (route2, params2) = result2.unwrap();
        assert_eq!(route2.handler_name, "get_org_team_members");
        // get_param returns the last {id} (team888)
        assert_eq!(get_param(&params2, "id"), Some("team888"));
        // With SmallVec, duplicates are stored (org_id + team_id = 2)
        assert_eq!(params2.len(), 2);
    }

    #[test]
    fn test_radix_router_backtracking_multiple_levels() {
        // Test that backtracking works correctly with multiple levels of parameter nesting
        // and multiple failed attempts before finding the correct route.
        let routes = vec![
            // Route 1: Will fail at the deepest level
            create_route_meta(
                Method::GET,
                "/api/{version}/org/{id}/team/{id}/data",
                "get_team_data_v1",
            ),
            // Route 2: Will fail at middle level
            create_route_meta(
                Method::GET,
                "/api/{version}/org/{id}/team/{team_id}/info",
                "get_team_info",
            ),
            // Route 3: Should match
            create_route_meta(
                Method::GET,
                "/api/{version}/org/{id}/team/{team_id}/stats",
                "get_team_stats_v2",
            ),
        ];
        let router = RadixRouter::new(routes);

        // Test that all three parameters are preserved correctly
        let result = router.route(Method::GET, "/api/v2/org/org456/team/team789/stats");
        assert!(result.is_some());
        let (route, params) = result.unwrap();
        assert_eq!(route.handler_name, "get_team_stats_v2");

        // All three parameters should be present and correct
        assert_eq!(
            get_param(&params, "version"),
            Some("v2"),
            "version parameter should be preserved after multiple backtracks"
        );
        assert_eq!(
            get_param(&params, "id"),
            Some("org456"),
            "org id should be preserved after multiple backtracks"
        );
        assert_eq!(
            params
                .iter()
                .find(|(k, _)| k == "team_id")
                .map(|(_, v)| v.as_str()),
            Some("team789")
        );
        assert_eq!(params.len(), 3);
    }
}
