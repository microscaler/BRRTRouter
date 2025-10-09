use super::types::{
    ParameterLocation, ParameterMeta, ParameterStyle, ResponseSpec, Responses, RouteMeta,
};
use super::SecurityScheme;
use crate::validator::{fail_if_issues, ValidationIssue};
use oas3::spec::{MediaTypeExamples, ObjectOrReference, Parameter};
use oas3::OpenApiV3Spec;
use serde_json::Value;

/// Resolve a JSON Schema `$ref` to the actual schema definition
///
/// Looks up schema references like `#/components/schemas/User` in the OpenAPI spec
/// and returns the resolved schema object.
///
/// # Arguments
///
/// * `spec` - The OpenAPI specification
/// * `ref_path` - The `$ref` path (e.g., `#/components/schemas/Pet`)
///
/// # Returns
///
/// The resolved schema object, or `None` if the reference can't be resolved
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

/// Recursively expand all JSON Schema `$ref` references in a value
///
/// Traverses the JSON value tree and replaces any `$ref` objects with their
/// resolved schema definitions from the OpenAPI spec. Adds an `x-ref-name` field
/// to track the original reference name.
///
/// # Arguments
///
/// * `spec` - The OpenAPI specification
/// * `value` - The JSON value to process (modified in-place)
pub fn expand_schema_refs(spec: &OpenApiV3Spec, value: &mut Value) {
    match value {
        Value::Object(obj) => {
            if let Some(ref_path) = obj.get("$ref").and_then(|v| v.as_str()) {
                if let Some(schema) = resolve_schema_ref(spec, ref_path) {
                    if let Ok(mut new_val) = serde_json::to_value(schema) {
                        expand_schema_refs(spec, &mut new_val);
                        if let Some(name) = ref_path.strip_prefix("#/components/schemas/") {
                            if let Value::Object(o) = &mut new_val {
                                o.insert("x-ref-name".to_string(), Value::String(name.to_string()));
                            }
                        }
                        *value = new_val;
                        return;
                    }
                }
            }
            for v in obj.values_mut() {
                expand_schema_refs(spec, v);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                expand_schema_refs(spec, v);
            }
        }
        _ => {}
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

/// Extract the request body schema from an OpenAPI operation
///
/// Parses the `requestBody` section of an operation and extracts the JSON schema
/// for `application/json` content type. Also determines if the request body is required.
///
/// # Arguments
///
/// * `spec` - The OpenAPI specification
/// * `operation` - The operation to extract from
///
/// # Returns
///
/// A tuple of `(schema, required)` where:
/// * `schema` - The JSON schema for the request body (if present)
/// * `required` - Whether the request body is required
pub fn extract_request_schema(
    spec: &OpenApiV3Spec,
    operation: &oas3::spec::Operation,
) -> (Option<Value>, bool) {
    let mut required = false;
    let mut schema = operation.request_body.as_ref().and_then(|r| match r {
        ObjectOrReference::Object(req_body) => {
            required = req_body.required.unwrap_or(false);
            req_body.content.get("application/json").and_then(|media| {
                match media.schema.as_ref()? {
                    ObjectOrReference::Object(schema_obj) => serde_json::to_value(schema_obj).ok(),
                    ObjectOrReference::Ref { ref_path } => resolve_schema_ref(spec, ref_path)
                        .and_then(|s| serde_json::to_value(s).ok()),
                }
            })
        }
        _ => None,
    });
    if let Some(ref mut val) = schema {
        expand_schema_refs(spec, val);
    }
    (schema, required)
}

/// Extract response schemas and examples from an OpenAPI operation
///
/// Parses all response definitions from an operation and extracts schemas, examples,
/// and content types for each status code. Prioritizes 200 OK with application/json,
/// then falls back to other 2xx responses.
///
/// # Arguments
///
/// * `spec` - The OpenAPI specification
/// * `operation` - The operation to extract from
///
/// # Returns
///
/// A tuple of:
/// * Default response schema (prioritizes 200 OK application/json)
/// * Default response example
/// * Map of all responses by status code and content type
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
                        Some(MediaTypeExamples::Examples { examples }) => {
                            examples.iter().find_map(|(_, v)| match v {
                                ObjectOrReference::Object(obj) => obj.value.clone(),
                                _ => None,
                            })
                        }
                        None => None,
                    };

                    let mut schema = match media.schema.as_ref() {
                        Some(ObjectOrReference::Object(schema_obj)) => {
                            serde_json::to_value(schema_obj).ok()
                        }
                        Some(ObjectOrReference::Ref { ref_path }) => {
                            resolve_schema_ref(spec, ref_path)
                                .and_then(|s| serde_json::to_value(s).ok())
                        }
                        None => None,
                    };
                    if let Some(ref mut val) = schema {
                        expand_schema_refs(spec, val);
                    }

                    all.entry(status).or_default().insert(
                        mt.clone(),
                        ResponseSpec {
                            schema: schema.clone(),
                            example: example.clone(),
                        },
                    );

                    if status == 200 && mt == "application/json" {
                        default_schema = schema;
                        default_example = example;
                    }
                }
            }
        }
    }

    // Fallback selection if no 200 application/json found
    if default_schema.is_none() {
        // Prefer any 2xx with application/json
        let mut statuses: Vec<u16> = all.keys().cloned().collect();
        statuses.sort_unstable();
        if let Some((schema, example)) = statuses
            .iter()
            .filter(|s| **s >= 200 && **s < 300)
            .find_map(|s| all.get(s).and_then(|m| m.get("application/json")))
            .map(|spec| (spec.schema.clone(), spec.example.clone()))
        {
            default_schema = schema;
            default_example = example;
        }
    }

    if default_schema.is_none() {
        // Next, any 2xx with any media type
        let mut statuses: Vec<u16> = all.keys().cloned().collect();
        statuses.sort_unstable();
        'outer: for s in statuses.iter().filter(|s| **s >= 200 && **s < 300) {
            if let Some(mt_map) = all.get(s) {
                for (_mt, spec) in mt_map {
                    if spec.schema.is_some() || spec.example.is_some() {
                        default_schema = spec.schema.clone();
                        default_example = spec.example.clone();
                        break 'outer;
                    }
                }
            }
        }
    }

    if default_schema.is_none() {
        // Finally, any status preferring application/json
        let mut statuses: Vec<u16> = all.keys().cloned().collect();
        statuses.sort_unstable();
        if let Some((schema, example)) = statuses
            .iter()
            .find_map(|s| all.get(s).and_then(|m| m.get("application/json")))
            .map(|spec| (spec.schema.clone(), spec.example.clone()))
        {
            default_schema = schema;
            default_example = example;
        }
    }

    (default_schema, default_example, all)
}

