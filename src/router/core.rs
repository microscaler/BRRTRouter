use crate::spec::RouteMeta;
use http::Method;
use regex::Regex;
use std::collections::HashMap;

/// Result of successfully matching a request path to a route
///
/// Contains the matched route metadata and extracted parameters.
#[derive(Debug, Clone)]
pub struct RouteMatch {
    /// The matched route metadata from the OpenAPI spec
    pub route: RouteMeta,
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
    routes: Vec<(Method, Regex, RouteMeta, Vec<String>)>,
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
            return Self {
                routes: Vec::new(),
                base_path: String::new(),
            };
        }
        // Ensure routes are sorted by path length (longest first) to optimize matching
        // This is useful for cases where paths may overlap, e.g. "/pets" and "/pets/{id}"
        let mut routes = routes;
        routes.sort_by_key(|r| r.path_pattern.len());
        routes.reverse();
        // Convert each route's path pattern to a regex and collect param names
        // Each route is represented as (method, compiled regex, RouteMeta, param names)
        let base_path = routes
            .first()
            .map(|r| r.base_path.clone())
            .unwrap_or_default();
        let routes = routes
            .into_iter()
            .map(|route| {
                let full_path = format!("{}{}", base_path, route.path_pattern);
                let (regex, param_names) = Self::path_to_regex(&full_path);
                (route.method.clone(), regex, route, param_names)
            })
            .collect();

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
                "[route] {} {} -> {}",
                method,
                format!("{}{}", self.base_path, meta.path_pattern),
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
        for (m, regex, route, param_names) in &self.routes {
            if *m != method {
                continue;
            }
            if let Some(captures) = regex.captures(path) {
                let mut params = HashMap::with_capacity(param_names.len());
                for (i, name) in param_names.iter().enumerate() {
                    if let Some(val) = captures.get(i + 1) {
                        params.insert(name.clone(), val.as_str().to_string());
                    }
                }
                return Some(RouteMatch {
                    route: route.clone(),
                    path_params: params,
                    handler_name: route.handler_name.clone(),
                    query_params: Default::default(),
                });
            }
        }
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
