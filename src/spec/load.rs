use super::build::{build_routes_with_security_presence, extract_security_schemes};
use super::security_presence::extract_operation_security_presence;
use super::types::RouteMeta;
use super::SecurityScheme;
use oas3::OpenApiV3Spec;

/// OpenAPI path-item extension carrying an RFC 10008 QUERY operation.
///
/// `oas3` 0.21 has no `PathItem::query` field; bare `query:` keys would be
/// stripped before deserialize. Loaders rewrite `query` / `QUERY` into this
/// extension (Story 11.2). After deserialize the key is `brrtrouter-query`
/// (the `x-` prefix is stripped by oas3).
pub const QUERY_OPERATION_EXTENSION: &str = "x-brrtrouter-query";

/// Extension map key after oas3 strips the `x-` prefix.
pub const QUERY_OPERATION_EXTENSION_KEY: &str = "brrtrouter-query";

/// Promote path-level `query` / `QUERY` operations to [`QUERY_OPERATION_EXTENSION`].
///
/// Fails closed on duplicates or non-object operation values so QUERY is never
/// silently dropped.
pub fn promote_query_operations(val: &mut serde_json::Value) -> anyhow::Result<()> {
    let Some(serde_json::Value::Object(paths_map)) = val.get_mut("paths") else {
        return Ok(());
    };

    for (path, item) in paths_map.iter_mut() {
        let serde_json::Value::Object(obj) = item else {
            continue;
        };

        let mut promoted: Option<serde_json::Value> = None;
        let query_keys: Vec<String> = obj
            .keys()
            .filter(|k| k.eq_ignore_ascii_case("query"))
            .cloned()
            .collect();

        for k in query_keys {
            let Some(op) = obj.remove(&k) else {
                continue;
            };
            if !op.is_object() {
                anyhow::bail!(
                    "path '{path}': QUERY operation ('{k}') must be an object, got {}",
                    value_kind(&op)
                );
            }
            if promoted.is_some() {
                anyhow::bail!(
                    "path '{path}': conflicting duplicate QUERY operations (multiple query keys)"
                );
            }
            promoted = Some(op);
        }

        if let Some(op) = promoted {
            if obj.contains_key(QUERY_OPERATION_EXTENSION) {
                anyhow::bail!(
                    "path '{path}': conflicting duplicate QUERY operations \
                     ('query' and {QUERY_OPERATION_EXTENSION} both present)"
                );
            }
            obj.insert(QUERY_OPERATION_EXTENSION.to_string(), op);
        }

        if let Some(ext) = obj.get(QUERY_OPERATION_EXTENSION) {
            if !ext.is_object() {
                anyhow::bail!(
                    "path '{path}': {QUERY_OPERATION_EXTENSION} must be an object, got {}",
                    value_kind(ext)
                );
            }
        }
    }
    Ok(())
}

fn value_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn strip_unknown_verbs(val: &mut serde_json::Value) {
    const METHODS: [&str; 8] = [
        "get", "post", "put", "delete", "patch", "options", "head", "trace",
    ];

    if let Some(serde_json::Value::Object(paths_map)) = val.get_mut("paths") {
        for item in paths_map.values_mut() {
            if let serde_json::Value::Object(obj) = item {
                let keys: Vec<String> = obj.keys().cloned().collect();
                for k in keys {
                    let lk = k.to_ascii_lowercase();
                    let keep = match lk.as_str() {
                        "summary" | "description" | "servers" | "parameters" | "$ref" => true,
                        m if METHODS.contains(&m) => true,
                        _ => k.starts_with("x-"),
                    };
                    if !keep {
                        obj.remove(&k);
                    }
                }
            }
        }
    }
}

fn prepare_spec_value(val: &mut serde_json::Value) -> anyhow::Result<()> {
    promote_query_operations(val)?;
    strip_unknown_verbs(val);
    Ok(())
}

/// Load an OpenAPI specification from a file and extract route metadata
///
/// Supports both YAML and JSON formats. Returns route metadata and a URL-safe project slug
/// derived from the API title.
///
/// # Arguments
///
/// * `file_path` - Path to the OpenAPI specification file
///
/// # Returns
///
/// A tuple of:
/// * `Vec<RouteMeta>` - Route metadata for all operations in the spec
/// * `String` - URL-safe project slug derived from API title
///
/// # Errors
///
/// Returns an error if:
/// - The file cannot be read
/// - The spec is invalid YAML/JSON
/// - The spec doesn't conform to OpenAPI 3.x
/// - Route extraction fails
pub fn load_spec(file_path: &str) -> anyhow::Result<(Vec<RouteMeta>, String)> {
    let content = std::fs::read_to_string(file_path)?;
    let mut value: serde_json::Value =
        if file_path.ends_with(".yaml") || file_path.ends_with(".yml") {
            serde_yaml::from_str(&content)?
        } else {
            serde_json::from_str(&content)?
        };

    prepare_spec_value(&mut value)?;
    let security_presence = extract_operation_security_presence(&value);
    let spec: OpenApiV3Spec = serde_json::from_value(value)?;

    let title = spec
        .info
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        .trim_matches('_')
        .to_string();

    let routes = build_routes_with_security_presence(&spec, &title, Some(&security_presence))?;
    Ok((routes, title))
}

