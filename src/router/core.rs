use crate::spec::RouteMeta;
use http::Method;
use regex::Regex;
use std::collections::HashMap;
use tracing::{debug, info, warn};

use super::radix::RadixRouter;

/// Result of successfully matching a request path to a route
///
/// Contains the matched route metadata and extracted parameters.
#[derive(Debug, Clone)]
pub struct RouteMatch {
    /// The matched route metadata from the OpenAPI spec (Arc to avoid expensive clones)
    pub route: std::sync::Arc<RouteMeta>,
    /// Path parameters extracted from the URL (e.g., `{id}` → `{"id": "123"}`)
    pub path_params: HashMap<String, String>,
    /// Name of the handler that should process this request
    pub handler_name: String,
    /// Query string parameters (populated by the server)
    pub query_params: HashMap<String, String>,
}

/// Router that matches HTTP requests to handlers using radix tree
///
/// Uses a radix tree (compact prefix tree) for O(k) route matching where k is the
/// path length. This is a significant improvement over the previous O(n) linear scan.
///
/// The router also maintains the old regex-based implementation for compatibility,
/// but the radix tree is used by default for better performance.
///
/// # Performance
///
/// - Route matching: O(k) where k is path length (not O(n) where n is number of routes)
/// - Memory efficient: Shared prefixes are stored only once
/// - Minimal allocations: Uses Arc and Cow to avoid unnecessary cloning
#[derive(Clone)]
pub struct Router {
    /// Radix tree for fast O(k) route lookup
    radix_router: RadixRouter,
    /// Legacy regex-based routes for fallback (kept for compatibility)
    routes: Vec<(Method, Regex, std::sync::Arc<RouteMeta>, Vec<String>)>,
    /// Base path prefix for all routes (e.g., `/api/v1`)
    #[allow(dead_code)]
    base_path: String,
}

impl Router {
    /// Create a new router from OpenAPI route metadata
    ///
    /// Builds both a radix tree for O(k) lookups and keeps legacy regex-based
    /// routing for compatibility. The radix tree is used by default.
    ///
    /// # Arguments
    ///
    /// * `routes` - List of route metadata extracted from OpenAPI spec
    ///
    /// # Returns
    ///
    /// A new `Router` ready to match incoming requests
    pub fn new(routes: Vec<RouteMeta>) -> Self {
        // Filter out routes that are not HTTP methods we care about
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

        if routes.is_empty() {
            info!(routes_count = 0, "Routing table loaded with no routes");
            return Self {
                radix_router: RadixRouter::new(Vec::new()),
                routes: Vec::new(),
                base_path: String::new(),
            };
        }

        let base_path = routes
            .first()
            .map(|r| r.base_path.clone())
            .unwrap_or_default();

        // Create the radix tree router for fast O(k) lookups
        let radix_router = RadixRouter::new(routes.clone());

        // Also build the legacy regex-based routes for compatibility
        // (though we'll primarily use the radix tree)
        let routes: Vec<_> = routes
            .into_iter()
            .map(|route| {
                let full_path = format!("{}{}", base_path, route.path_pattern);
                let (regex, param_names) = Self::path_to_regex(&full_path);
                let method = route.method.clone();
                (method, regex, std::sync::Arc::new(route), param_names)
            })
            .collect();

        // RT5: Routing table loaded
        let routes_summary: Vec<String> = routes
            .iter()
            .take(10)
            .map(|(method, _, meta, _)| format!("{} {}{}", method, base_path, meta.path_pattern))
            .collect();

        info!(
            routes_count = routes.len(),
            base_path = %base_path,
            routes_summary = ?routes_summary,
            routing_algorithm = "radix_tree",
            "Routing table loaded with O(k) radix tree"
        );

        Self {
            radix_router,
            routes,
            base_path,
        }
    }

    /// Print all registered routes to stdout
    ///
    /// Useful for debugging and verifying that routes are loaded correctly.
    pub fn dump_routes(&self) {
        println!(
            "[routes] base_path={} count={}",
            self.base_path,
            self.routes.len()
        );
        for (method, _re, meta, _params) in &self.routes {
            println!(
                "[route] {method} {} -> {}",
                format_args!("{}{}", self.base_path, meta.path_pattern),
                meta.handler_name
            );
        }
    }

