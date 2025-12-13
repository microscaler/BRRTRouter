use super::types::{
    ParameterLocation, ParameterMeta, ParameterStyle, ResponseSpec, Responses, RouteMeta,
};
use super::SecurityScheme;
use crate::validator::{fail_if_issues, ValidationIssue};
use oas3::spec::{MediaTypeExamples, ObjectOrReference, Parameter};
use oas3::OpenApiV3Spec;
use serde_json::Value;
use std::cmp;
use std::sync::Arc;

/// Maximum estimated size for unbounded types (arrays/strings without maxItems/maxLength)
const DEFAULT_MAX_ARRAY_ITEMS: usize = 100;
const DEFAULT_MAX_STRING_LENGTH: usize = 1024;
const DEFAULT_OBJECT_PROPERTY_SIZE: usize = 50;
const MAX_ESTIMATED_BODY_SIZE: usize = 10 * 1024 * 1024; // 10MB cap

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

/// Estimate the maximum size in bytes of a JSON body based on OpenAPI schema
///
/// This provides a conservative estimate by analyzing schema constraints:
/// - String: uses `maxLength` or defaults to DEFAULT_MAX_STRING_LENGTH
/// - Array: uses `maxItems * item_size` or defaults to DEFAULT_MAX_ARRAY_ITEMS
/// - Object: sums property sizes, clamped to reasonable bounds
/// - Number/Integer: assumes 20 bytes for JSON representation
/// - Boolean: assumes 5 bytes ("true" or "false")
///
/// The function also checks for vendor extension `x-brrtrouter-body-size-bytes`
/// which allows explicit size overrides in the OpenAPI spec.
///
/// # Arguments
///
/// * `schema` - The JSON Schema to analyze
///
/// # Returns
///
/// Estimated maximum body size in bytes, or None if schema is absent
pub fn estimate_body_size(schema: Option<&Value>) -> Option<usize> {
    fn estimate_schema_size(schema: &Value, depth: usize) -> usize {
        // Prevent infinite recursion
        if depth > 10 {
            return DEFAULT_OBJECT_PROPERTY_SIZE;
        }

        let obj = match schema.as_object() {
            Some(o) => o,
            None => return DEFAULT_OBJECT_PROPERTY_SIZE,
        };

        // Check for vendor extension override
        if let Some(override_size) = obj
            .get("x-brrtrouter-body-size-bytes")
            .and_then(|v| v.as_u64())
        {
            return cmp::min(override_size as usize, MAX_ESTIMATED_BODY_SIZE);
        }

        let type_str = obj.get("type").and_then(|v| v.as_str());

        match type_str {
            Some("string") => {
                let max_len =
                    obj.get("maxLength")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(DEFAULT_MAX_STRING_LENGTH as u64) as usize;
                // Account for JSON quotes and potential escaping
                cmp::min(max_len * 2 + 2, MAX_ESTIMATED_BODY_SIZE)
            }
            Some("integer") | Some("number") => {
                // JSON numbers can be large, but typically around 20 bytes
                20
            }
            Some("boolean") => {
                // "true" or "false"
                5
            }
            Some("array") => {
                let max_items =
                    obj.get("maxItems")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(DEFAULT_MAX_ARRAY_ITEMS as u64) as usize;
                let item_size = obj
                    .get("items")
                    .map(|items| estimate_schema_size(items, depth + 1))
                    .unwrap_or(DEFAULT_OBJECT_PROPERTY_SIZE);
                // Array overhead: brackets, commas
                let overhead = 2 + max_items.saturating_sub(1);
                cmp::min(
                    max_items.saturating_mul(item_size).saturating_add(overhead),
                    MAX_ESTIMATED_BODY_SIZE,
                )
            }
            Some("object") | None => {
                // For objects, sum up property sizes
                let properties = obj.get("properties").and_then(|v| v.as_object());
                let additional_props = obj.get("additionalProperties");

                let mut total: usize = 2; // {} brackets

                if let Some(props) = properties {
                    for (key, prop_schema) in props {
                        // Key length + quotes + colon + space
                        let key_overhead = key.len() + 4;
                        let value_size = estimate_schema_size(prop_schema, depth + 1);
                        // Add comma overhead
                        total = total
                            .saturating_add(key_overhead)
                            .saturating_add(value_size)
                            .saturating_add(1);
                    }
                }

                // If additionalProperties is true or a schema, add some buffer
                if additional_props.is_some() {
                    total = total.saturating_add(DEFAULT_OBJECT_PROPERTY_SIZE * 5);
                }

                cmp::min(total, MAX_ESTIMATED_BODY_SIZE)
            }
            _ => DEFAULT_OBJECT_PROPERTY_SIZE,
        }
    }

    schema.map(|s| estimate_schema_size(s, 0))
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
                    ObjectOrReference::Ref { ref_path, .. } => resolve_schema_ref(spec, ref_path)
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
                        Some(ObjectOrReference::Ref { ref_path, .. }) => {
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
                for spec in mt_map.values() {
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
            ObjectOrReference::Ref { ref_path, .. } => resolve_parameter_ref(spec, ref_path),
        };

        if let Some(param) = param {
            let schema = param.schema.as_ref().and_then(|s| match s {
                ObjectOrReference::Object(obj) => serde_json::to_value(obj).ok(),
                ObjectOrReference::Ref { ref_path, .. } => resolve_schema_ref(spec, ref_path)
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

/// Extract the stack size vendor extension from an OpenAPI operation
///
/// Checks for `x-brrtrouter-stack-size` extension field to get an explicit
/// stack size override for the handler coroutine.
///
/// # Arguments
///
/// * `operation` - The OpenAPI operation definition
///
/// # Returns
///
/// The stack size in bytes if specified, otherwise `None`
pub fn extract_stack_size_override(operation: &oas3::spec::Operation) -> Option<usize> {
    operation
        .extensions
        .get("x-brrtrouter-stack-size")
        .and_then(|v| {
            v.as_u64()
                .map(|n| n as usize)
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
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

                // Estimate request body size from schema
                let estimated_request_body_bytes = estimate_body_size(request_schema.as_ref());

                // Extract vendor extension for stack size override
                let x_brrtrouter_stack_size = extract_stack_size_override(operation);

                // Extract route-specific CORS policy from x-cors extension
                let cors_policy = crate::middleware::extract_route_cors_config(operation);

                routes.push(RouteMeta {
                    method,
                    // JSF P0-2: Use Arc<str> for O(1) cloning
                    path_pattern: Arc::from(path.as_str()),
                    handler_name: Arc::from(handler_name.as_str()),
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
                    estimated_request_body_bytes,
                    x_brrtrouter_stack_size,
                    cors_policy,
                });
            }
        }
    }

    fail_if_issues(issues);
    Ok(routes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_estimate_body_size_string() {
        let schema = json!({
            "type": "string",
            "maxLength": 100
        });
        let size = estimate_body_size(Some(&schema));
        assert!(size.is_some());
        // String with maxLength 100 should estimate to 100*2 + 2 (quotes) = 202
        assert_eq!(size.unwrap(), 202);
    }

    #[test]
    fn test_estimate_body_size_string_no_max() {
        let schema = json!({
            "type": "string"
        });
        let size = estimate_body_size(Some(&schema));
        assert!(size.is_some());
        // Should use DEFAULT_MAX_STRING_LENGTH (1024)
        assert_eq!(size.unwrap(), 1024 * 2 + 2);
    }

    #[test]
    fn test_estimate_body_size_integer() {
        let schema = json!({
            "type": "integer"
        });
        let size = estimate_body_size(Some(&schema));
        assert_eq!(size, Some(20));
    }

    #[test]
    fn test_estimate_body_size_boolean() {
        let schema = json!({
            "type": "boolean"
        });
        let size = estimate_body_size(Some(&schema));
        assert_eq!(size, Some(5));
    }

    #[test]
    fn test_estimate_body_size_array() {
        let schema = json!({
            "type": "array",
            "maxItems": 10,
            "items": {
                "type": "string",
                "maxLength": 50
            }
        });
        let size = estimate_body_size(Some(&schema));
        assert!(size.is_some());
        // 10 items * (50*2+2) + 2 (brackets) + 9 (commas)
        let expected = 10 * 102 + 2 + 9;
        assert_eq!(size.unwrap(), expected);
    }

    #[test]
    fn test_estimate_body_size_array_no_max() {
        let schema = json!({
            "type": "array",
            "items": {
                "type": "integer"
            }
        });
        let size = estimate_body_size(Some(&schema));
        assert!(size.is_some());
        // Should use DEFAULT_MAX_ARRAY_ITEMS (100)
        let expected = 100 * 20 + 2 + 99;
        assert_eq!(size.unwrap(), expected);
    }

    #[test]
    fn test_estimate_body_size_object() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "maxLength": 50},
                "age": {"type": "integer"},
                "active": {"type": "boolean"}
            }
        });
        let size = estimate_body_size(Some(&schema));
        assert!(size.is_some());
        // {} brackets: 2
        // "name" key: 4 + 4 = 8, value: 102, comma: 1 = 111
        // "age" key: 3 + 4 = 7, value: 20, comma: 1 = 28
        // "active" key: 6 + 4 = 10, value: 5, comma: 1 = 16
        // Total: 2 + 111 + 28 + 16 = 157
        assert_eq!(size.unwrap(), 157);
    }

    #[test]
    fn test_estimate_body_size_nested_object() {
        let schema = json!({
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "maxLength": 30}
                    }
                }
            }
        });
        let size = estimate_body_size(Some(&schema));
        assert!(size.is_some());
        // Nested structure should be estimated
        assert!(size.unwrap() > 0);
    }

    #[test]
    fn test_estimate_body_size_vendor_extension() {
        let schema = json!({
            "type": "object",
            "x-brrtrouter-body-size-bytes": 5000
        });
        let size = estimate_body_size(Some(&schema));
        assert_eq!(size, Some(5000));
    }

    #[test]
    fn test_estimate_body_size_vendor_extension_capped() {
        let schema = json!({
            "type": "object",
            "x-brrtrouter-body-size-bytes": 50000000
        });
        let size = estimate_body_size(Some(&schema));
        // Should be capped at MAX_ESTIMATED_BODY_SIZE (10MB)
        assert_eq!(size, Some(MAX_ESTIMATED_BODY_SIZE));
    }

    #[test]
    fn test_estimate_body_size_none() {
        let size = estimate_body_size(None);
        assert_eq!(size, None);
    }

    #[test]
    fn test_estimate_body_size_prevents_recursion() {
        // Create a deeply nested structure
        let mut schema = json!({"type": "integer"});
        for _ in 0..20 {
            schema = json!({
                "type": "object",
                "properties": {
                    "nested": schema
                }
            });
        }
        let size = estimate_body_size(Some(&schema));
        assert!(size.is_some());
        // Should not panic or overflow
        assert!(size.unwrap() > 0);
    }
}
