use crate::spec::{resolve_schema_ref, ParameterMeta};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct TypeDefinition {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub original_schema: Option<Value>, // Store the original OpenAPI schema
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub ty: String,
    pub optional: bool,
    pub value: String,
    pub documentation: Option<String>,
    pub validation_attrs: Option<String>,
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
        Value::String(s) => format!("{s:?}.to_string()"),
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
                                let json = serde_json::to_string(item).unwrap();
                                format!("serde_json::json!({json})")
                            } else if is_named_type(inner_ty) {
                                let json = serde_json::to_string(item).unwrap();
                                format!("serde_json::from_value::<{inner_ty}>(serde_json::json!({json})).unwrap()")
                            } else {
                                "Default::default()".to_string()
                            }
                        } else {
                            "Default::default()".to_string()
                        }
                    }
                    _ => "Default::default()".to_string(),
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("vec![{inner}]")
        }
        Value::Object(_) => {
            let json = serde_json::to_string(example).unwrap();
            if field.ty == "serde_json::Value" || field.ty == "Value" {
                format!("serde_json::json!({json})")
            } else if is_named_type(&field.ty) {
                format!(
                    "serde_json::from_value::<{}>(serde_json::json!({json})).unwrap()",
                    field.ty
                )
            } else {
                format!("serde_json::json!({json})")
            }
        }
        _ => "Default::default()".to_string(),
    };
    if field.optional {
        format!("Some({literal})")
    } else {
        literal
    }
}

