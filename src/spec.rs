use http::Method;
use oas3::OpenApiV3Spec;
use serde_json::Value;
use crate::validator::{ValidationIssue, fail_if_issues};
use oas3::spec::{ObjectOrReference, Parameter, ParameterIn};
#[allow(unused_imports)]
use std::collections::HashMap;

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

fn resolve_parameter_reference(spec: &OpenApiV3Spec, ref_path: &str) -> Option<String> {
    let ref_parts: Vec<&str> = ref_path.split('/').collect();
    let param_name = ref_parts.last()?;
    let components = spec.components.as_ref()?;
    let parameters = &components.parameters;

    match parameters.get(*param_name)? {
        ObjectOrReference::Object(param) => match param {
            Parameter { name, location: ParameterIn::Path, .. } => Some(name.clone()),
            _ => None,
        },
        ObjectOrReference::Ref { .. } => None,
    }
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

                let path_param_names: Vec<String> = path
                    .split('/')
                    .filter_map(|seg| {
                        if seg.starts_with('{') && seg.ends_with('}') {
                            Some(seg.trim_start_matches('{').trim_end_matches('}').to_string())
                        } else {
                            None
                        }
                    })
                    .collect();

                let mut declared_param_names = vec![];
                for param in &operation.parameters {
                    match param {
                        ObjectOrReference::Object(p) => {
                            if matches!(p.location, ParameterIn::Path) {
                                declared_param_names.push(p.name.clone());
                            }
                        }
                        ObjectOrReference::Ref { ref_path } => {
                            if let Some(name) = resolve_parameter_reference(spec, ref_path) {
                                declared_param_names.push(name);
                            }
                        }
                    }
                }

                for param in &path_param_names {
                    if !declared_param_names.contains(param) {
                        issues.push(ValidationIssue::new(
                            &location,
                            "MissingParameter",
                            format!("Path param '{{{}}}' not declared in parameters", param),
                        ));
                    }
                }

                if let Some(ObjectOrReference::Object(req_body)) = &operation.request_body {
                    if !req_body.content.contains_key("application/json") {
                        issues.push(ValidationIssue::new(
                            &location,
                            "InvalidRequestSchema",
                            "Missing 'application/json' requestBody content",
                        ));
                    } else {
                        let json_schema = req_body.content.get("application/json").unwrap();
                        if json_schema.schema.is_none() {
                            issues.push(ValidationIssue::new(
                                &location,
                                "InvalidRequestSchema",
                                "Missing JSON schema under requestBody.content",
                            ));
                        }
                    }
                }

                match &operation.responses {
                    Some(responses) => {
                        if let Some(ObjectOrReference::Object(resp)) = responses.get("200") {
                            if !resp.content.contains_key("application/json") {
                                issues.push(ValidationIssue::new(
                                    &location,
                                    "InvalidResponseSchema",
                                    "Missing 'application/json' in 200 response",
                                ));
                            } else {
                                let json_schema = resp.content.get("application/json").unwrap();
                                if json_schema.schema.is_none() {
                                    issues.push(ValidationIssue::new(
                                        &location,
                                        "InvalidResponseSchema",
                                        "Missing schema in 200 response's JSON content",
                                    ));
                                }
                            }
                        } else {
                            issues.push(ValidationIssue::new(
                                &location,
                                "InvalidResponseSchema",
                                "Missing 200 response definition",
                            ));
                        }
                    }
                    None => {
                        issues.push(ValidationIssue::new(
                            &location,
                            "InvalidResponseSchema",
                            "Operation has no responses defined",
                        ));
                    }
                }

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
