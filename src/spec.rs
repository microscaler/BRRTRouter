use crate::validator::{fail_if_issues, ValidationIssue};
use http::Method;
use oas3::spec::{ObjectOrReference, Parameter, Schema};
use oas3::{OpenApiV3Spec, Spec};
use serde_json::Value;
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

fn resolve_schema_ref<'a>(spec: &'a Spec, ref_path: &str) -> Option<&'a oas3::spec::ObjectSchema> {
    if let Some(name) = ref_path.strip_prefix("#/components/schemas/") {
        spec.components
            .as_ref()?
            .schemas
            .get(name)
            .and_then(|schema_ref| match schema_ref {
                ObjectOrReference::Object(schema) => Some(schema),
                _ => None,
            })
    } else {
        None
    }
}

pub(crate) fn build_routes(spec: &OpenApiV3Spec, verbose: bool) -> anyhow::Result<Vec<RouteMeta>> {
    let mut routes = Vec::new();
    let mut issues = Vec::new();

    if let Some(paths_map) = spec.paths.as_ref() {
        for (path, item) in paths_map {
            for (method_str, operation) in item.methods() {
                let method = method_str.clone();
                let location = format!("{} → {}", path, method);

                let handler_name = operation
                    .extensions
                    .iter()
                    .find_map(|(key, val)| {
                        if key.starts_with("x-handler") {
                            if let Value::String(s) = val {
                                return Some(s.clone());
                            }
                        }
                        None
                    })
                    .or_else(|| operation.operation_id.clone());

                let handler_name = match handler_name {
                    Some(name) => name,
                    None => {
                        issues.push(ValidationIssue::new(
                            &location,
                            "MissingHandler",
                            "Missing operationId or x-handler-* extension",
                        ));
                        continue;
                    }
                };

                let request_schema = operation.request_body.as_ref().and_then(|r| match r {
                    ObjectOrReference::Object(req_body) => {
                        if !req_body.content.contains_key("application/json") {
                            issues.push(ValidationIssue::new(
                                &location,
                                "InvalidRequestSchema",
                                "Missing 'application/json' requestBody content",
                            ));
                            return None;
                        }
                        let media = req_body.content.get("application/json").unwrap();
                        match media.schema.as_ref()? {
                            ObjectOrReference::Object(schema_obj) => {
                                let val = serde_json::to_value(schema_obj).ok();
                                if let Some(v) = &val {
                                    if v.get("type").is_none() {
                                        issues.push(ValidationIssue::new(
                                            &location,
                                            "InvalidRequestSchema",
                                            "Request schema is missing 'type' field",
                                        ));
                                    }
                                }
                                val
                            }
                            ObjectOrReference::Ref { ref_path } => {
                                resolve_schema_ref(spec, ref_path)
                                    .and_then(|schema| serde_json::to_value(schema).ok())
                            }
                        }
                    }
                    _ => None,
                });

                let response_schema = operation.responses.as_ref().and_then(|responses_map| {
                    let resp = responses_map.get("200")?;
                    match resp {
                        ObjectOrReference::Object(resp_obj) => {
                            let media = resp_obj.content.get("application/json")?;
                            match media.schema.as_ref()? {
                                ObjectOrReference::Object(schema_obj) => {
                                    let val = serde_json::to_value(schema_obj).ok();
                                    if let Some(v) = &val {
                                        if v.get("type").is_none() {
                                            issues.push(ValidationIssue::new(
                                                &location,
                                                "InvalidResponseSchema",
                                                "Response schema is missing 'type' field",
                                            ));
                                        }
                                    }
                                    val
                                }
                                ObjectOrReference::Ref { ref_path } => {
                                    resolve_schema_ref(spec, ref_path)
                                        .and_then(|schema| serde_json::to_value(schema).ok())
                                }
                            }
                        }
                        _ => None,
                    }
                });

                routes.push(RouteMeta {
                    method,
                    path_pattern: path.clone(),
                    handler_name,
                    parameters: vec![],
                    request_schema,
                    response_schema,
                });
            }
        }
    }

    fail_if_issues(issues);
    Ok(routes)
}
