#![allow(clippy::unwrap_used, clippy::expect_used)]

use brrtrouter::server::decode_param_value;
use brrtrouter::spec::ParameterStyle;
use serde_json::json;

#[test]
fn test_decode_array_csv() {
    let schema = json!({"type": "array", "items": {"type": "integer"}});
    let v = decode_param_value(
        "1,2,3",
        Some(&schema),
        Some(ParameterStyle::Form),
        Some(false),
    );
    assert_eq!(v, json!([1, 2, 3]));
}

#[test]
fn test_decode_array_pipe() {
    let schema = json!({"type": "array", "items": {"type": "string"}});
    let v = decode_param_value(
        "a|b|c",
        Some(&schema),
        Some(ParameterStyle::PipeDelimited),
        Some(false),
    );
    assert_eq!(v, json!(["a", "b", "c"]));
}

#[test]
fn test_decode_object_json() {
    let schema = json!({"type": "object"});
    let v = decode_param_value("{\"a\":1}", Some(&schema), None, None);
    assert_eq!(v, json!({"a":1}));
}

/// Matrix path ` /matrix/{coords}` may capture `1;2;3` (semicolons). Must decode to integers, not a
/// single string (which caused 400: expected i32).
#[test]
fn test_decode_array_matrix_semicolon_separated() {
    let schema = json!({"type": "array", "items": {"type": "integer"}});
    let v = decode_param_value(
        "1;2;3",
        Some(&schema),
        Some(ParameterStyle::Matrix),
        Some(false),
    );
    assert_eq!(v, json!([1, 2, 3]));
}

/// OpenAPI matrix: values after `;name=` are often comma-separated.
#[test]
fn test_decode_array_matrix_comma_after_name() {
    let schema = json!({"type": "array", "items": {"type": "integer"}});
    let v = decode_param_value(
        "coords=1,2,3",
        Some(&schema),
        Some(ParameterStyle::Matrix),
        Some(false),
    );
    assert_eq!(v, json!([1, 2, 3]));
}

#[test]
fn test_decode_array_matrix_comma_only() {
    let schema = json!({"type": "array", "items": {"type": "integer"}});
    let v = decode_param_value(
        "1,2,3",
        Some(&schema),
        Some(ParameterStyle::Matrix),
        Some(false),
    );
    assert_eq!(v, json!([1, 2, 3]));
}
