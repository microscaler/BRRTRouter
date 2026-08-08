//! Pre-handler parameter validation (Story 12.4).
//!
//! After route match + auth, enforce OpenAPI **required** path/query/header/cookie
//! parameters and basic JSON Schema `type` checks before the handler runs.

use crate::dispatcher::HeaderVec;
use crate::router::ParamVec;
use crate::server::request::decode_param_value;
use crate::spec::{ParameterLocation, ParameterMeta};
use serde_json::{json, Value};

/// Max octets for a single query/header/cookie/path param value (DoS guard).
pub const MAX_PARAM_VALUE_OCTETS: usize = 8192;

/// Stable `reason` for 400 JSON bodies.
pub const REASON_PARAMETER_VALIDATION_FAILED: &str = "parameter_validation_failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamFieldError {
    pub name: String,
    pub location: String,
    pub error: String,
}

/// Validate required params + basic types against the matched route.
///
/// Returns `Ok(())` when all checks pass. On failure, returns the list of field errors
/// (never panics).
pub fn validate_route_parameters(
    parameters: &[ParameterMeta],
    path_params: &ParamVec,
    query_params: &ParamVec,
    headers: &HeaderVec,
    cookies: &HeaderVec,
) -> Result<(), Vec<ParamFieldError>> {
    if parameters.is_empty() {
        return Ok(());
    }
    let mut errors = Vec::new();
    for p in parameters {
        let raw = lookup_param(p, path_params, query_params, headers, cookies);
        match raw {
            None => {
                if p.required {
                    errors.push(ParamFieldError {
                        name: p.name.clone(),
                        location: location_str(&p.location).to_string(),
                        error: "required".to_string(),
                    });
                }
            }
            Some(value) => {
                if value.len() > MAX_PARAM_VALUE_OCTETS {
                    errors.push(ParamFieldError {
                        name: p.name.clone(),
                        location: location_str(&p.location).to_string(),
                        error: "value_too_large".to_string(),
                    });
                    continue;
                }
                if p.required && value.trim().is_empty() {
                    errors.push(ParamFieldError {
                        name: p.name.clone(),
                        location: location_str(&p.location).to_string(),
                        error: "required".to_string(),
                    });
                    continue;
                }
                if let Some(err) = type_mismatch(p, value) {
                    errors.push(ParamFieldError {
                        name: p.name.clone(),
                        location: location_str(&p.location).to_string(),
                        error: err,
                    });
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Stable 400 JSON body listing fields.
#[must_use]
pub fn param_validation_error_json(fields: &[ParamFieldError]) -> Value {
    json!({
        "error": "Bad Request",
        "reason": REASON_PARAMETER_VALIDATION_FAILED,
        "message": "One or more request parameters are missing or invalid",
        "fields": fields.iter().map(|f| json!({
            "name": f.name,
            "in": f.location,
            "error": f.error,
        })).collect::<Vec<_>>(),
    })
}

fn location_str(loc: &ParameterLocation) -> &'static str {
    match loc {
        ParameterLocation::Path => "path",
        ParameterLocation::Query => "query",
        ParameterLocation::Header => "header",
        ParameterLocation::Cookie => "cookie",
    }
}

fn lookup_param<'a>(
    p: &ParameterMeta,
    path_params: &'a ParamVec,
    query_params: &'a ParamVec,
    headers: &'a HeaderVec,
    cookies: &'a HeaderVec,
) -> Option<&'a str> {
    match p.location {
        ParameterLocation::Path => path_params
            .iter()
            .find(|(k, _)| k.as_ref() == p.name)
            .map(|(_, v)| v.as_str()),
        ParameterLocation::Query => query_params
            .iter()
            .find(|(k, _)| k.as_ref() == p.name)
            .map(|(_, v)| v.as_str()),
        ParameterLocation::Header => headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&p.name))
            .map(|(_, v)| v.as_str()),
        ParameterLocation::Cookie => cookies
            .iter()
            .find(|(k, _)| k.as_ref() == p.name || k.eq_ignore_ascii_case(&p.name))
            .map(|(_, v)| v.as_str()),
    }
}

