use http::Method;
use oas3::OpenApiV3Spec;
use serde_json::Value;

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

    if let Some(paths_map) = spec.paths.as_ref() {
        for (path, item) in paths_map {
            for (method_str, operation) in item.methods() {
                if verbose {
                    println!("Inspecting operation: {} {}", method_str, path);
                    println!("  Extensions: {:?}", operation.extensions);
                }

                let method = method_str.clone();

                let handler_name: String = operation
                    .extensions
                    .iter()
                    .find_map(|(key, val)| {
                        if key.starts_with("handler") {
                            if verbose {
                                println!("  → Found handler extension: {} = {}", key, val);
                                println!(
                                    "Extensions on {} {}: {:?}",
                                    method, path, operation.extensions
                                );
                            }
                            match val {
                                serde_json::Value::String(s) => Some(s.clone()),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| {
                        let fallback = format!("{}_{}", method_str, path.replace("/", "_"));
                        if verbose {
                            println!("  → No handler extension, using fallback: {}", fallback);
                        }
                        fallback
                    });

                if verbose {
                    println!("  → Final route: {} {} -> {}", method, path, handler_name);
                }

                routes.push(RouteMeta {
                    method,
                    path_pattern: path.clone(),
                    handler_name,
                    parameters: vec![], // fill in as before
                    request_schema: None,
                    response_schema: None,
                });
            }
        }
    }

    Ok(routes)
}
