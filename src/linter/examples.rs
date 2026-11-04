//! Request and response examples whose values do not match their schema's JSON types.
//!
//! The generator turns a response example into the Rust literal the generated controller returns. A
//! value of the wrong JSON type for its property (a string where the schema says `integer`, say) is
//! wrong documentation and cannot become a literal of the generated type, so the generator drops it and
//! uses a placeholder. This lint names every such value. It checks JSON types only, through `$ref`,
//! `allOf`, `properties` and `items`; formats, enums, required and additional properties are out of
//! its scope.

use super::{LintIssue, LintSeverity};
use serde_json::Value;

const METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// Deepest schema nesting followed, so a recursive schema cannot recurse forever.
const MAX_DEPTH: usize = 32;

/// One mismatch: JSON pointer into the example, the schema's type(s), the value's JSON kind.
type Mismatch = (String, String, &'static str);

/// Lint every `example` and `examples.*.value` of every operation's request body and responses.
pub(super) fn lint_examples(spec: &Value, issues: &mut Vec<LintIssue>) {
    let Some(paths) = spec.get("paths").and_then(Value::as_object) else {
        return;
    };
    for (path, item) in paths {
        for method in METHODS {
            let Some(op) = item.get(method) else {
                continue;
            };
            let location = format!("{path} {}", method.to_uppercase());
            if let Some(body) = op.get("requestBody").and_then(|b| resolve(spec, b)) {
                lint_content(spec, body, &format!("{location} (requestBody)"), issues);
            }
            if let Some(responses) = op.get("responses").and_then(Value::as_object) {
                for (status, response) in responses {
                    if let Some(response) = resolve(spec, response) {
                        lint_content(
                            spec,
                            response,
                            &format!("{location} (response {status})"),
                            issues,
                        );
                    }
                }
            }
        }
    }
}

fn lint_content(spec: &Value, holder: &Value, location: &str, issues: &mut Vec<LintIssue>) {
    let Some(content) = holder.get("content").and_then(Value::as_object) else {
        return;
    };
    for (media, media_type) in content {
        let Some(schema) = media_type.get("schema") else {
            continue;
        };
        let mut examples: Vec<(String, &Value)> = Vec::new();
        if let Some(example) = media_type.get("example") {
            examples.push(("example".to_string(), example));
        }
        if let Some(named) = media_type.get("examples").and_then(Value::as_object) {
            for (name, example) in named {
                if let Some(value) = resolve(spec, example).and_then(|e| e.get("value")) {
                    examples.push((format!("examples.{name}"), value));
                }
            }
        }
        for (which, example) in examples {
            let mut mismatches = Vec::new();
            check(spec, schema, example, "", &mut mismatches, 0);
            for (pointer, expected, found) in mismatches {
                let at = if pointer.is_empty() {
                    String::new()
                } else {
                    format!(" at {pointer}")
                };
                issues.push(
                    LintIssue::new(
                        location,
                        LintSeverity::Warning,
                        "example_type_mismatch",
                        format!("{media} {which}{at}: the value is {found} but the schema says {expected}"),
                    )
                    .with_suggestion(
                        "Make the example match the schema; the generator drops a mismatched value \
                         and the generated controller returns a placeholder instead",
                    ),
                );
            }
        }
    }
}

/// Follow a local `$ref` (any `#/...` JSON pointer); anything else is returned as is.
fn resolve<'a>(spec: &'a Value, v: &'a Value) -> Option<&'a Value> {
    match v.get("$ref").and_then(Value::as_str) {
        Some(reference) => spec.pointer(reference.strip_prefix('#')?),
        None => Some(v),
    }
}

fn check(
    spec: &Value,
    schema: &Value,
    value: &Value,
    pointer: &str,
    out: &mut Vec<Mismatch>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let Some(schema) = resolve(spec, schema) else {
        return;
    };
    if let Some(members) = schema.get("allOf").and_then(Value::as_array) {
        for member in members {
            check(spec, member, value, pointer, out, depth + 1);
        }
    }
    let types = declared_types(schema);
    if !types.is_empty() && !types.iter().any(|t| matches_type(t, value)) {
        out.push((pointer.to_string(), types.join(" or "), json_kind(value)));
        return;
    }
    match value {
        Value::Object(map) => {
            if let Some(props) = schema.get("properties").and_then(Value::as_object) {
                for (key, v) in map {
                    if let Some(prop) = props.get(key) {
                        check(spec, prop, v, &format!("{pointer}/{key}"), out, depth + 1);
                    }
                }
            }
        }
        Value::Array(items) => {
            if let Some(item_schema) = schema.get("items") {
                for (i, v) in items.iter().enumerate() {
                    check(
                        spec,
                        item_schema,
                        v,
                        &format!("{pointer}/{i}"),
                        out,
                        depth + 1,
                    );
                }
            }
        }
        _ => {}
    }
}

/// The schema's `type` (a string or, in OpenAPI 3.1, a list), plus `null` for 3.0 `nullable: true`.
fn declared_types(schema: &Value) -> Vec<&str> {
    let mut types: Vec<&str> = match schema.get("type") {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(ts)) => ts.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    if !types.is_empty() && schema.get("nullable").and_then(Value::as_bool) == Some(true) {
        types.push("null");
    }
    types
}

fn matches_type(t: &str, v: &Value) -> bool {
    match t {
        "null" => v.is_null(),
        "boolean" => v.is_boolean(),
        // 1.0 is a number, not an integer literal: as generated code it does not fit an integer field.
        "integer" => v.is_i64() || v.is_u64(),
        "number" => v.is_number(),
        "string" => v.is_string(),
        "array" => v.is_array(),
        "object" => v.is_object(),
        _ => true,
    }
}

fn json_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(n) if n.is_f64() => "a number",
        Value::Number(_) => "an integer",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}
