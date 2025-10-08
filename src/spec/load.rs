use super::build::{build_routes, extract_security_schemes};
use super::types::RouteMeta;
use super::SecurityScheme;
use oas3::OpenApiV3Spec;

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

    strip_unknown_verbs(&mut value);
    let spec: OpenApiV3Spec = serde_json::from_value(value)?;

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
    let routes = build_routes(&spec, &slug)?;
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
}
