use super::build::{build_routes, extract_security_schemes};
use super::types::RouteMeta;
use super::SecurityScheme;
use oas3::OpenApiV3Spec;

fn strip_unknown_verbs(val: &mut serde_json::Value) {
    const METHODS: [&str; 8] = ["get", "post", "put", "delete", "patch", "options", "head", "trace"];

    if let Some(paths) = val.get_mut("paths") {
        if let serde_json::Value::Object(paths_map) = paths {
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
}

pub fn load_spec(file_path: &str) -> anyhow::Result<(Vec<RouteMeta>, String)> {
    let content = std::fs::read_to_string(file_path)?;
    let mut value: serde_json::Value = if file_path.ends_with(".yaml") || file_path.ends_with(".yml") {
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