/// Extract all security schemes from an OpenAPI specification
///
/// Parses the `components.securitySchemes` section and returns a map of scheme names
/// to their definitions (API keys, HTTP auth, OAuth2, OpenID Connect, etc.).
///
/// # Arguments
///
/// * `spec` - The OpenAPI specification
///
/// # Returns
///
/// A map of security scheme names to their definitions
pub fn extract_security_schemes(
    spec: &OpenApiV3Spec,
) -> std::collections::HashMap<String, SecurityScheme> {
    spec.components
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

/// Extract parameter metadata from an OpenAPI operation
///
/// Resolves parameter references and extracts metadata for path, query, header,
/// and cookie parameters. Each parameter includes its name, location, schema,
/// whether it's required, and serialization style.
///
/// # Arguments
///
/// * `spec` - The OpenAPI specification
/// * `params` - List of parameters (may include references)
///
/// # Returns
///
/// A vector of resolved parameter metadata
pub fn extract_parameters(
    spec: &OpenApiV3Spec,
    params: &Vec<ObjectOrReference<Parameter>>,
) -> Vec<ParameterMeta> {
    let mut out = Vec::new();
    for p in params {
        let param = match p {
            ObjectOrReference::Object(obj) => Some(obj),
            ObjectOrReference::Ref { ref_path } => resolve_parameter_ref(spec, ref_path),
        };

        if let Some(param) = param {
            let schema = param.schema.as_ref().and_then(|s| match s {
                ObjectOrReference::Object(obj) => serde_json::to_value(obj).ok(),
                ObjectOrReference::Ref { ref_path } => resolve_schema_ref(spec, ref_path)
                    .and_then(|sch| serde_json::to_value(sch).ok()),
            });

            out.push(ParameterMeta {
                name: param.name.clone(),
                location: ParameterLocation::from(param.location),
                required: param.required.is_some(),
                schema,
                style: param.style.map(ParameterStyle::from),
                explode: param.explode,
            });
        }
    }
    out
}

/// Extract the SSE flag from an OpenAPI operation
///
/// Checks for `x-sse` or `sse` extension fields to determine if the operation
/// uses Server-Sent Events for streaming responses.
///
/// # Arguments
///
/// * `operation` - The OpenAPI operation definition
///
/// # Returns
///
/// `true` if the operation uses SSE, `false` otherwise
pub fn extract_sse_flag(operation: &oas3::spec::Operation) -> bool {
    operation
        .extensions
        .get("x-sse")
        .or_else(|| operation.extensions.get("sse"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Build route metadata for all operations in an OpenAPI specification
///
/// This is the main function that processes an OpenAPI spec and extracts all the
/// metadata needed to generate handlers, validate requests, and register routes.
/// It validates the spec and reports any issues found.
///
/// # Arguments
///
/// * `spec` - The parsed OpenAPI specification
/// * `slug` - URL-safe project slug (used for generated file names)
///
/// # Returns
///
/// A vector of `RouteMeta` for all valid operations
///
/// # Errors
///
/// Returns an error if critical validation issues are found that prevent
/// code generation (e.g., missing handler names, invalid parameters).
pub fn build_routes(spec: &OpenApiV3Spec, slug: &str) -> anyhow::Result<Vec<RouteMeta>> {
    let mut routes = Vec::new();
    let mut issues = Vec::new();

    let base_path = if let Some(server) = spec.servers.first() {
        let url_str = &server.url;
        url::Url::parse(url_str)
            .or_else(|_| url::Url::parse(&format!("http://dummy{url_str}")))
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
                let location = format!("{path} → {method}");

                let handler_name = match resolve_handler_name(operation, &location, &mut issues) {
                    Some(name) => name,
                    None => continue,
                };

                let (request_schema, request_body_required) =
                    extract_request_schema(spec, operation);
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
                    request_body_required,
                    response_schema,
                    example,
                    responses,
                    security,
                    example_name: format!("{slug}_example"),
                    project_slug: slug.to_string(),
                    output_dir: std::path::PathBuf::from("examples").join(slug).join("src"),
                    base_path: base_path.clone(),
                    sse: extract_sse_flag(operation),
                });
            }
        }
    }

    fail_if_issues(issues);
    Ok(routes)
}