/// Load an OpenAPI specification with full security scheme information
///
/// Like `load_spec` but also extracts security schemes for authentication/authorization.
///
/// # Arguments
///
/// * `file_path` - Path to the OpenAPI specification file
///
/// # Returns
///
/// A tuple of:
/// * `Vec<RouteMeta>` - Route metadata for all operations
/// * `HashMap<String, SecurityScheme>` - Security schemes defined in the spec
/// * `String` - URL-safe project slug
///
/// # Errors
///
/// Returns an error if the spec cannot be loaded or parsed.
pub fn load_spec_full(
    file_path: &str,
) -> anyhow::Result<(
    Vec<RouteMeta>,
    std::collections::HashMap<String, SecurityScheme>,
    String,
)> {
    let content = std::fs::read_to_string(file_path)?;
    let mut value: serde_json::Value =
        if file_path.ends_with(".yaml") || file_path.ends_with(".yml") {
            serde_yaml::from_str(&content)?
        } else {
            serde_json::from_str(&content)?
        };

    prepare_spec_value(&mut value)?;
    let security_presence = extract_operation_security_presence(&value);
    let spec: OpenApiV3Spec = serde_json::from_value(value)?;

    let title = spec
        .info
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        .trim_matches('_')
        .to_string();

    let routes = build_routes_with_security_presence(&spec, &title, Some(&security_presence))?;
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

    let routes = build_routes_with_security_presence(&spec, &slug, None)?;
    Ok(routes)
}

/// Extract route metadata and security schemes from an already-parsed OpenAPI spec
///
/// Useful when you already have a parsed `OpenApiV3Spec` and want to extract
/// both routes and security information without reloading from a file.
///
/// # Arguments
///
/// * `spec` - Parsed OpenAPI specification
///
/// # Returns
///
/// A tuple of:
/// * `Vec<RouteMeta>` - Route metadata
/// * `HashMap<String, SecurityScheme>` - Security schemes
///
/// # Errors
///
/// Returns an error if route extraction fails.
pub fn load_spec_from_spec_full(
    spec: OpenApiV3Spec,
) -> anyhow::Result<(
    Vec<RouteMeta>,
    std::collections::HashMap<String, SecurityScheme>,
)> {
    let slug = spec
        .info
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        .trim_matches('_')
        .to_string();
    let routes = build_routes_with_security_presence(&spec, &slug, None)?;
    let schemes = extract_security_schemes(&spec);
    Ok((routes, schemes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_strip_unknown_verbs() {
        let mut v = json!({
            "paths": {
                "/x": { "get": {}, "patch": {}, "unknown": {} }
            }
        });
        strip_unknown_verbs(&mut v);
        assert!(v["paths"]["/x"].get("unknown").is_none());
    }

    #[test]
    fn promote_query_rewrites_to_extension() {
        let mut v = json!({
            "paths": {
                "/search": {
                    "get": { "operationId": "get_search" },
                    "query": {
                        "operationId": "query_search",
                        "requestBody": { "required": true, "content": {} }
                    }
                }
            }
        });
        promote_query_operations(&mut v).unwrap();
        assert!(v["paths"]["/search"].get("query").is_none());
        assert!(v["paths"]["/search"][QUERY_OPERATION_EXTENSION].is_object());
        strip_unknown_verbs(&mut v);
        assert!(v["paths"]["/search"][QUERY_OPERATION_EXTENSION].is_object());
    }

    #[test]
    fn promote_query_rejects_duplicate_with_extension() {
        let mut v = json!({
            "paths": {
                "/search": {
                    "query": { "operationId": "a" },
                    "x-brrtrouter-query": { "operationId": "b" }
                }
            }
        });
        let err = promote_query_operations(&mut v).unwrap_err().to_string();
        assert!(err.contains("duplicate"), "{err}");
    }

    #[test]
    fn promote_query_rejects_non_object() {
        let mut v = json!({
            "paths": { "/search": { "query": "not-an-operation" } }
        });
        let err = promote_query_operations(&mut v).unwrap_err().to_string();
        assert!(err.contains("must be an object"), "{err}");
    }
}
