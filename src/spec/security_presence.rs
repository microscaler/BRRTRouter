//! Tracks whether OpenAPI `security` keys were **explicitly present** in the raw spec.
//!
//! `oas3` deserializes both omitted `security` and explicit `security: []` to an empty
//! `Vec`. OpenAPI 3 assigns different meanings (inherit global vs public). We recover
//! the distinction by scanning the parsed JSON/YAML value before struct deserialization.

use std::collections::HashSet;

use oas3::spec::Operation;
use oas3::OpenApiV3Spec;

use super::SecurityRequirement;

const HTTP_METHODS: [&str; 8] = [
    "get", "post", "put", "delete", "patch", "options", "head", "trace",
];

/// Paths and HTTP methods whose operation object contained an explicit `security` key.
#[derive(Debug, Default, Clone)]
pub struct OperationSecurityPresence {
    explicit_operation: HashSet<(String, String)>,
}

impl OperationSecurityPresence {
    /// Returns true when the operation object had an explicit `security` field in the spec.
    #[must_use]
    pub fn operation_security_explicit(&self, path: &str, method: &str) -> bool {
        self.explicit_operation
            .contains(&(path.to_string(), method.to_ascii_lowercase()))
    }
}

/// Scan raw OpenAPI JSON for explicit operation-level `security` keys.
#[must_use]
pub fn extract_operation_security_presence(value: &serde_json::Value) -> OperationSecurityPresence {
    let mut explicit_operation = HashSet::new();

    let Some(paths) = value.get("paths").and_then(|p| p.as_object()) else {
        return OperationSecurityPresence { explicit_operation };
    };

    for (path, item) in paths {
        let Some(obj) = item.as_object() else {
            continue;
        };
        for method in HTTP_METHODS {
            let Some(op) = obj.get(method).and_then(|o| o.as_object()) else {
                continue;
            };
            if op.contains_key("security") {
                explicit_operation.insert((path.clone(), method.to_string()));
            }
        }
    }

    OperationSecurityPresence { explicit_operation }
}

/// Resolve effective security for an operation per OpenAPI 3 inheritance rules.
///
/// - Non-empty operation `security` → use as-is (includes explicit requirements).
/// - Explicit `security: []` on the operation → public (empty vec).
/// - Omitted operation `security` → inherit [`OpenApiV3Spec::security`].
#[must_use]
pub fn resolve_operation_security(
    path: &str,
    method: &str,
    operation: &Operation,
    spec: &OpenApiV3Spec,
    presence: Option<&OperationSecurityPresence>,
) -> Vec<SecurityRequirement> {
    if !operation.security.is_empty() {
        return operation.security.clone();
    }

    if let Some(p) = presence {
        if p.operation_security_explicit(path, method) {
            return Vec::new();
        }
    }

    spec.security.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spec_with_global() -> OpenApiV3Spec {
        let yaml = r"
openapi: 3.1.0
info:
  title: Test
  version: '1.0'
security:
  - BearerAuth: []
paths:
  /public:
    get:
      operationId: public_get
      security: []
      responses:
        '200':
          description: OK
  /protected:
    get:
      operationId: protected_get
      responses:
        '200':
          description: OK
  /explicit:
    get:
      operationId: explicit_get
      security:
        - ApiKey: []
      responses:
        '200':
          description: OK
components:
  securitySchemes:
    BearerAuth:
      type: http
      scheme: bearer
    ApiKey:
      type: apiKey
      in: header
      name: X-API-Key
";
        serde_yaml::from_str(yaml).unwrap()
    }

    #[test]
    fn explicit_empty_security_is_public() {
        let raw: serde_json::Value = serde_yaml::from_str(
            r"
openapi: 3.1.0
info: { title: T, version: '1' }
security:
  - BearerAuth: []
paths:
  /public:
    get:
      operationId: public_get
      security: []
      responses:
        '200': { description: OK }
",
        )
        .unwrap();
        let presence = extract_operation_security_presence(&raw);
        let spec: OpenApiV3Spec = serde_json::from_value(raw).unwrap();
        let op = spec
            .paths
            .as_ref()
            .unwrap()
            .get("/public")
            .unwrap()
            .get
            .as_ref()
            .unwrap();

        let sec = resolve_operation_security("/public", "get", op, &spec, Some(&presence));
        assert!(sec.is_empty(), "security: [] must override global security");
    }

    #[test]
    fn omitted_security_inherits_global() {
        let raw: serde_json::Value = serde_yaml::from_str(
            r"
openapi: 3.1.0
info: { title: T, version: '1' }
security:
  - BearerAuth: []
paths:
  /protected:
    get:
      operationId: protected_get
      responses:
        '200': { description: OK }
",
        )
        .unwrap();
        let presence = extract_operation_security_presence(&raw);
        let spec: OpenApiV3Spec = serde_json::from_value(raw).unwrap();
        let op = spec
            .paths
            .as_ref()
            .unwrap()
            .get("/protected")
            .unwrap()
            .get
            .as_ref()
            .unwrap();

        let sec = resolve_operation_security("/protected", "get", op, &spec, Some(&presence));
        assert_eq!(sec.len(), 1);
        assert!(sec[0].0.contains_key("BearerAuth"));
    }

    #[test]
    fn explicit_non_empty_security_used() {
        let spec = spec_with_global();
        let op = spec
            .paths
            .as_ref()
            .unwrap()
            .get("/explicit")
            .unwrap()
            .get
            .as_ref()
            .unwrap();
        let sec = resolve_operation_security("/explicit", "get", op, &spec, None);
        assert_eq!(sec.len(), 1);
        assert!(sec[0].0.contains_key("ApiKey"));
    }

    #[test]
    fn extract_presence_detects_explicit_key_only() {
        let v = json!({
            "paths": {
                "/a": {
                    "get": { "operationId": "a", "security": [] },
                    "post": { "operationId": "b" }
                }
            }
        });
        let p = extract_operation_security_presence(&v);
        assert!(p.operation_security_explicit("/a", "get"));
        assert!(!p.operation_security_explicit("/a", "post"));
    }
}
