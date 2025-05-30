use brrtrouter::server::decode_param_value;
use brrtrouter::spec::ParameterStyle;
use serde_json::json;

#[test]
fn test_decode_array_csv() {
    let schema = json!({"type": "array", "items": {"type": "integer"}});
    let v = decode_param_value("1,2,3", Some(&schema), Some(ParameterStyle::Form), Some(false));
    assert_eq!(v, json!([1,2,3]));
}

#[test]
fn test_decode_array_pipe() {
    let schema = json!({"type": "array", "items": {"type": "string"}});
    let v = decode_param_value("a|b|c", Some(&schema), Some(ParameterStyle::PipeDelimited), Some(false));
    assert_eq!(v, json!( ["a","b","c"] ));
}

#[test]
fn test_decode_object_json() {
    let schema = json!({"type": "object"});
    let v = decode_param_value("{\"a\":1}", Some(&schema), None, None);
    assert_eq!(v, json!({"a":1}));
}
