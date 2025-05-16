use http::Method;
use oas3::OpenApiV3Spec;
use serde_json::Value;
use crate::validator::{ValidationIssue, fail_if_issues};

#[derive(Debug, Clone)]
pub struct RouteMeta {
    pub method: Method,
    pub path_pattern: String,
    pub handler_name: String,
    pub parameters: Vec<ParameterMeta>,
    pub request_schema: Option<Value>,
    pub response_schema: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct ParameterMeta {
    pub name: String,
    pub location: String,
    pub required: bool,
    pub schema: Option<Value>,
}

pub fn load_spec(file_path: &str, verbose: bool) -> anyhow::Result<Vec<RouteMeta>> {
    let content = std::fs::read_to_string(file_path)?;
    let spec: OpenApiV3Spec = if file_path.ends_with(".yaml") || file_path.ends_with(".yml") {
        serde_yaml::from_str(&content)?
    } else {
        serde_json::from_str(&content)?
    };

    build_routes(&spec, verbose)
}

pub fn load_spec_from_spec(spec: OpenApiV3Spec, verbose: bool) -> anyhow::Result<Vec<RouteMeta>> {
    build_routes(&spec, verbose)
}

fn build_routes(spec: &OpenApiV3Spec, verbose: bool) -> anyhow::Result<Vec<RouteMeta>> {
    let mut routes = Vec::new();
    let mut issues = Vec::new();

    if let Some(paths_map) = spec.paths.as_ref() {
        for (path, item) in paths_map {
            for (method_str, operation) in item.methods() {
                if verbose {
                    println!("Inspecting operation: {} {}", method_str, path);
                    println!("  Extensions: {:?}", operation.extensions);
                }

                let method = method_str.clone();
                let location = format!("{} → {}", path, method);

                let handler_name = operation
                    .extensions
                    .iter()
                    .find_map(|(key, val)| {
                        if key.starts_with("handler") {
                            if verbose {
                                println!("  → Found handler extension: {} = {}", key, val);
                                println!("Extensions on {} {}: {:?}", method, path, operation.extensions);
                            }
                            match val {
                                serde_json::Value::String(s) => Some(s.clone()),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    });

                let handler_name = match handler_name {
                    Some(name) => name,
                    None => {
                        issues.push(ValidationIssue::new(
                            &location,
                            "MissingHandler",
                            "Missing x-handler-* extension",
                        ));
                        continue;
                    }
                };

                if verbose {
                    println!("  → Final route: {} {} -> {}", method, path, handler_name);
                }

                routes.push(RouteMeta {
                    method,
                    path_pattern: path.clone(),
                    handler_name,
                    parameters: vec![],
                    request_schema: None,
                    response_schema: None,
                });
            }
        }
    }

    fail_if_issues(issues);
    Ok(routes)
}
