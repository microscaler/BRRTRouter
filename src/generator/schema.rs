use crate::dummy_value;
use crate::spec::{resolve_schema_ref, ParameterMeta};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct TypeDefinition {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub original_name: String,
    pub ty: String,
    pub optional: bool,
    pub value: String,
}

pub fn to_camel_case(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

pub fn is_named_type(ty: &str) -> bool {
    let primitives = [
        "String",
        "i32",
        "i64",
        "f32",
        "f64",
        "bool",
        "Value",
        "serde_json::Value",
    ];
    if let Some(inner) = ty.strip_prefix("Vec<").and_then(|s| s.strip_suffix(">")) {
        return !primitives.contains(&inner)
            && !inner.starts_with("serde_json")
            && matches!(inner.chars().next(), Some('A'..='Z'));
    }
    !primitives.contains(&ty) && matches!(ty.chars().next(), Some('A'..='Z'))
}

fn sanitize_rust_identifier(name: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn",
    ];
    if KEYWORDS.contains(&name) {
        format!("r#{}", name)
    } else {
        name.to_string()
    }
}

fn sanitize_field_name(name: &str) -> String {
    // Replace invalid identifier characters with underscores and ensure it doesn't start with a digit.
    let mut s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        s = "_".to_string();
    }
    if s.chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        s.insert(0, '_');
    }
    s
}

pub(crate) fn unique_handler_name(seen: &mut HashSet<String>, name: &str) -> String {
    if !seen.contains(name) {
        seen.insert(name.to_string());
        return name.to_string();
    }
    let mut counter = 1;
    loop {
        let candidate = format!("{name}_{counter}");
        if !seen.contains(&candidate) {
            println!("⚠️  Duplicate handler name '{name}' → using '{candidate}'");
            seen.insert(candidate.clone());
            return candidate;
        }
        counter += 1;
    }
}

