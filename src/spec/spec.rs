use crate::validator::{fail_if_issues, ValidationIssue};
use http::Method;
pub use oas3::spec::{SecurityRequirement, SecurityScheme};
use oas3::spec::{
    MediaTypeExamples, ObjectOrReference, Parameter, ParameterIn as OasParameterLocation,
};
use oas3::OpenApiV3Spec;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterLocation {
    Path,
    Query,
    Header,
    Cookie,
}

impl std::fmt::Display for ParameterLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParameterLocation::Path => write!(f, "Path"),
            ParameterLocation::Query => write!(f, "Query"),
            ParameterLocation::Header => write!(f, "Header"),
            ParameterLocation::Cookie => write!(f, "Cookie"),
        }
    }
}

impl From<OasParameterLocation> for ParameterLocation {
    fn from(loc: OasParameterLocation) -> Self {
        match loc {
            OasParameterLocation::Path => ParameterLocation::Path,
            OasParameterLocation::Query => ParameterLocation::Query,
            OasParameterLocation::Header => ParameterLocation::Header,
            OasParameterLocation::Cookie => ParameterLocation::Cookie,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RouteMeta {
    pub method: Method,
    pub path_pattern: String,
    pub handler_name: String,
    pub parameters: Vec<ParameterMeta>,
    pub request_schema: Option<Value>,
    pub response_schema: Option<Value>,
    pub example: Option<Value>,
    pub responses: Responses,
    pub security: Vec<SecurityRequirement>,
    pub example_name: String,
    pub project_slug: String,
    pub output_dir: PathBuf,
    pub base_path: String,
}

#[derive(Debug, Clone)]
pub struct ParameterMeta {
    pub name: String,
    pub location: ParameterLocation,
    pub required: bool,
    pub schema: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResponseSpec {
    pub schema: Option<Value>,
    pub example: Option<Value>,
}

pub type Responses = std::collections::HashMap<u16, std::collections::HashMap<String, ResponseSpec>>;

pub fn load_spec(file_path: &str) -> anyhow::Result<(Vec<RouteMeta>, String)> {
    let content = std::fs::read_to_string(file_path)?;
    let spec: OpenApiV3Spec = if file_path.ends_with(".yaml") || file_path.ends_with(".yml") {
        serde_yaml::from_str(&content)?
    } else {
        serde_json::from_str(&content)?
    };

    let title = spec
        .info
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        .trim_matches('_')
        .to_string();

    let routes = build_routes(&spec, &title)?;
    Ok((routes, title))
}

pub fn load_spec_full(
    file_path: &str,
) -> anyhow::Result<(Vec<RouteMeta>, std::collections::HashMap<String, SecurityScheme>, String)> {
    let content = std::fs::read_to_string(file_path)?;
    let spec: OpenApiV3Spec = if file_path.ends_with(".yaml") || file_path.ends_with(".yml") {
        serde_yaml::from_str(&content)?
    } else {
        serde_json::from_str(&content)?
    };

    let title = spec
        .info
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        .trim_matches('_')
        .to_string();

    let routes = build_routes(&spec, &title)?;
    let schemes = extract_security_schemes(&spec);
    Ok((routes, schemes, title))
}

/// Build route metadata from an already parsed [`OpenApiV3Spec`].
pub fn load_spec_from_spec(spec: OpenApiV3Spec) -> anyhow::Result<Vec<RouteMeta>> {
    let slug = spec
        .info
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        .trim_matches('_')
        .to_string();

    let routes = build_routes(&spec, &slug)?;
    Ok(routes)
}

pub fn load_spec_from_spec_full(
    spec: OpenApiV3Spec,
) -> anyhow::Result<(Vec<RouteMeta>, std::collections::HashMap<String, SecurityScheme>)> {
    let slug = spec
        .info
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        .trim_matches('_')
        .to_string();
    let routes = build_routes(&spec, &slug)?;
    let schemes = extract_security_schemes(&spec);
    Ok((routes, schemes))
}

pub fn resolve_schema_ref<'a>(
    spec: &'a OpenApiV3Spec,
    ref_path: &str,
) -> Option<&'a oas3::spec::ObjectSchema> {
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
fn resolve_handler_name(
    operation: &oas3::spec::Operation,
    location: &str,
    issues: &mut Vec<ValidationIssue>,
) -> Option<String> {
    operation
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
        .or_else(|| operation.operation_id.clone())
        .or_else(|| {
            issues.push(ValidationIssue::new(
                location,
                "MissingHandler",
                "Missing operationId or x-handler-* extension",
            ));
            None
        })
}

pub fn extract_request_schema(
    spec: &OpenApiV3Spec,
    operation: &oas3::spec::Operation,
) -> Option<Value> {
    operation.request_body.as_ref().and_then(|r| match r {
        ObjectOrReference::Object(req_body) => {
            req_body.content.get("application/json").and_then(|media| {
                match media.schema.as_ref()? {
                    ObjectOrReference::Object(schema_obj) => serde_json::to_value(schema_obj).ok(),
                    ObjectOrReference::Ref { ref_path } => resolve_schema_ref(spec, ref_path)
                        .and_then(|s| serde_json::to_value(s).ok()),
                }
            })
        }
        _ => None,
    })
}

pub fn extract_response_schema_and_example(
    spec: &OpenApiV3Spec,
    operation: &oas3::spec::Operation,
) -> (Option<Value>, Option<Value>, Responses) {
    let mut all: Responses = std::collections::HashMap::new();
    let mut default_schema = None;
    let mut default_example = None;

    if let Some(responses_map) = operation.responses.as_ref() {
        for (status_str, resp_ref) in responses_map {
            let status: u16 = match status_str.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let ObjectOrReference::Object(resp_obj) = resp_ref {
                for (mt, media) in &resp_obj.content {
                    let example = match &media.examples {
                        Some(MediaTypeExamples::Example { example }) => Some(example.clone()),
                        Some(MediaTypeExamples::Examples { examples }) => examples
                            .iter()
                            .find_map(|(_, v)| match v {
                                ObjectOrReference::Object(obj) => obj.value.clone(),
                                _ => None,
                            }),
                        None => None,
                    };

                    let schema = match media.schema.as_ref() {
                        Some(ObjectOrReference::Object(schema_obj)) => {
                            serde_json::to_value(schema_obj).ok()
                        }
                        Some(ObjectOrReference::Ref { ref_path }) => {
                            resolve_schema_ref(spec, ref_path)
                                .and_then(|s| serde_json::to_value(s).ok())
                        }
                        None => None,
                    };

                    all.entry(status)
                        .or_insert_with(std::collections::HashMap::new)
                        .insert(mt.clone(), ResponseSpec { schema: schema.clone(), example: example.clone() });

                    if status == 200 && mt == "application/json" {
                        default_schema = schema;
                        default_example = example;
                    }
                }
            }
        }
    }

    (default_schema, default_example, all)
}

pub fn extract_security_schemes(
    spec: &OpenApiV3Spec,
) -> std::collections::HashMap<String, SecurityScheme> {
    spec
        .components
        .as_ref()
        .map(|c| {
            c.security_schemes
                .iter()
                .filter_map(|(name, scheme)| match scheme {
                    ObjectOrReference::Object(obj) => Some((name.clone(), obj.clone())),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn resolve_parameter_ref<'a>(
    spec: &'a OpenApiV3Spec,
    ref_path: &str,
) -> Option<&'a oas3::spec::Parameter> {
    if let Some(name) = ref_path.strip_prefix("#/components/parameters/") {
        spec.components
            .as_ref()?
            .parameters
            .get(name)
            .and_then(|param_ref| match param_ref {
                ObjectOrReference::Object(param) => Some(param),
                _ => None,
            })
    } else {
        None
    }
}

pub fn extract_parameters(
    spec: &OpenApiV3Spec,
    params: &Vec<ObjectOrReference<Parameter>>,
) -> Vec<ParameterMeta> {
    let mut out = Vec::new();
    for p in params {
        let param = match p {
            ObjectOrReference::Object(obj) => Some(obj),
            ObjectOrReference::Ref { ref_path } => resolve_parameter_ref(spec, &ref_path),
        };

        if let Some(param) = param {
            let schema = param.schema.as_ref().and_then(|s| match s {
                ObjectOrReference::Object(obj) => serde_json::to_value(obj).ok(),
                ObjectOrReference::Ref { ref_path } => resolve_schema_ref(spec, ref_path)
                    .and_then(|sch| serde_json::to_value(sch).ok()),
            });

            out.push(ParameterMeta {
                name: param.name.clone(),
                location: ParameterLocation::from(param.location.clone()),
                required: param.required.is_some(),
                schema,
            });
        }
    }
    out
}

pub fn build_routes(spec: &OpenApiV3Spec, slug: &str) -> anyhow::Result<Vec<RouteMeta>> {
    let mut routes = Vec::new();
    let mut issues = Vec::new();

    let base_path = if let Some(server) = spec.servers.get(0) {
        let url_str = &server.url;
        url::Url::parse(url_str)
            .or_else(|_| url::Url::parse(&format!("http://dummy{}", url_str)))
            .map(|u| {
                let p = u.path().trim_end_matches('/');
                if p == "/" || p.is_empty() {
                    String::new()
                } else {
                    p.to_string()
                }
            })
            .unwrap_or_default()
    } else {
        String::new()
    };

    if let Some(paths_map) = spec.paths.as_ref() {
        for (path, item) in paths_map {
            for (method_str, operation) in item.methods() {
                let method = method_str.clone();
                let location = format!("{} → {}", path, method);

                let handler_name = match resolve_handler_name(operation, &location, &mut issues) {
                    Some(name) => name,
                    None => continue,
                };

                let request_schema = extract_request_schema(spec, operation);
                let (response_schema, example, responses) =
                    extract_response_schema_and_example(spec, operation);

                let security = if !operation.security.is_empty() {
                    operation.security.clone()
                } else {
                    spec.security.clone()
                };

                let mut parameters = Vec::new();
                parameters.extend(extract_parameters(spec, &item.parameters));
                parameters.extend(extract_parameters(spec, &operation.parameters));

                routes.push(RouteMeta {
                    method,
                    path_pattern: path.clone(),
                    handler_name,
                    parameters,
                    request_schema,
                    response_schema,
                    example,
                    responses,
                    security,
                    example_name: format!("{}_example", slug),
                    project_slug: slug.to_string(),
                    output_dir: PathBuf::from("examples").join(slug).join("src"),
                    base_path: base_path.clone(),
                });
            }
        }
    }

    fail_if_issues(issues);
    Ok(routes)
}