    /// Match an HTTP request to a route using radix tree
    ///
    /// Uses the radix tree for O(k) route matching where k is the path length.
    /// This is significantly faster than the previous O(n) linear scan.
    ///
    /// # Arguments
    ///
    /// * `method` - HTTP method (GET, POST, etc.)
    /// * `path` - Request path (e.g., `/users/123`)
    ///
    /// # Returns
    ///
    /// * `Some(RouteMatch)` - If a matching route is found
    /// * `None` - If no route matches (results in 404)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use http::Method;
    ///
    /// if let Some(m) = router.route(Method::GET, "/users/123") {
    ///     println!("Handler: {}", m.handler_name);
    ///     println!("User ID: {}", m.path_params["id"]);
    /// }
    /// ```
    pub fn route(&self, method: Method, path: &str) -> Option<RouteMatch> {
        // RT1: Route match attempt
        debug!(
            method = %method,
            path = %path,
            algorithm = "radix_tree",
            "Route match attempt"
        );
        
        // Track route matching performance
        let match_start = std::time::Instant::now();

        // Use radix tree for O(k) lookup
        let result = self.radix_router.route(method.clone(), path);
        
        let match_duration = match_start.elapsed();

        if let Some((route, params)) = result {
            // RT3: Route matched
            let handler_name = route.handler_name.clone();
            
            if match_duration > std::time::Duration::from_millis(1) {
                warn!(
                    method = %method,
                    path = %path,
                    handler_name = %handler_name,
                    route_pattern = %route.path_pattern,
                    path_params = ?params,
                    duration_us = match_duration.as_micros(),
                    algorithm = "radix_tree",
                    "Slow route matching detected"
                );
            } else {
                info!(
                    method = %method,
                    path = %path,
                    handler_name = %handler_name,
                    route_pattern = %route.path_pattern,
                    path_params = ?params,
                    duration_us = match_duration.as_micros(),
                    algorithm = "radix_tree",
                    "Route matched"
                );
            }

            return Some(RouteMatch {
                route,
                path_params: params,
                handler_name,
                query_params: Default::default(),
            });
        }

        // RT4: No route found (404)
        warn!(
            method = %method,
            path = %path,
            duration_us = match_duration.as_micros(),
            algorithm = "radix_tree",
            "No route matched"
        );

        None
    }
    /// Convert an OpenAPI path pattern to a regex and extract parameter names
    ///
    /// Transforms path patterns like `/users/{id}` into regex patterns like
    /// `^/users/([^/]+)$` and extracts parameter names `["id"]`.
    ///
    /// # Arguments
    ///
    /// * `path` - OpenAPI path pattern (e.g., `/users/{id}/posts/{postId}`)
    ///
    /// # Returns
    ///
    /// A tuple of:
    /// * `Regex` - Compiled regex for matching paths
    /// * `Vec<String>` - Ordered list of parameter names
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let (regex, params) = Router::path_to_regex("/users/{id}");
    /// assert_eq!(params, vec!["id"]);
    /// assert!(regex.is_match("/users/123"));
    /// ```
    pub(crate) fn path_to_regex(path: &str) -> (Regex, Vec<String>) {
        if path == "/" {
            return (
                Regex::new(r"^/$").expect("Failed to compile path regex"),
                Vec::new(),
            );
        }

        // Reserve space for the final regex string and parameter list
        let mut pattern = String::with_capacity(path.len() + 5);
        pattern.push('^');
        let mut param_names = Vec::with_capacity(path.matches('{').count());

        for segment in path.split('/') {
            if segment.starts_with('{') && segment.ends_with('}') {
                let param_name = segment
                    .trim_start_matches('{')
                    .trim_end_matches('}')
                    .to_string();
                pattern.push_str("/([^/]+)");
                param_names.push(param_name);
            } else if !segment.is_empty() {
                pattern.push('/');
                pattern.push_str(segment);
            }
        }

        pattern.push('$');
        let regex = Regex::new(&pattern).expect("Failed to compile path regex");

        (regex, param_names)
    }
}
