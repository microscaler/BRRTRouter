//! Router core module - hot path for request routing.
//!
//! # JSF Compliance (Rule 206)
//!
//! This module is part of the request hot path. The following clippy lints
//! are denied to enforce "no heap allocations after initialization":
//!
//! - `clippy::disallowed_methods` - Blocks String/Vec allocation methods
//! - `clippy::inefficient_to_string` - Catches unnecessary allocations
//! - `clippy::format_push_string` - Prevents format! string building

// JSF Rule 206: Deny heap allocations in the hot path
#![deny(clippy::inefficient_to_string)]
#![deny(clippy::format_push_string)]
#![deny(clippy::unnecessary_to_owned)]

use crate::spec::RouteMeta;
use http::Method;
use regex::Regex;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::radix::RadixRouter;

/// Maximum number of path/query parameters before heap allocation.
/// Most REST APIs have ≤4 path params (e.g., /users/{id}/posts/{postId}).
/// JSF Rule: No heap allocations in the hot path for common cases.
pub const MAX_INLINE_PARAMS: usize = 8;

/// Stack-allocated parameter storage for the hot path.
/// Uses SmallVec to avoid heap allocation for routes with ≤8 params.
/// 
/// # JSF Optimization (P0)
/// 
/// Param names use `Arc<str>` instead of `String` because:
/// - Names come from the static route tree (known at startup)
/// - `Arc::clone()` is O(1) atomic increment vs O(n) string copy
/// - Values remain `String` as they're per-request data from the URL
pub type ParamVec = SmallVec<[(Arc<str>, String); MAX_INLINE_PARAMS]>;

/// Result of successfully matching a request path to a route
///
/// Contains the matched route metadata and extracted parameters.
///
/// # JSF Compliance
///
/// Uses `SmallVec` instead of `HashMap` for path/query parameters to avoid
/// heap allocation in the common case (≤8 params). This follows JSF Rule 206:
/// "No heap allocations after initialization" for the hot path.
#[derive(Debug, Clone)]
pub struct RouteMatch {
    /// The matched route metadata from the OpenAPI spec (Arc to avoid expensive clones)
    pub route: std::sync::Arc<RouteMeta>,
    /// Path parameters extracted from the URL (e.g., `{id}` → `{"id": "123"}`)
    /// Stack-allocated for ≤8 params (JSF: no heap in hot path)
    pub path_params: ParamVec,
    /// Name of the handler that should process this request
    pub handler_name: String,
    /// Query string parameters (populated by the server)
    /// Stack-allocated for ≤8 params (JSF: no heap in hot path)
    pub query_params: ParamVec,
}

impl RouteMatch {
    /// Get a path parameter by name
    ///
    /// Uses "last write wins" semantics: if duplicate parameter names exist
    /// at different path depths (e.g., `/org/{id}/team/{team_id}/user/{id}`),
    /// returns the last occurrence (the user id, not the org id).
    ///
    /// # Arguments
    /// * `name` - The parameter name (e.g., "id")
    ///
    /// # Returns
    /// The parameter value if found, None otherwise
    #[inline]
    #[must_use]
    pub fn get_path_param(&self, name: &str) -> Option<&str> {
        self.path_params
            .iter()
            .rfind(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }

    /// Get a query parameter by name
    ///
    /// Uses "last write wins" semantics: if duplicate query parameter names exist
    /// (e.g., `?limit=10&limit=20`), returns the last occurrence.
    ///
    /// # Arguments
    /// * `name` - The parameter name
    ///
    /// # Returns
    /// The parameter value if found, None otherwise
    #[inline]
    #[must_use]
    pub fn get_query_param(&self, name: &str) -> Option<&str> {
        self.query_params
            .iter()
            .rfind(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }

    /// Convert path_params to HashMap for compatibility with existing code
    /// Note: This allocates - use get_path_param() in hot paths instead
    #[must_use]
    pub fn path_params_map(&self) -> HashMap<String, String> {
        self.path_params
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    /// Convert query_params to HashMap for compatibility with existing code
    /// Note: This allocates - use get_query_param() in hot paths instead
    #[must_use]
    pub fn query_params_map(&self) -> HashMap<String, String> {
        self.query_params
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }
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
    #[must_use]
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
    #[must_use]
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
            // JSF P0-2: Convert Arc<str> to String for RouteMatch
            let handler_name = route.handler_name.to_string();

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
    /// Get all registered path patterns for metrics pre-registration
    ///
    /// Returns a list of all path patterns (with base path prepended) that are
    /// registered in this router. This is useful for pre-registering paths in
    /// the metrics middleware at startup to avoid runtime allocation.
    ///
    /// # Returns
    ///
    /// A vector of path patterns (e.g., `["/api/users", "/api/posts/{id}"]`)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let router = Router::new(routes);
    /// let paths = router.get_all_path_patterns();
    /// metrics.pre_register_paths(&paths);
    /// ```
    #[must_use]
    pub fn get_all_path_patterns(&self) -> Vec<String> {
        self.routes
            .iter()
            .map(|(_method, _regex, meta, _params)| {
                format!("{}{}", self.base_path, meta.path_pattern)
            })
            .collect()
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