pub fn rust_literal_for_example(field: &FieldDef, example: &Value) -> String {
    let literal = match example {
        Value::String(s) => {
            if field.ty == "serde_json::Value" || field.ty == "Value" {
                format!("serde_json::Value::String({s:?}.to_string())")
            } else {
                format!("{s:?}.to_string()")
            }
        }
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(items) => {
            let inner_ty_opt = field
                .ty
                .strip_prefix("Vec<")
                .and_then(|s| s.strip_suffix(">"));
            let is_vec_string = inner_ty_opt == Some("String");
            let is_vec_json_value =
                inner_ty_opt == Some("serde_json::Value") || inner_ty_opt == Some("Value");
            let inner = items
                .iter()
                .map(|item| match item {
                    Value::String(s) => {
                        if is_vec_string {
                            format!("{s:?}.to_string()")
                        } else if is_vec_json_value {
                            format!("serde_json::Value::String({s:?}.to_string())")
                        } else {
                            format!("{s:?}.to_string().parse().unwrap()")
                        }
                    }
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Object(_) => {
                        if let Some(inner_ty) = inner_ty_opt {
                            if inner_ty == "serde_json::Value" || inner_ty == "Value" {
                                let json = serde_json::to_string(item).unwrap_or_else(|_| "null".to_string());
                                format!("serde_json::json!({json})")
                            } else if is_named_type(inner_ty) {
                                let json = serde_json::to_string(item).unwrap_or_else(|_| "null".to_string());
                                format!(
                                    "match serde_json::from_value::<{inner_ty}>(serde_json::json!({json})) {{ Ok(v) => v, Err(_) => Default::default() }}"
                                )
                            } else {
                                dummy_value::dummy_value(inner_ty).unwrap_or_else(|_| "Default::default()".to_string())
                            }
                        } else {
                            let json = serde_json::to_string(item).unwrap_or_else(|_| "null".to_string());
                            format!("serde_json::json!({json})")
                        }
                    }
                    _ => {
                        if let Some(inner_ty) = inner_ty_opt {
                            dummy_value::dummy_value(inner_ty).unwrap_or_else(|_| "Default::default()".to_string())
                        } else if is_vec_json_value {
                            "serde_json::Value::Null".to_string()
                        } else {
                            "Default::default()".to_string()
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("vec![{inner}]")
        }
        Value::Object(_) => {
            let json = serde_json::to_string(example).unwrap_or_else(|_| "null".to_string());
            if field.ty == "serde_json::Value" || field.ty == "Value" {
                format!("serde_json::json!({json})")
            } else if is_named_type(&field.ty) {
                format!(
                    "match serde_json::from_value::<{}>(serde_json::json!({json})) {{ Ok(v) => v, Err(_) => Default::default() }}",
                    field.ty
                )
            } else {
                format!("serde_json::json!({json})")
            }
        }
        _ => {
            if field.ty == "serde_json::Value" || field.ty == "Value" {
                "serde_json::Value::Null".to_string()
            } else {
                dummy_value::dummy_value(&field.ty)
                    .unwrap_or_else(|_| "Default::default()".to_string())
            }
        }
    };
    if field.optional {
        format!("Some({literal})")
    } else {
        literal
    }
}

pub fn process_schema_type(
    name: &str,
    schema: &Value,
    types: &mut HashMap<String, TypeDefinition>,
) {
    let name = to_camel_case(name);
    if types.contains_key(&name) {
        return;
    }
    let fields = extract_fields(schema);
    if !fields.is_empty() {
        types.insert(name.clone(), TypeDefinition { name, fields });
    }
}

pub fn extract_fields(schema: &Value) -> Vec<FieldDef> {
    let mut fields = vec![];
    if let Some(schema_type) = schema.get("type").and_then(|t| t.as_str()) {
        if schema_type == "array" {
            if let Some(items) = schema.get("items") {
                let ty = schema_to_type(items);
                fields.push(FieldDef {
                    name: "items".to_string(),
                    original_name: "items".to_string(),
                    ty: format!("Vec<{ty}>"),
                    optional: false,
                    value: "vec![]".to_string(),
                });
                return fields;
            }
        }
    }
    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        for (name, prop) in props {
            // Detect oneOf with null → map to Option<Inner>
            let (mut inferred_ty, mut nullable_oneof) =
                if let Some(one_of) = prop.get("oneOf").and_then(|v| v.as_array()) {
                    let mut inner_ty: Option<String> = None;
                    let mut has_null = false;
                    for variant in one_of {
                        if variant.get("type").and_then(|t| t.as_str()) == Some("null") {
                            has_null = true;
                        } else {
                            inner_ty = Some(schema_to_type(variant));
                        }
                    }
                    (
                        inner_ty.unwrap_or_else(|| "serde_json::Value".to_string()),
                        has_null,
                    )
                } else {
                    (String::new(), false)
                };

            let ty = if !inferred_ty.is_empty() {
                inferred_ty
            } else if let Some(name) = prop.get("x-ref-name").and_then(|v| v.as_str()) {
                to_camel_case(name)
            } else if let Some(r) = prop.get("$ref").and_then(|v| v.as_str()) {
                if let Some(name) = r.strip_prefix("#/components/schemas/") {
                    to_camel_case(name)
                } else {
                    "serde_json::Value".to_string()
                }
            } else {
                match prop.get("type").and_then(|t| t.as_str()) {
                    Some("string") => "String".to_string(),
                    Some("integer") => "i32".to_string(),
                    Some("number") => "f64".to_string(),
                    Some("boolean") => "bool".to_string(),
                    Some("array") => {
                        if let Some(items) = prop.get("items") {
                            format!("Vec<{}>", schema_to_type(items))
                        } else {
                            "Vec<serde_json::Value>".to_string()
                        }
                    }
                    _ => "serde_json::Value".to_string(),
                }
            };
            let optional = !required.contains(name) || nullable_oneof;
            let value = dummy_value::dummy_value(&ty)
                .map(|v| if optional { format!("Some({v})") } else { v })
                .unwrap_or_else(|_| "Default::default()".to_string());
            fields.push(FieldDef {
                name: sanitize_field_name(name),
                original_name: name.clone(),
                ty,
                optional,
                value,
            });
        }
    }
    fields
}

pub fn schema_to_type(schema: &Value) -> String {
    if let Some(name) = schema.get("x-ref-name").and_then(|v| v.as_str()) {
        return to_camel_case(name);
    }
    if let Some(r) = schema.get("$ref").and_then(|v| v.as_str()) {
        if let Some(name) = r.strip_prefix("#/components/schemas/") {
            return to_camel_case(name);
        }
        return "serde_json::Value".to_string();
    }
    match schema.get("type").and_then(|t| t.as_str()) {
        Some("string") => "String".to_string(),
        Some("integer") => "i32".to_string(),
        Some("number") => "f64".to_string(),
        Some("boolean") => "bool".to_string(),
        Some("array") => {
            if let Some(items) = schema.get("items") {
                if let Some(item_ty) = items.get("type").and_then(|v| v.as_str()) {
                    let inner = match item_ty {
                        "string" => "String".to_string(),
                        "integer" => "i32".to_string(),
                        "number" => "f64".to_string(),
                        "boolean" => "bool".to_string(),
                        _ => schema_to_type(items),
                    };
                    return format!("Vec<{inner}>");
                }
                if let Some(item_ref) = items.get("$ref").and_then(|v| v.as_str()) {
                    if let Some(name) = item_ref.strip_prefix("#/components/schemas/") {
                        return format!("Vec<{}>", to_camel_case(name));
                    }
                }
                return format!("Vec<{}>", schema_to_type(items));
            }
            "Vec<serde_json::Value>".to_string()
        }
        _ => "serde_json::Value".to_string(),
    }
}

pub fn parameter_to_field(param: &ParameterMeta) -> FieldDef {
    let ty = param
        .schema
        .as_ref()
        .map(schema_to_type)
        .unwrap_or_else(|| "String".to_string());
    let optional = !param.required;
    let value = dummy_value::dummy_value(&ty)
        .map(|v| if optional { format!("Some({v})") } else { v })
        .unwrap_or_else(|_| "Default::default()".to_string());
    FieldDef {
        name: sanitize_field_name(&param.name),
        original_name: param.name.clone(),
        ty,
        optional,
        value,
    }
}

pub fn collect_component_schemas(
    spec_path: &std::path::Path,
) -> anyhow::Result<HashMap<String, TypeDefinition>> {
    let spec: oas3::OpenApiV3Spec = if spec_path.extension().map(|s| s == "yaml").unwrap_or(false) {
        serde_yaml::from_str(&std::fs::read_to_string(spec_path)?)?
    } else {
        serde_json::from_str(&std::fs::read_to_string(spec_path)?)?
    };
    let mut types = HashMap::new();
    if let Some(components) = spec.components.as_ref() {
        for (name, schema) in &components.schemas {
            match schema {
                oas3::spec::ObjectOrReference::Object(obj) => {
                    let json = serde_json::to_value(obj).unwrap_or_default();
                    process_schema_type(name, &json, &mut types);
                }
                oas3::spec::ObjectOrReference::Ref { ref_path } => {
                    if let Some(resolved) = resolve_schema_ref(&spec, ref_path) {
                        let json = serde_json::to_value(resolved).unwrap_or_default();
                        process_schema_type(name, &json, &mut types);
                    }
                }
            }
        }
    }
    Ok(types)
}
