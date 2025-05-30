use crate::spec::RouteMeta;
use http::Method;
use regex::Regex;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RouteMatch {
    pub route: RouteMeta,
    pub path_params: HashMap<String, String>,
    pub handler_name: String,
    pub query_params: HashMap<String, String>,
}

/// Router to match HTTP requests to handlers
/// method, compiled regex, meta, param names
#[derive(Clone)]
pub struct Router {
    routes: Vec<(Method, Regex, RouteMeta, Vec<String>)>,
    base_path: String,
}

impl Router {
    pub fn new(routes: Vec<RouteMeta>) -> Self {
        // Filter out routes that are not HTTP methods we care about
        // We only support GET, POST, PUT, DELETE, PATCH, and OPTIONS
        let supported_methods = vec![
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
