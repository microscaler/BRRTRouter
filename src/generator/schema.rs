use crate::dummy_value;
use crate::spec::{resolve_schema_ref, ParameterMeta};
use oas3;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// A Rust type definition generated from an OpenAPI schema
///
/// Represents a struct that will be generated in the output code.
#[derive(Debug, Clone)]
pub struct TypeDefinition {
    /// The Rust struct name (e.g., `Pet`, `User`)
    pub name: String,
    /// The fields that make up this struct
    pub fields: Vec<FieldDef>,
}

/// A field definition for a generated Rust struct
///
/// Contains all information needed to generate a struct field including
/// its name, type, and whether it's optional.
#[derive(Debug, Clone)]
pub struct FieldDef {
    /// Sanitized Rust field name (e.g., `user_id`)
    pub name: String,
    /// Original field name from OpenAPI spec (for serde rename)
    pub original_name: String,
    /// Rust type (e.g., `String`, `i64`, `Vec<Pet>`)
    pub ty: String,
    /// Whether the field is optional (`Option<T>`)
    pub optional: bool,
    /// Example value as a Rust literal
    pub value: String,
}

/// Convert a snake_case string to CamelCase
///
/// Used for generating Rust struct names from OpenAPI schema names.
///
/// # Example
///
/// ```rust,ignore
/// assert_eq!(to_camel_case("user_profile"), "UserProfile");
/// ```
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

/// Check if a type string represents a named (custom) type vs a primitive
///
/// Returns `true` for custom types like `Pet`, `User`, `Vec<Pet>`.
/// Returns `false` for primitives like `String`, `i64`, `bool`.
///
/// Used to determine if a type needs to be imported or defined.
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