/// Convert a primitive example value from OpenAPI to Rust literal
pub fn rust_literal_for_primitive_example(ty: &str, example: &Value) -> String {
    match example {
        Value::String(s) => {
            match ty {
                "String" => format!("{s:?}.to_string()"),
                _ => format!("{s:?}.to_string()"),
            }
        }
        Value::Number(n) => {
            match ty {
                "i32" => n.as_i64().unwrap_or(0).to_string(),
                "i64" => n.as_i64().unwrap_or(0).to_string(),
                "f32" => n.as_f64().unwrap_or(0.0).to_string(),
                "f64" => n.as_f64().unwrap_or(0.0).to_string(),
                _ => n.to_string(),
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Array(arr) => {
            // Handle array examples 
            let inner_ty = ty.strip_prefix("Vec<").and_then(|s| s.strip_suffix(">")).unwrap_or("String");
            let items = arr.iter()
                .map(|item| rust_literal_for_primitive_example(inner_ty, item))
                .collect::<Vec<_>>()
                .join(", ");
            format!("vec![{items}]")
        }
        Value::Object(_) => {
            // For complex objects, use serde_json serialization
            let json_str = serde_json::to_string(example).unwrap_or_default();
            if ty.starts_with("Vec<") {
                format!("vec![serde_json::from_str({json_str:?}).unwrap()]")
            } else if ty == "serde_json::Value" {
                format!("serde_json::json!({json_str})")
            } else {
                format!("serde_json::from_str({json_str:?}).unwrap()")
            }
        }
        _ => "Default::default()".to_string(),
    }
}

/// Convert an array example from OpenAPI to Rust Vec literal
pub fn rust_literal_for_array_example(vec_ty: &str, example: &Value) -> String {
    match example {
        Value::Array(arr) => {
            let inner_ty = vec_ty.strip_prefix("Vec<").and_then(|s| s.strip_suffix(">")).unwrap_or("String");
            let items = arr.iter()
                .map(|item| rust_literal_for_primitive_example(inner_ty, item))
                .collect::<Vec<_>>()
                .join(", ");
            format!("vec![{items}]")
        }
        _ => "vec![Default::default()]".to_string(),
    }
}

/// Fallback to dummy value when no OpenAPI example is available
pub fn fallback_to_dummy_value(ty: &str, optional: bool) -> String {
    let base_value = match ty {
        "String" => "String::from(\"dummy_string\")".to_string(),
        "i32" => "42".to_string(),
        "i64" => "42".to_string(),
        "f32" => "42.0".to_string(),
        "f64" => "42.0".to_string(),
        "bool" => "true".to_string(),
        "serde_json::Value" => "serde_json::json!({})".to_string(),
        _ => "Default::default()".to_string(),
    };
    
    if optional {
        format!("Some({base_value})")
    } else {
        base_value
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
        types.insert(name.clone(), TypeDefinition { name, fields, original_schema: Some(schema.clone()) });
    }
}

pub fn extract_fields(schema: &Value) -> Vec<FieldDef> {
    let mut fields = vec![];
    if let Some(schema_type) = schema.get("type").and_then(|t| t.as_str()) {
        if schema_type == "array" {
            if let Some(items) = schema.get("items") {
                let ty = schema_to_type(items);
                
                // Try to get example from the array schema or items
                let value = if let Some(example) = schema.get("example") {
                    // Use the schema-level example
                    rust_literal_for_array_example(&format!("Vec<{ty}>"), example)
                } else if let Some(items_example) = items.get("example") {
                    // Use the items example to create a single-item array
                    format!("vec![{}]", rust_literal_for_primitive_example(&ty, items_example))
                } else {
                    "vec![Default::default()]".to_string()
                };
                
                fields.push(FieldDef {
                    name: "items".to_string(),
                    ty: format!("Vec<{ty}>"),
                    optional: false,
                    value,
                    documentation: None,
                    validation_attrs: None,
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
            let ty = if let Some(name) = prop.get("x-ref-name").and_then(|v| v.as_str()) {
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
            let optional = !required.contains(name);
            
            // CRITICAL FIX: Use OpenAPI schema examples when available
            let value = if let Some(example) = prop.get("example") {
                // Use the actual example from the OpenAPI spec
                let literal = rust_literal_for_primitive_example(&ty, example);
                if optional {
                    format!("Some({literal})")
                } else {
                    literal
                }
            } else if let Some(enum_values) = prop.get("enum").and_then(|v| v.as_array()) {
                // Use the first enum value as the example
                if let Some(first_enum) = enum_values.first() {
                    let literal = rust_literal_for_primitive_example(&ty, first_enum);
                    if optional {
                        format!("Some({literal})")
                    } else {
                        literal
                    }
                } else {
                    fallback_to_dummy_value(&ty, optional)
                }
            } else if let Some(default_val) = prop.get("default") {
                // Use the default value from the schema
                let literal = rust_literal_for_primitive_example(&ty, default_val);
                if optional {
                    format!("Some({literal})")
                } else {
                    literal
                }
            } else {
                // Fall back to dummy value as last resort
                fallback_to_dummy_value(&ty, optional)
            };
            
            fields.push(FieldDef {
                name: name.clone(),
                ty,
                optional,
                value,
                documentation: None,
                validation_attrs: None,
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
    let value = fallback_to_dummy_value(&ty, optional);
    FieldDef {
        name: param.name.clone(),
        ty,
        optional,
        value,
        documentation: None,
        validation_attrs: None,
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

/// Build a complete example object from an OpenAPI schema with all required fields
pub fn build_complete_example_object(schema: &Value) -> Value {
    println!("🔍 DEBUG: Building complete example object from schema: {}", serde_json::to_string_pretty(schema).unwrap_or("invalid".to_string()));
    
    let mut example_obj = serde_json::Map::new();
    
    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect::<std::collections::HashSet<_>>()
        })
        .unwrap_or_default();
    
    println!("🔍 DEBUG: Required fields: {:?}", required);
    
    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        println!("🔍 DEBUG: Found {} properties", props.len());
        for (name, prop) in props {
            let is_required = required.contains(name);
            println!("🔍 DEBUG: Processing field '{}' (required: {})", name, is_required);
            
            // Extract example value for this property
            let example_value = if let Some(example) = prop.get("example") {
                println!("🔍 DEBUG: Using example: {}", example);
                example.clone()
            } else if let Some(enum_values) = prop.get("enum").and_then(|v| v.as_array()) {
                // Use first enum value
                let first_enum = enum_values.first().cloned().unwrap_or_else(|| Value::String("unknown".to_string()));
                println!("🔍 DEBUG: Using enum value: {}", first_enum);
                first_enum
            } else if let Some(default_val) = prop.get("default") {
                println!("🔍 DEBUG: Using default: {}", default_val);
                default_val.clone()
            } else {
                // Generate a sensible default based on type
                let fallback = match prop.get("type").and_then(|t| t.as_str()) {
                    Some("string") => {
                        if let Some(format) = prop.get("format").and_then(|f| f.as_str()) {
                            match format {
                                "email" => Value::String("user@example.com".to_string()),
                                "date-time" => Value::String("2023-01-01T00:00:00Z".to_string()),
                                "uuid" => Value::String("550e8400-e29b-41d4-a716-446655440000".to_string()),
                                "uri" => Value::String("https://example.com".to_string()),
                                _ => Value::String("example".to_string()),
                            }
                        } else {
                            Value::String("example".to_string())
                        }
                    }
                    Some("integer") => Value::Number(serde_json::Number::from(42)),
                    Some("number") => Value::Number(serde_json::Number::from_f64(42.0).unwrap()),
                    Some("boolean") => Value::Bool(true),
                    Some("array") => Value::Array(vec![]),
                    _ => Value::String("unknown".to_string()),
                };
                println!("🔍 DEBUG: Using fallback: {}", fallback);
                fallback
            };
            
            // Include required fields and optional fields that have examples
            if is_required || prop.get("example").is_some() || prop.get("default").is_some() {
                example_obj.insert(name.clone(), example_value);
                println!("🔍 DEBUG: Added field '{}' to example object", name);
            } else {
                println!("🔍 DEBUG: Skipped optional field '{}' (no example)", name);
            }
        }
    } else {
        println!("🔍 DEBUG: No properties found in schema");
    }
    
    let result = Value::Object(example_obj);
    println!("🔍 DEBUG: Final example object: {}", serde_json::to_string_pretty(&result).unwrap_or("invalid".to_string()));
    result
}

pub fn debug_user_schema() {
    let user_schema_json = r#"
    {
      "type": "object",
      "required": ["id", "name", "email"],
      "properties": {
        "id": {
          "type": "string",
          "format": "uuid",
          "example": "abc-123"
        },
        "name": {
          "type": "string",
          "minLength": 1,
          "maxLength": 100,
          "example": "John"
        },
        "email": {
          "type": "string",
          "format": "email",
          "example": "john@example.com"
        },
        "phone": {
          "type": "string",
          "pattern": "^\\+?[1-9]\\d{1,14}$",
          "example": "+1-555-123-4567"
        }
      }
    }
    "#;
    
    println!("🧪 DEBUG: Testing User schema processing...");
    let schema: serde_json::Value = serde_json::from_str(user_schema_json).unwrap();
    let result = build_complete_example_object(&schema);
    println!("🧪 DEBUG: Result: {}", serde_json::to_string_pretty(&result).unwrap());
}
