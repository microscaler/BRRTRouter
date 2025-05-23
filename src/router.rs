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
}

impl Router {
    pub fn new(routes: Vec<RouteMeta>) -> Self {
        let routes = routes
            .into_iter()
            .map(|route| {
                let (regex, param_names) = Self::path_to_regex(&route.path_pattern);
                (route.method.clone(), regex, route, param_names)
            })
            .collect();

        Self { routes }
    }

    pub fn route(&self, method: Method, path: &str) -> Option<RouteMatch> {
        for (m, regex, route, param_names) in &self.routes {
            if *m != method {
                continue;
            }
            if let Some(captures) = regex.captures(path) {
                let mut params = HashMap::new();
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

    fn path_to_regex(path: &str) -> (Regex, Vec<String>) {
        let mut pattern = String::from("^");
        let mut param_names = Vec::new();

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