/// Sanitize a Rust identifier by escaping keywords (private helper)
///
/// Rust keywords like `type`, `self`, `fn` cannot be used as identifiers.
/// This function detects keywords and prefixes them with `r#` (raw identifier syntax).
///
/// # Arguments
///
/// * `name` - Identifier to sanitize
///
/// # Returns
///
/// Either the original name or `r#{name}` if it's a keyword
///
/// # Example
///
/// ```ignore
/// assert_eq!(sanitize_rust_identifier("type"), "r#type");
/// assert_eq!(sanitize_rust_identifier("user_id"), "user_id");
/// ```
#[allow(dead_code)]
fn sanitize_rust_identifier(name: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn",
    ];
    if KEYWORDS.contains(&name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

/// Sanitize a field name to be a valid Rust identifier (private helper)
///
/// Field names from OpenAPI specs may contain characters invalid in Rust (hyphens, dots, etc.).
/// This function:
/// 1. Converts CamelCase and kebab-case to snake_case
/// 2. Replaces invalid characters with underscores
/// 3. Ensures the name doesn't start with a digit
/// 4. Handles empty strings
///
/// # Arguments
///
/// * `name` - Field name from OpenAPI spec
///
/// # Returns
///
/// A valid Rust identifier in snake_case
///
/// # Example
///
/// ```ignore
/// assert_eq!(sanitize_field_name("user-id"), "user_id");
/// assert_eq!(sanitize_field_name("X-Trace-Id"), "x_trace_id");
/// assert_eq!(sanitize_field_name("UserId"), "user_id");
/// assert_eq!(sanitize_field_name("123field"), "_123field");
/// assert_eq!(sanitize_field_name(""), "_");
/// ```
fn sanitize_field_name(name: &str) -> String {
    // First, convert to snake_case by inserting underscores before uppercase letters
    // and lowercasing everything
    let mut result = String::with_capacity(name.len() + 4);
    let mut prev_was_upper = false;
    let mut prev_was_underscore = true; // Start as true to avoid leading underscore

    for c in name.chars() {
        if c.is_ascii_uppercase() {
            // Insert underscore before uppercase if previous wasn't underscore/uppercase
            if !prev_was_underscore && !prev_was_upper {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
            prev_was_upper = true;
            prev_was_underscore = false;
        } else if c.is_ascii_alphanumeric() {
            result.push(c);
            prev_was_upper = false;
            prev_was_underscore = false;
        } else {
            // Replace non-alphanumeric with underscore
            if !prev_was_underscore {
                result.push('_');
            }
            prev_was_upper = false;
            prev_was_underscore = true;
        }
    }

    // Handle edge cases
    if result.is_empty() {
        return "_".to_string();
    }

    // Remove trailing underscores
    while result.ends_with('_') && result.len() > 1 {
        result.pop();
    }

    // Ensure doesn't start with a digit
    if result
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        result.insert(0, '_');
    }

    result
}

/// Generate a unique handler name to avoid duplicates (internal helper)
///
/// Ensures handler names are unique by appending a counter if a duplicate is detected.
/// This prevents compilation errors when multiple operations have the same operation ID.
///
/// # Arguments
///
/// * `seen` - Mutable set of already-used handler names
/// * `name` - Desired handler name
///
/// # Returns
///
/// Either the original name (if unique) or `{name}_{counter}` (if duplicate)
///
/// # Example
///
/// ```ignore
/// let mut seen = HashSet::new();
/// assert_eq!(unique_handler_name(&mut seen, "get_user"), "get_user");
/// assert_eq!(unique_handler_name(&mut seen, "get_user"), "get_user_1");
/// assert_eq!(unique_handler_name(&mut seen, "get_user"), "get_user_2");
/// ```
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

/// Generate a Rust literal expression from an example value
///
/// Converts JSON example values into Rust code that can be used as default
/// values in generated structs.
///
/// # Complex Type Handling
///
/// This function handles multiple complex scenarios:
/// - **Strings**: Converted to `.to_string()` or `serde_json::Value::String(...)`
/// - **Numbers**: Used as-is (e.g., `42`, `3.14`)
/// - **Booleans**: Used as-is (`true`, `false`)
/// - **Arrays**: Recursively processed with special handling for:
///   - `Vec<String>` - each item becomes `"x".to_string()`
///   - `Vec<serde_json::Value>` - each item becomes `serde_json::Value::String(...)`
///   - `Vec<CustomType>` - attempts JSON deserialization with fallback to Default
/// - **Objects**: Attempts deserialization for named types, or uses `serde_json::json!(...)`
///
/// # Arguments
///
/// * `field` - The field definition (provides type context for correct conversion)
/// * `example` - The JSON example value from OpenAPI spec
///
/// # Returns
///
/// A Rust expression string (e.g., `"example".to_string()`, `42i64`, `vec![]`)
pub fn rust_literal_for_example(field: &FieldDef, example: &Value) -> String {
    let literal = match example {
        // Simple string conversion - check if target type is Value or String
        Value::String(s) => {
            if field.ty == "serde_json::Value" || field.ty == "Value" {
                // Target is serde_json::Value, wrap as Value::String
                format!("serde_json::Value::String({s:?}.to_string())")
            } else {
                // Target is Rust String, use .to_string()
                format!("{s:?}.to_string()")
            }
        }
        // Numbers need type-aware conversion
        Value::Number(n) => {
            // Check if field type is Decimal (for money/decimal formats)
            if field.ty.contains("rust_decimal::Decimal") || field.ty.contains("Decimal") {
                // Convert number to Decimal::new(mantissa, scale)
                // For example: 123.45 → Decimal::new(12345, 2)
                if let Some(f) = n.as_f64() {
                    // Parse as string to preserve precision, then convert to Decimal
                    let s = n.to_string();
                    if s.contains('.') {
                        let parts: Vec<&str> = s.split('.').collect();
                        let integer = parts[0].parse::<i64>().unwrap_or(0);
                        let decimal = parts.get(1).unwrap_or(&"0");
                        let scale = decimal.len() as u32;
                        let mantissa = format!("{}{}", integer, decimal)
                            .parse::<i64>()
                            .unwrap_or(0);
                        format!("rust_decimal::Decimal::new({}, {})", mantissa, scale)
                    } else {
                        // Integer: 123 → Decimal::new(123, 0)
                        let mantissa = n.as_i64().unwrap_or(0);
                        format!("rust_decimal::Decimal::new({}, 0)", mantissa)
                    }
                } else {
                    // Fallback for other number types
                    let mantissa = n.as_i64().unwrap_or(0);
                    format!("rust_decimal::Decimal::new({}, 0)", mantissa)
                }
            }
            // If field type is f64 but number is integer, add .0 to make it a float literal
            else if field.ty == "f64" || field.ty == "Option<f64>" {
                if let Some(i) = n.as_i64() {
                    format!("{}.0", i)
                } else if let Some(u) = n.as_u64() {
                    format!("{}.0", u)
                } else {
                    n.to_string()
                }
            } else {
                n.to_string()
            }
        }
        Value::Bool(b) => b.to_string(),
        // Arrays require complex processing based on element type
        Value::Array(items) => {
            // Extract the inner type from Vec<T> - e.g., "String" from "Vec<String>"
            let inner_ty_opt = field
                .ty
                .strip_prefix("Vec<")
                .and_then(|s| s.strip_suffix(">"));
            // Determine what kind of vec we're generating
            let is_vec_string = inner_ty_opt == Some("String");
            let is_vec_json_value =
                inner_ty_opt == Some("serde_json::Value") || inner_ty_opt == Some("Value");
            // Process each array element - type conversion depends on target Vec<T> type
            let inner = items
                .iter()
                .map(|item| match item {
                    Value::String(s) => {
                        if is_vec_string {
                            // Vec<String>: simple .to_string() conversion
                            format!("{s:?}.to_string()")
                        } else if is_vec_json_value {
                            // Vec<Value>: wrap in serde_json::Value::String
                            format!("serde_json::Value::String({s:?}.to_string())")
                        } else {
                            // Other types: try parsing from string (e.g., Vec<i32>)
                            // Use unwrap_or_default to avoid panics if parsing fails
                            format!("{s:?}.to_string().parse().unwrap_or_default()")
                        }
                    }
                    // Numbers need type-aware conversion in arrays
                    Value::Number(n) => {
                        // Check if array element type is Decimal
                        if let Some(inner_ty) = inner_ty_opt {
                            if inner_ty.contains("rust_decimal::Decimal") || inner_ty.contains("Decimal") {
                                // Convert number to Decimal::new(mantissa, scale)
                                let s = n.to_string();
                                if s.contains('.') {
                                    let parts: Vec<&str> = s.split('.').collect();
                                    let integer = parts[0].parse::<i64>().unwrap_or(0);
                                    let decimal = parts.get(1).unwrap_or(&"0");
                                    let scale = decimal.len() as u32;
                                    let mantissa = format!("{}{}", integer, decimal).parse::<i64>().unwrap_or(0);
                                    format!("rust_decimal::Decimal::new({}, {})", mantissa, scale)
                                } else {
                                    let mantissa = n.as_i64().unwrap_or(0);
                                    format!("rust_decimal::Decimal::new({}, 0)", mantissa)
                                }
                            }
                            // If array element type is f64 but number is integer, add .0
                            else if inner_ty == "f64" {
                                if let Some(i) = n.as_i64() {
                                    format!("{}.0", i)
                                } else if let Some(u) = n.as_u64() {
                                    format!("{}.0", u)
                                } else {
                                    n.to_string()
                                }
                            } else {
                                n.to_string()
                            }
                        } else {
                            n.to_string()
                        }
                    },
                    Value::Bool(b) => b.to_string(),
                    // Object items require deserialization or dummy values
                    Value::Object(_) => {
                        if let Some(inner_ty) = inner_ty_opt {
                            if inner_ty == "serde_json::Value" || inner_ty == "Value" {
                                // Target is Vec<Value>, use json! macro
                                let json = serde_json::to_string(item).unwrap_or_else(|_| "null".to_string());
                                format!("serde_json::json!({json})")
                            } else if is_named_type(inner_ty) {
                                // Target is Vec<CustomType>, deserialize with fallback
                                let json = serde_json::to_string(item).unwrap_or_else(|_| "null".to_string());
                                format!(
                                    "serde_json::from_value::<{inner_ty}>(serde_json::json!({json})).unwrap_or_default()"
                                )
                            } else {
                                // Use dummy value generator for primitives
                                dummy_value::dummy_value(inner_ty).unwrap_or_else(|_| "Default::default()".to_string())
                            }
                        } else {
                            // No type info, fallback to json!
                            let json = serde_json::to_string(item).unwrap_or_else(|_| "null".to_string());
                            format!("serde_json::json!({json})")
                        }
                    }
                    // Other types (null, etc.) - use dummy or Default
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
            // Wrap all elements in vec![] macro
            format!("vec![{inner}]")
        }
        Value::Object(_) => {
            let json = serde_json::to_string(example).unwrap_or_else(|_| "null".to_string());
            if field.ty == "serde_json::Value" || field.ty == "Value" {
                format!("serde_json::json!({json})")
            } else if is_named_type(&field.ty) {
                format!(
                    "serde_json::from_value::<{}>(serde_json::json!({json})).unwrap_or_default()",
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

/// Process an OpenAPI schema and generate a Rust type definition
///
/// Extracts fields from the schema and adds the resulting type to the types map.
/// Skips schemas that don't define any fields or are already processed.
/// Recursively processes all referenced types to ensure they're included.
///
/// # Arguments
///
/// * `name` - Schema name from OpenAPI spec
/// * `schema` - JSON Schema definition
/// * `types` - Mutable map of generated types (updated in-place)
/// * `spec` - Optional OpenAPI spec for resolving $ref references
pub fn process_schema_type(
    name: &str,
    schema: &Value,
    types: &mut HashMap<String, TypeDefinition>,
) {
    process_schema_type_with_spec(name, schema, types, None);
}

/// Process an OpenAPI schema with spec context for resolving $ref
///
/// This version can resolve $ref references to ensure all referenced types are collected.
pub fn process_schema_type_with_spec(
    name: &str,
    schema: &Value,
    types: &mut HashMap<String, TypeDefinition>,
    spec: Option<&oas3::OpenApiV3Spec>,
) {
    let name = to_camel_case(name);
    if types.contains_key(&name) {
        return;
    }

    // First, recursively collect all referenced types from this schema
    if let Some(spec_ref) = spec {
        collect_referenced_types(schema, spec_ref, types);
    }

    let fields = extract_fields(schema);
    if !fields.is_empty() {
        types.insert(name.clone(), TypeDefinition { name, fields });
    }
}

/// Recursively collect all types referenced via $ref in a schema
///
/// This ensures that when a schema references another type (e.g., Account, AccountRole),
/// that referenced type is also added to the types map.
fn collect_referenced_types(
    schema: &Value,
    spec: &oas3::OpenApiV3Spec,
    types: &mut HashMap<String, TypeDefinition>,
) {
    // Check if this schema itself is a $ref
    if let Some(ref_path) = schema.get("$ref").and_then(|v| v.as_str()) {
        if let Some(schema_name) = ref_path.strip_prefix("#/components/schemas/") {
            let camel_name = to_camel_case(schema_name);
            if !types.contains_key(&camel_name) {
                // Resolve the referenced schema and process it
                if let Some(components) = spec.components.as_ref() {
                    if let Some(schema_obj) = components.schemas.get(schema_name) {
                        match schema_obj {
                            oas3::spec::ObjectOrReference::Object(obj) => {
                                let json = serde_json::to_value(obj).unwrap_or_default();
                                process_schema_type_with_spec(
                                    schema_name,
                                    &json,
                                    types,
                                    Some(spec),
                                );
                            }
                            oas3::spec::ObjectOrReference::Ref {
                                ref_path: nested_ref,
                                ..
                            } => {
                                if let Some(resolved) = resolve_schema_ref(spec, nested_ref) {
                                    let json = serde_json::to_value(resolved).unwrap_or_default();
                                    process_schema_type_with_spec(
                                        schema_name,
                                        &json,
                                        types,
                                        Some(spec),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Recursively check properties for $ref
    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        for (_prop_name, prop_schema) in props {
            collect_referenced_types(prop_schema, spec, types);
        }
    }

    // Check items for arrays
    if let Some(items) = schema.get("items") {
        collect_referenced_types(items, spec, types);
    }

    // Check oneOf variants
    if let Some(one_of) = schema.get("oneOf").and_then(|v| v.as_array()) {
        for variant in one_of {
            collect_referenced_types(variant, spec, types);
        }
    }

    // Check allOf variants
    if let Some(all_of) = schema.get("allOf").and_then(|v| v.as_array()) {
        for variant in all_of {
            collect_referenced_types(variant, spec, types);
        }
    }
}

/// Extract field definitions from an OpenAPI/JSON Schema
///
/// Parses the schema's `properties` and generates Rust field definitions with
/// appropriate types, handling arrays, objects, primitives, and nested types.
///
/// # Complex Logic Explained
///
/// This function handles several intricate scenarios:
///
/// ## 1. Array Schema (Special Case)
/// If the schema itself is an array type, we return a single field named "items"
/// with type `Vec<T>`. This is used for response types that return arrays directly.
///
/// ## 2. Required Fields Extraction
/// The OpenAPI `required` array is parsed to determine which fields are mandatory.
/// This affects whether we generate `Option<T>` or `T` for each field.
///
/// ## 3. oneOf with Null Handling (Most Complex!)
/// OpenAPI's `oneOf: [{type: null}, {type: T}]` pattern indicates optional fields.
/// We detect this pattern and:
/// - Extract the non-null type as `inner_ty`
/// - Set `nullable_oneof = true` to wrap the type in `Option<T>` later
/// - Fallback to `serde_json::Value` if we can't determine the inner type
///
/// ## 4. Type Resolution Priority
/// For each property, we resolve the Rust type in this order:
/// 1. oneOf inferred type (if present)
/// 2. x-ref-name extension (explicit type hint)
/// 3. $ref pointer to schema component
/// 4. Inline type definition (string, integer, array, etc.)
/// 5. Fallback to `serde_json::Value`
///
/// # Arguments
///
/// * `schema` - JSON Schema definition
///
/// # Returns
///
/// A vector of field definitions that can be used to generate a Rust struct
pub fn extract_fields(schema: &Value) -> Vec<FieldDef> {
    let mut fields = vec![];

    // Special case: if schema is itself an array, return a single "items" field
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

    // Extract the list of required field names from the schema
    // This is used to determine if fields should be Option<T> or T
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

    // Process each property in the schema
    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        for (name, prop) in props {
            // COMPLEX: Detect oneOf with null pattern: oneOf: [{type: null}, {type: T}]
            // This indicates an optional field in OpenAPI 3.1 style
            let (inferred_ty, nullable_oneof) =
                if let Some(one_of) = prop.get("oneOf").and_then(|v| v.as_array()) {
                    let mut inner_ty: Option<String> = None;
                    let mut has_null = false;
                    // Scan all oneOf variants to find the null and non-null types
                    for variant in one_of {
                        if variant.get("type").and_then(|t| t.as_str()) == Some("null") {
                            has_null = true;
                        } else {
                            // This is the actual type (not null)
                            inner_ty = Some(schema_to_type(variant));
                        }
                    }
                    (
                        // Return the inner type, or fallback to Value if unclear
                        inner_ty.unwrap_or_else(|| "serde_json::Value".to_string()),
                        has_null, // true if we found a null variant
                    )
                } else {
                    // No oneOf present, use empty string to signal fallback to regular type detection
                    (String::new(), false)
                };

            // Resolve the Rust type for this field using priority chain
            let ty = if !inferred_ty.is_empty() {
                // Priority 1: Use the type inferred from oneOf (if we detected one)
                inferred_ty
            } else if let Some(name) = prop.get("x-ref-name").and_then(|v| v.as_str()) {
                // Priority 2: Use explicit x-ref-name extension (custom type hint)
                to_camel_case(name)
            } else if let Some(r) = prop.get("$ref").and_then(|v| v.as_str()) {
                // Priority 3: Resolve $ref pointer to schema component
                if let Some(name) = r.strip_prefix("#/components/schemas/") {
                    to_camel_case(name) // Convert schema name to Rust type name
                } else {
                    "serde_json::Value".to_string() // Invalid $ref, fallback
                }
            } else {
                // Priority 4: Use inline type definition
                match prop.get("type").and_then(|t| t.as_str()) {
                    Some("string") => "String".to_string(),
                    Some("integer") => "i32".to_string(),
                    Some("number") => {
                        // Format-based type differentiation for number types
                        // number (no format) → f64 (mathematical numbers)
                        // number format:decimal → rust_decimal::Decimal (general decimals)
                        // number format:money → rusty_money::Money (financial amounts)
                        match prop.get("format").and_then(|f| f.as_str()) {
                            // For API serialization, use Decimal for money (Money has lifetime parameter incompatible with owned Deserialize)
                            // Money types should be used in entities, converted from Decimal in business logic
                            Some("money") => "rust_decimal::Decimal".to_string(),
                            Some("decimal") => "rust_decimal::Decimal".to_string(),
                            _ => "f64".to_string(), // Default to f64 for mathematical numbers
                        }
                    }
                    Some("boolean") => "bool".to_string(),
                    Some("array") => {
                        if let Some(items) = prop.get("items") {
                            // Recursively determine array element type
                            format!("Vec<{}>", schema_to_type(items))
                        } else {
                            // No items schema, use Value
                            "Vec<serde_json::Value>".to_string()
                        }
                    }
                    // Priority 5: Fallback for unknown or missing types
                    _ => "serde_json::Value".to_string(),
                }
            };

            // Determine if field is optional:
            // - Not in required array, OR
            // - Has oneOf with null variant
            let optional = !required.contains(name) || nullable_oneof;

            // Generate a dummy value for this field
            // If optional, wrap in Some(...), otherwise use value directly
            let value = dummy_value::dummy_value(&ty)
                .map(|v| if optional { format!("Some({v})") } else { v })
                .unwrap_or_else(|_| "Default::default()".to_string());

            // Create the field definition with sanitized name and original name for serde
            // Sanitize the field name and escape Rust keywords
            let sanitized = sanitize_field_name(name);
            let rust_safe_name = sanitize_rust_identifier(&sanitized);
            fields.push(FieldDef {
                name: rust_safe_name,        // Rust-safe identifier (escapes keywords)
                original_name: name.clone(), // Original JSON name for #[serde(rename)]
                ty,
                optional,
                value,
            });
        }
    }
    fields
}

/// Convert a JSON Schema to a Rust type string
///
/// Maps OpenAPI/JSON Schema types to their Rust equivalents:
/// - `string` → `String`
/// - `integer` → `i32`
/// - `number` → `f64`
/// - `boolean` → `bool`
/// - `array` → `Vec<T>`
/// - `$ref` → Named type (e.g., `Pet`, `User`)
/// - default → `serde_json::Value`
///
/// # Arguments
///
/// * `schema` - JSON Schema definition
///
/// # Returns
///
/// A Rust type string (e.g., `String`, `Vec<Pet>`, `Option<i64>`)
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
        Some("number") => {
            // Format-based type differentiation for number types
            // number (no format) → f64 (mathematical numbers)
            // number format:decimal → rust_decimal::Decimal (general decimals)
            // number format:money → rusty_money::Money (financial amounts)
            match schema.get("format").and_then(|f| f.as_str()) {
                // For API serialization, use Decimal for money (Money has lifetime parameter incompatible with owned Deserialize)
                // Money types should be used in entities, converted from Decimal in business logic
                Some("money") => "rust_decimal::Decimal".to_string(),
                Some("decimal") => "rust_decimal::Decimal".to_string(),
                _ => "f64".to_string(), // Default to f64 for mathematical numbers
            }
        }
        Some("boolean") => "bool".to_string(),
        Some("array") => {
            if let Some(items) = schema.get("items") {
                if let Some(item_ty) = items.get("type").and_then(|v| v.as_str()) {
                    let inner = match item_ty {
                        "string" => "String".to_string(),
                        "integer" => "i32".to_string(),
                        "number" => {
                            // Check format for array items too
                            match items.get("format").and_then(|f| f.as_str()) {
                                // For API serialization, use Decimal for money (Money has lifetime parameter incompatible with owned Deserialize)
                                Some("money") => "rust_decimal::Decimal".to_string(),
                                Some("decimal") => "rust_decimal::Decimal".to_string(),
                                _ => "f64".to_string(),
                            }
                        }
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

/// Convert an OpenAPI parameter to a field definition
///
/// Extracts type information from the parameter's schema and creates
/// a field definition suitable for code generation.
///
/// # Arguments
///
/// * `param` - Parameter metadata from OpenAPI spec
///
/// # Returns
///
/// A field definition with the parameter's name, type, and a default value
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

/// Collect all component schemas from an OpenAPI specification
///
/// Parses the spec file and extracts all schema definitions from `components.schemas`,
/// converting them to Rust type definitions. Resolves all `$ref` references and
/// processes nested schemas recursively.
///
/// # Arguments
///
/// * `spec_path` - Path to the OpenAPI specification file
///
/// # Returns
///
/// A map of type names to their definitions
///
/// # Errors
///
/// Returns an error if the spec file cannot be read or parsed.
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
                    // Pass spec context to recursively collect referenced types
                    process_schema_type_with_spec(name, &json, &mut types, Some(&spec));
                }
                oas3::spec::ObjectOrReference::Ref { ref_path, .. } => {
                    if let Some(resolved) = resolve_schema_ref(&spec, ref_path) {
                        let json = serde_json::to_value(resolved).unwrap_or_default();
                        // Pass spec context to recursively collect referenced types
                        process_schema_type_with_spec(name, &json, &mut types, Some(&spec));
                    }
                }
            }
        }
    }
    Ok(types)
}
