use crate::spec::RouteMeta;
use http::Method;
use regex::Regex;
use std::collections::HashMap;
use tracing::{debug, info, warn};

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

/// Router that matches HTTP requests to handlers using regex patterns
///
/// Compiles OpenAPI path patterns (e.g., `/users/{id}`) into regex patterns
/// and matches incoming requests to the appropriate handler. Routes are sorted
/// by path length (longest first) to ensure most specific routes match first.
///
/// # Performance
///
/// Current implementation uses O(n) linear scanning with regex matching.
/// For v1.0, this will be replaced with a trie-based router for O(log n) lookup.
#[derive(Clone)]
pub struct Router {
    /// List of routes: (method, regex, metadata, param_names)
    routes: Vec<(Method, Regex, std::sync::Arc<RouteMeta>, Vec<String>)>,
    /// Base path prefix for all routes (e.g., `/api/v1`)
    #[allow(dead_code)]
    base_path: String,
}

impl Router {
    /// Create a new router from OpenAPI route metadata
    ///
    /// Compiles all route patterns into regex matchers and sorts them by
    /// specificity (longest paths first) for optimal matching.
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
        // We only support GET, POST, PUT, DELETE, PATCH, and OPTIONS
        let supported_methods = [
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
            Method::HEAD,
            Method::TRACE, // TRACE is included but filtered out later
        ];

        let routes: Vec<RouteMeta> = routes
            .into_iter()
            .filter(|r| supported_methods.contains(&r.method))
            .collect();
        // // Filter out routes that are not valid HTTP methods
        // let routes: Vec<RouteMeta> = routes
        //     .into_iter()
        //     .filter(|r| r.method != Method::TRACE && r.method != Method::CONNECT)
        //     .collect();

        if routes.is_empty() {
            info!(routes_count = 0, "Routing table loaded with no routes");
            return Self {
                routes: Vec::new(),
                base_path: String::new(),
            };
        }

        // RT6: Route sorting applied
        let routes_before = routes.len();
        // Ensure routes are sorted by path length (longest first) to optimize matching
        // This is useful for cases where paths may overlap, e.g. "/pets" and "/pets/{id}"
        let mut routes = routes;
        routes.sort_by_key(|r| r.path_pattern.len());
        routes.reverse();

        debug!(
            routes_before = routes_before,
            routes_after = routes.len(),
            sort_strategy = "longest_first",
            "Route sorting applied"
        );
        // Convert each route's path pattern to a regex and collect param names
        // Each route is represented as (method, compiled regex, RouteMeta, param names)
        let base_path = routes
            .first()
            .map(|r| r.base_path.clone())
            .unwrap_or_default();
        let routes: Vec<_> = routes
            .into_iter()
            .map(|route| {
                let full_path = format!("{}{}", base_path, route.path_pattern);
                let (regex, param_names) = Self::path_to_regex(&full_path);
                (route.method.clone(), regex, std::sync::Arc::new(route), param_names)
            })
            .collect();

        // RT5: Routing table loaded
        let routes_summary: Vec<String> = routes
            .iter()
            .take(10) // Limit to first 10 routes to avoid log spam
            .map(|(method, _, meta, _)| format!("{} {}{}", method, base_path, meta.path_pattern))
            .collect();

        info!(
            routes_count = routes.len(),
            base_path = %base_path,
            routes_summary = ?routes_summary,
            "Routing table loaded"
        );

        Self { routes, base_path }
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

    /// Match an HTTP request to a route
    ///
    /// Attempts to find a route that matches both the HTTP method and path pattern.
    /// Path parameters are extracted and returned in the match result.
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
            routes_count = self.routes.len(),
            "Route match attempt"
        );
        
        // Track route matching performance
        let match_start = std::time::Instant::now();
        let mut iterations = 0;

        for (m, regex, route, param_names) in &self.routes {
            iterations += 1;
            if *m != method {
                continue;
            }
            if let Some(captures) = regex.captures(path) {
                // RT2: Regex match success
                debug!(
                    pattern = %regex,
                    params_extracted = param_names.len(),
                    "Regex match success"
                );

                let mut params = HashMap::with_capacity(param_names.len());
                for (i, name) in param_names.iter().enumerate() {
                    if let Some(val) = captures.get(i + 1) {
                        params.insert(name.clone(), val.as_str().to_string());
                    }
                }

                // RT3: Route matched
                let match_duration = match_start.elapsed();
                
                // Warn if route matching is slow
                if match_duration > std::time::Duration::from_millis(1) {
                    warn!(
                        method = %method,
                        path = %path,
                        handler_name = %route.handler_name,
                        route_pattern = %route.path_pattern,
                        path_params = ?params,
                        duration_us = match_duration.as_micros(),
                        iterations = iterations,
                        "Slow route matching detected"
                    );
                } else {
                    info!(
                        method = %method,
                        path = %path,
                        handler_name = %route.handler_name,
                        route_pattern = %route.path_pattern,
                        path_params = ?params,
                        duration_us = match_duration.as_micros(),
                        iterations = iterations,
                        "Route matched"
                    );
                }

                return Some(RouteMatch {
                    route: std::sync::Arc::clone(route),
                    path_params: params,
                    handler_name: route.handler_name.clone(),
                    query_params: Default::default(),
                });
            }
        }

        // RT4: No route found (404)
        let match_duration = match_start.elapsed();
        let attempted_patterns: Vec<String> = self
            .routes
            .iter()
            .filter(|(m, _, _, _)| *m == method)
            .take(5) // Limit to avoid log spam
            .map(|(_, _, route, _)| route.path_pattern.clone())
            .collect();

        warn!(
            method = %method,
            path = %path,
            duration_us = match_duration.as_micros(),
            iterations = iterations,
            attempted_patterns = ?attempted_patterns,
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