/// Reject soft-coerce: if schema type is integer/number/boolean and decode keeps a String, fail.
fn type_mismatch(p: &ParameterMeta, value: &str) -> Option<String> {
    let Some(schema) = p.schema.as_ref() else {
        return None;
    };
    let Some(ty) = schema.get("type").and_then(|v| v.as_str()) else {
        return None;
    };
    match ty {
        "integer" | "number" | "boolean" => {
            let decoded = decode_param_value(value, Some(schema), p.style, p.explode);
            if matches!(decoded, Value::String(_)) {
                Some(format!("expected_{ty}"))
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::ParameterLocation;
    use serde_json::json;
    use std::sync::Arc;

    fn meta(
        name: &str,
        loc: ParameterLocation,
        required: bool,
        schema: Option<Value>,
    ) -> ParameterMeta {
        ParameterMeta {
            name: name.to_string(),
            location: loc,
            required,
            schema,
            style: None,
            explode: None,
        }
    }

    fn q(name: &str, val: &str) -> ParamVec {
        let mut v = ParamVec::new();
        v.push((Arc::from(name), val.to_string()));
        v
    }

    fn h(name: &str, val: &str) -> HeaderVec {
        let mut v = HeaderVec::new();
        v.push((Arc::from(name), val.to_string()));
        v
    }

    #[test]
    fn param_validation_p1_required_query_ok() {
        let params = [meta(
            "limit",
            ParameterLocation::Query,
            true,
            Some(json!({"type": "integer"})),
        )];
        assert!(validate_route_parameters(
            &params,
            &ParamVec::new(),
            &q("limit", "10"),
            &HeaderVec::new(),
            &HeaderVec::new()
        )
        .is_ok());
    }

    #[test]
    fn param_validation_p2_optional_omitted() {
        let params = [meta("q", ParameterLocation::Query, false, None)];
        assert!(validate_route_parameters(
            &params,
            &ParamVec::new(),
            &ParamVec::new(),
            &HeaderVec::new(),
            &HeaderVec::new()
        )
        .is_ok());
    }

    #[test]
    fn param_validation_n1_missing_query() {
        let params = [meta("limit", ParameterLocation::Query, true, None)];
        let err = validate_route_parameters(
            &params,
            &ParamVec::new(),
            &ParamVec::new(),
            &HeaderVec::new(),
            &HeaderVec::new(),
        )
        .unwrap_err();
        assert_eq!(err[0].error, "required");
        assert_eq!(err[0].location, "query");
    }

    #[test]
    fn param_validation_n2_missing_header() {
        let params = [meta("X-Request-Id", ParameterLocation::Header, true, None)];
        let err = validate_route_parameters(
            &params,
            &ParamVec::new(),
            &ParamVec::new(),
            &HeaderVec::new(),
            &HeaderVec::new(),
        )
        .unwrap_err();
        assert_eq!(err[0].location, "header");
    }

    #[test]
    fn param_validation_n3_wrong_type() {
        let params = [meta(
            "limit",
            ParameterLocation::Query,
            true,
            Some(json!({"type": "integer"})),
        )];
        let err = validate_route_parameters(
            &params,
            &ParamVec::new(),
            &q("limit", "abc"),
            &HeaderVec::new(),
            &HeaderVec::new(),
        )
        .unwrap_err();
        assert_eq!(err[0].error, "expected_integer");
    }

    #[test]
    fn param_validation_n4_empty_required() {
        let params = [meta("q", ParameterLocation::Query, true, None)];
        let err = validate_route_parameters(
            &params,
            &ParamVec::new(),
            &q("q", "  "),
            &HeaderVec::new(),
            &HeaderVec::new(),
        )
        .unwrap_err();
        assert_eq!(err[0].error, "required");
    }

    #[test]
    fn param_validation_n6_error_json_shape() {
        let fields = [ParamFieldError {
            name: "q".into(),
            location: "query".into(),
            error: "required".into(),
        }];
        let v = param_validation_error_json(&fields);
        assert_eq!(v["reason"], REASON_PARAMETER_VALIDATION_FAILED);
        assert!(v["fields"].as_array().unwrap().len() == 1);
    }

    #[test]
    fn param_validation_p3_header_case_insensitive() {
        let params = [meta("X-Trace", ParameterLocation::Header, true, None)];
        assert!(validate_route_parameters(
            &params,
            &ParamVec::new(),
            &ParamVec::new(),
            &h("x-trace", "1"),
            &HeaderVec::new()
        )
        .is_ok());
    }
}
