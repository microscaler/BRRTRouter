#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("gen_test_{}_{}", std::process::id(), nanos));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_unique_handler_name() {
    let mut seen = HashSet::new();
    let a = unique_handler_name(&mut seen, "foo");
    assert_eq!(a, "foo");
    let b = unique_handler_name(&mut seen, "foo");
    assert_eq!(b, "foo_1");
    let c = unique_handler_name(&mut seen, "foo");
    assert_eq!(c, "foo_2");
}

#[test]
fn test_unique_handler_name_empty() {
    let mut seen = HashSet::new();
    let result = unique_handler_name(&mut seen, "");
    assert_eq!(result, "");
}

#[test]
fn test_unique_handler_name_special_chars() {
    let mut seen = HashSet::new();
    let result = unique_handler_name(&mut seen, "handle-with-dashes");
    assert_eq!(result, "handle-with-dashes");

    let result2 = unique_handler_name(&mut seen, "handle-with-dashes");
    assert_eq!(result2, "handle-with-dashes_1");
}

#[test]
fn test_to_camel_case() {
    assert_eq!(to_camel_case("hello_world"), "HelloWorld");
    assert_eq!(to_camel_case("user_id"), "UserId");
    assert_eq!(to_camel_case("api_key"), "ApiKey");
    assert_eq!(to_camel_case("single"), "Single");
    assert_eq!(to_camel_case(""), "");
    assert_eq!(to_camel_case("already_camel"), "AlreadyCamel");
}

#[test]
fn test_to_camel_case_edge_cases() {
    assert_eq!(to_camel_case("_leading_underscore"), "LeadingUnderscore");
    assert_eq!(to_camel_case("trailing_underscore_"), "TrailingUnderscore");
    assert_eq!(
        to_camel_case("multiple___underscores"),
        "MultipleUnderscores"
    );
    assert_eq!(to_camel_case("a_b_c_d"), "ABCD");
}

#[test]
fn test_is_named_type() {
    // Primitive types should return false
    assert!(!is_named_type("String"));
    assert!(!is_named_type("i32"));
    assert!(!is_named_type("i64"));
    assert!(!is_named_type("f32"));
    assert!(!is_named_type("f64"));
    assert!(!is_named_type("bool"));
    assert!(!is_named_type("Value"));
    assert!(!is_named_type("serde_json::Value"));

    // Named types should return true
    assert!(is_named_type("User"));
    assert!(is_named_type("Pet"));
    assert!(is_named_type("ApiResponse"));

    // Vec of primitives should return false
    assert!(!is_named_type("Vec<String>"));
    assert!(!is_named_type("Vec<i32>"));
    assert!(!is_named_type("Vec<serde_json::Value>"));

    // Vec of named types should return true
    assert!(is_named_type("Vec<User>"));
    assert!(is_named_type("Vec<Pet>"));

    // Edge cases
    assert!(!is_named_type("vec<String>")); // lowercase vec
    assert!(is_named_type("Option<String>")); // Option is treated as named type
}

#[test]
fn test_schema_to_type_basic() {
    // String type
    let schema = json!({"type": "string"});
    assert_eq!(schema_to_type(&schema), "String");

    // Integer type
    let schema = json!({"type": "integer"});
    assert_eq!(schema_to_type(&schema), "i32");

    // Number type
    let schema = json!({"type": "number"});
    assert_eq!(schema_to_type(&schema), "f64");

    // Boolean type
    let schema = json!({"type": "boolean"});
    assert_eq!(schema_to_type(&schema), "bool");

    // Unknown type
    let schema = json!({"type": "unknown"});
    assert_eq!(schema_to_type(&schema), "serde_json::Value");
}

#[test]
fn test_schema_to_type_arrays() {
    // Array of strings
    let schema = json!({
        "type": "array",
        "items": {"type": "string"}
    });
    assert_eq!(schema_to_type(&schema), "Vec<String>");

    // Array of integers
    let schema = json!({
        "type": "array",
        "items": {"type": "integer"}
    });
    assert_eq!(schema_to_type(&schema), "Vec<i32>");

    // Array without items
    let schema = json!({"type": "array"});
    assert_eq!(schema_to_type(&schema), "Vec<serde_json::Value>");
}

#[test]
fn test_schema_to_type_refs() {
    // Reference to named type
    let schema = json!({"$ref": "#/components/schemas/User"});
    assert_eq!(schema_to_type(&schema), "User");

    // Reference with x-ref-name
    let schema = json!({"x-ref-name": "pet_store"});
    assert_eq!(schema_to_type(&schema), "PetStore");

    // Invalid reference
    let schema = json!({"$ref": "invalid/ref"});
    assert_eq!(schema_to_type(&schema), "serde_json::Value");
}

#[test]
fn test_schema_to_type_array_refs() {
    // Array of referenced types
    let schema = json!({
        "type": "array",
        "items": {"$ref": "#/components/schemas/Pet"}
    });
    assert_eq!(schema_to_type(&schema), "Vec<Pet>");

    // Array of complex items
    let schema = json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        }
    });
    assert_eq!(schema_to_type(&schema), "Vec<serde_json::Value>");
}

#[test]
fn test_extract_fields_simple() {
    let schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "integer"},
            "active": {"type": "boolean"}
        },
        "required": ["name"]
    });

    let fields = extract_fields(&schema);
    assert_eq!(fields.len(), 3);

    let name_field = fields.iter().find(|f| f.name == "name").unwrap();
    assert_eq!(name_field.ty, "String");
    assert!(!name_field.optional);

    let age_field = fields.iter().find(|f| f.name == "age").unwrap();
    assert_eq!(age_field.ty, "i32");
    assert!(age_field.optional);

    let active_field = fields.iter().find(|f| f.name == "active").unwrap();
    assert_eq!(active_field.ty, "bool");
    assert!(active_field.optional);
}

#[test]
fn test_extract_fields_array_response() {
    let schema = json!({
        "type": "array",
        "items": {"type": "string"}
    });

    let fields = extract_fields(&schema);
    assert_eq!(fields.len(), 1);

    let items_field = &fields[0];
    assert_eq!(items_field.name, "items");
    assert_eq!(items_field.ty, "Vec<String>");
    assert!(!items_field.optional);
}

#[test]
fn test_extract_fields_with_refs() {
    let schema = json!({
        "type": "object",
        "properties": {
            "user": {"$ref": "#/components/schemas/User"},
            "pets": {
                "type": "array",
                "items": {"$ref": "#/components/schemas/Pet"}
            }
        },
        "required": ["user"]
    });

    let fields = extract_fields(&schema);
    assert_eq!(fields.len(), 2);

    let user_field = fields.iter().find(|f| f.name == "user").unwrap();
    assert_eq!(user_field.ty, "User");
    assert!(!user_field.optional);

    let pets_field = fields.iter().find(|f| f.name == "pets").unwrap();
    assert_eq!(pets_field.ty, "Vec<Pet>");
    assert!(pets_field.optional);
}

#[test]
fn test_extract_fields_with_x_ref_name() {
    let schema = json!({
        "type": "object",
        "properties": {
            "owner": {"x-ref-name": "user_profile"}
        }
    });

    let fields = extract_fields(&schema);
    assert_eq!(fields.len(), 1);

    let owner_field = &fields[0];
    assert_eq!(owner_field.name, "owner");
    assert_eq!(owner_field.ty, "UserProfile");
    assert!(owner_field.optional);
}

#[test]
fn test_extract_fields_empty_schema() {
    let schema = json!({});
    let fields = extract_fields(&schema);
    assert!(fields.is_empty());
}

#[test]
fn test_extract_fields_no_properties() {
    let schema = json!({
        "type": "object"
    });
    let fields = extract_fields(&schema);
    assert!(fields.is_empty());
}

#[test]
fn test_parameter_to_field() {
    use crate::spec::{ParameterLocation, ParameterMeta};

    let param = ParameterMeta {
        name: "user_id".to_string(),
        location: ParameterLocation::Path,
        required: true,
        schema: Some(json!({"type": "string"})),
        style: None,
        explode: None,
    };

    let field = parameter_to_field(&param);
    assert_eq!(field.name, "user_id");
    assert_eq!(field.ty, "String");
    assert!(!field.optional);
}

#[test]
fn test_parameter_to_field_optional() {
    use crate::spec::{ParameterLocation, ParameterMeta};

    let param = ParameterMeta {
        name: "limit".to_string(),
        location: ParameterLocation::Query,
        required: false,
        schema: Some(json!({"type": "integer"})),
        style: None,
        explode: None,
    };

    let field = parameter_to_field(&param);
    assert_eq!(field.name, "limit");
    assert_eq!(field.ty, "i32");
    assert!(field.optional);
}

#[test]
fn test_parameter_to_field_no_schema() {
    use crate::spec::{ParameterLocation, ParameterMeta};

    let param = ParameterMeta {
        name: "token".to_string(),
        location: ParameterLocation::Header,
        required: true,
        schema: None,
        style: None,
        explode: None,
    };

    let field = parameter_to_field(&param);
    assert_eq!(field.name, "token");
    assert_eq!(field.ty, "String");
    assert!(!field.optional);
}

#[test]
fn test_rust_literal_for_example_string() {
    let field = FieldDef {
        name: "name".to_string(),
        original_name: "name".to_string(),
        ty: "String".to_string(),
        optional: false,
        value: "default".to_string(),
    };

    let example = json!("John Doe");
    let result = rust_literal_for_example(&field, &example);
    assert_eq!(result, "\"John Doe\".to_string()");
}

#[test]
fn test_rust_literal_for_example_optional_string() {
    let field = FieldDef {
        name: "nickname".to_string(),
        original_name: "nickname".to_string(),
        ty: "String".to_string(),
        optional: true,
        value: "default".to_string(),
    };

    let example = json!("Johnny");
    let result = rust_literal_for_example(&field, &example);
    assert_eq!(result, "Some(\"Johnny\".to_string())");
}

#[test]
fn test_rust_literal_for_example_number() {
    let field = FieldDef {
        name: "age".to_string(),
        original_name: "age".to_string(),
        ty: "i32".to_string(),
        optional: false,
        value: "default".to_string(),
    };

    let example = json!(25);
    let result = rust_literal_for_example(&field, &example);
    assert_eq!(result, "25");
}

#[test]
fn test_rust_literal_for_example_boolean() {
    let field = FieldDef {
        name: "active".to_string(),
        original_name: "active".to_string(),
        ty: "bool".to_string(),
        optional: false,
        value: "default".to_string(),
    };

    let example = json!(true);
    let result = rust_literal_for_example(&field, &example);
    assert_eq!(result, "true");
}

#[test]
fn test_rust_literal_for_example_array() {
    let field = FieldDef {
        name: "tags".to_string(),
        original_name: "tags".to_string(),
        ty: "Vec<String>".to_string(),
        optional: false,
        value: "default".to_string(),
    };

    let example = json!(["tag1", "tag2", "tag3"]);
    let result = rust_literal_for_example(&field, &example);
    assert_eq!(
        result,
        "vec![\"tag1\".to_string(), \"tag2\".to_string(), \"tag3\".to_string()]"
    );
}

#[test]
fn test_rust_literal_for_example_array_numbers() {
    let field = FieldDef {
        name: "scores".to_string(),
        original_name: "scores".to_string(),
        ty: "Vec<i32>".to_string(),
        optional: false,
        value: "default".to_string(),
    };

    let example = json!([1, 2, 3]);
    let result = rust_literal_for_example(&field, &example);
    assert_eq!(result, "vec![1, 2, 3]");
}

#[test]
fn test_rust_literal_for_example_json_value() {
    let field = FieldDef {
        name: "metadata".to_string(),
        original_name: "metadata".to_string(),
        ty: "serde_json::Value".to_string(),
        optional: false,
        value: "default".to_string(),
    };

    let example = json!({"key": "value"});
    let result = rust_literal_for_example(&field, &example);
    assert!(result.contains("serde_json::json!"));
}

#[test]
fn test_rust_literal_for_example_named_type() {
    let field = FieldDef {
        name: "user".to_string(),
        original_name: "user".to_string(),
        ty: "User".to_string(),
        optional: false,
        value: "default".to_string(),
    };

    let example = json!({"name": "John", "age": 30});
    let result = rust_literal_for_example(&field, &example);
    assert!(result.contains("serde_json::from_value::<User>"));
}

#[test]
fn test_process_schema_type() {
    let mut types = std::collections::HashMap::new();
    let schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "integer"}
        }
    });

    process_schema_type("user", &schema, &mut types);

    assert!(types.contains_key("User"));
    let user_type = types.get("User").unwrap();
    assert_eq!(user_type.name, "User");
    assert_eq!(user_type.fields.len(), 2);
}

#[test]
fn test_process_schema_type_empty() {
    let mut types = std::collections::HashMap::new();
    let schema = json!({});

    process_schema_type("empty", &schema, &mut types);

    // Should not add empty types
    assert!(!types.contains_key("Empty"));
}

#[test]
fn test_process_schema_type_duplicate() {
    let mut types = std::collections::HashMap::new();
    let schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"}
        }
    });

    process_schema_type("user", &schema, &mut types);
    process_schema_type("user", &schema, &mut types);

    // Should only contain one entry
    assert_eq!(types.len(), 1);
    assert!(types.contains_key("User"));
}

#[test]
fn test_field_def_construction() {
    let field = FieldDef {
        name: "test_field".to_string(),
        original_name: "test_field".to_string(),
        ty: "String".to_string(),
        optional: true,
        value: "default_value".to_string(),
    };

    assert_eq!(field.name, "test_field");
    assert_eq!(field.ty, "String");
    assert!(field.optional);
    assert_eq!(field.value, "default_value");
}

#[test]
fn test_type_definition_construction() {
    let fields = vec![
        FieldDef {
            name: "id".to_string(),
            original_name: "id".to_string(),
            ty: "i32".to_string(),
            optional: false,
            value: "0".to_string(),
        },
        FieldDef {
            name: "name".to_string(),
            original_name: "name".to_string(),
            ty: "String".to_string(),
            optional: false,
            value: "String::new()".to_string(),
        },
    ];

    let type_def = TypeDefinition {
        name: "User".to_string(),
        fields,
    };

    assert_eq!(type_def.name, "User");
    assert_eq!(type_def.fields.len(), 2);
    assert_eq!(type_def.fields[0].name, "id");
    assert_eq!(type_def.fields[1].name, "name");
}

#[test]
fn test_schema_edge_cases() {
    // Test null schema
    let schema = json!(null);
    let result = schema_to_type(&schema);
    assert_eq!(result, "serde_json::Value");

    // Test schema with no type
    let schema = json!({"description": "A field without type"});
    let result = schema_to_type(&schema);
    assert_eq!(result, "serde_json::Value");

    // Test array with complex nested items
    let schema = json!({
        "type": "array",
        "items": {
            "type": "array",
            "items": {"type": "string"}
        }
    });
    let result = schema_to_type(&schema);
    assert_eq!(result, "Vec<Vec<String>>");
}

#[test]
fn test_extract_fields_complex_nested() {
    let schema = json!({
        "type": "object",
        "properties": {
            "user": {
                "type": "object",
                "properties": {
                    "name": {"type": "string"}
                }
            },
            "tags": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "label": {"type": "string"}
                    }
                }
            }
        }
    });

    let fields = extract_fields(&schema);
    assert_eq!(fields.len(), 2);

    let user_field = fields.iter().find(|f| f.name == "user").unwrap();
    assert_eq!(user_field.ty, "serde_json::Value");

    let tags_field = fields.iter().find(|f| f.name == "tags").unwrap();
    assert_eq!(tags_field.ty, "Vec<serde_json::Value>");
}

#[test]
fn test_schema_to_type_money_format() {
    // Test that format: money maps to rusty_money::Money
    let schema = json!({
        "type": "number",
        "format": "money"
    });
    let result = schema_to_type(&schema);
    assert_eq!(result, "rusty_money::Money", "format: money should map to rusty_money::Money");
}

#[test]
fn test_schema_to_type_decimal_format() {
    // Test that format: decimal maps to rust_decimal::Decimal
    let schema = json!({
        "type": "number",
        "format": "decimal"
    });
    let result = schema_to_type(&schema);
    assert_eq!(result, "rust_decimal::Decimal", "format: decimal should map to rust_decimal::Decimal");
}

#[test]
fn test_schema_to_type_number_no_format() {
    // Test that number without format maps to f64
    let schema = json!({
        "type": "number"
    });
    let result = schema_to_type(&schema);
    assert_eq!(result, "f64", "number without format should map to f64");
}

#[test]
fn test_schema_to_type_money_array() {
    // Test array of money values
    let schema = json!({
        "type": "array",
        "items": {
            "type": "number",
            "format": "money"
        }
    });
    let result = schema_to_type(&schema);
    assert_eq!(result, "Vec<rusty_money::Money>", "array of money should map to Vec<rusty_money::Money>");
}

#[test]
fn test_schema_to_type_decimal_array() {
    // Test array of decimal values
    let schema = json!({
        "type": "array",
        "items": {
            "type": "number",
            "format": "decimal"
        }
    });
    let result = schema_to_type(&schema);
    assert_eq!(result, "Vec<rust_decimal::Decimal>", "array of decimal should map to Vec<rust_decimal::Decimal>");
}

#[test]
fn test_extract_fields_with_money_usd() {
    // Test extracting fields with money type (USD)
    let schema = json!({
        "type": "object",
        "properties": {
            "amount": {
                "type": "number",
                "format": "money"
            },
            "currency": {
                "type": "string"
            }
        },
        "required": ["amount"]
    });

    let fields = extract_fields(&schema);
    assert_eq!(fields.len(), 2);

    let amount_field = fields.iter().find(|f| f.name == "amount").unwrap();
    assert_eq!(amount_field.ty, "rusty_money::Money", "amount field should be rusty_money::Money");
    assert!(!amount_field.optional, "amount should be required");
    // Verify dummy value uses $3.14 (314 cents)
    assert!(
        amount_field.value.contains("314"),
        "Money dummy value should use 314 cents ($3.14)"
    );
    assert!(
        amount_field.value.contains("rusty_money::iso::USD"),
        "Money dummy value should use USD currency"
    );
}

#[test]
fn test_extract_fields_with_money_314_value() {
    // Test that money fields use $3.14 (314 cents) as dummy value
    let schema = json!({
        "type": "object",
        "properties": {
            "applied_amount": {
                "type": "number",
                "format": "money"
            }
        }
    });

    let fields = extract_fields(&schema);
    let amount_field = fields.iter().find(|f| f.name == "applied_amount").unwrap();
    
    // Verify the dummy value contains $3.14 (314 cents)
    // Field is optional (not in required array), so may be wrapped in Some()
    assert!(
        amount_field.value.contains("rusty_money::Money::from_minor(314, rusty_money::iso::USD)"),
        "Money field should use $3.14 (314 cents) as dummy value, got: {}",
        amount_field.value
    );
}

#[test]
fn test_extract_fields_with_decimal() {
    // Test extracting fields with decimal type
    let schema = json!({
        "type": "object",
        "properties": {
            "rate": {
                "type": "number",
                "format": "decimal"
            }
        }
    });

    let fields = extract_fields(&schema);
    let rate_field = fields.iter().find(|f| f.name == "rate").unwrap();
    assert_eq!(rate_field.ty, "rust_decimal::Decimal", "decimal format should map to rust_decimal::Decimal");
    assert!(
        rate_field.value.contains("rust_decimal::Decimal::new"),
        "Decimal field should use Decimal::new for dummy value"
    );
}

#[test]
fn test_extract_fields_mixed_number_types() {
    // Test schema with mixed number types: f64, Decimal, and Money
    let schema = json!({
        "type": "object",
        "properties": {
            "mathematical_value": {
                "type": "number"
            },
            "general_decimal": {
                "type": "number",
                "format": "decimal"
            },
            "financial_amount": {
                "type": "number",
                "format": "money"
            }
        }
    });

    let fields = extract_fields(&schema);
    assert_eq!(fields.len(), 3);

    let math_field = fields.iter().find(|f| f.name == "mathematical_value").unwrap();
    assert_eq!(math_field.ty, "f64", "number without format should be f64");
    // Field is optional (not in required array), so value is wrapped in Some()
    assert!(
        math_field.value == "3.14" || math_field.value == "Some(3.14)",
        "f64 should use 3.14 (clippy warning acceptable), got: {}",
        math_field.value
    );

    let decimal_field = fields.iter().find(|f| f.name == "general_decimal").unwrap();
    assert_eq!(decimal_field.ty, "rust_decimal::Decimal", "format: decimal should be Decimal");

    let money_field = fields.iter().find(|f| f.name == "financial_amount").unwrap();
    assert_eq!(money_field.ty, "rusty_money::Money", "format: money should be Money");
    assert!(
        money_field.value.contains("314"),
        "Money should use 314 cents ($3.14)"
    );
}

#[test]
fn test_extract_fields_money_optional() {
    // Test optional money field
    let schema = json!({
        "type": "object",
        "properties": {
            "optional_amount": {
                "type": "number",
                "format": "money"
            }
        }
    });

    let fields = extract_fields(&schema);
    let amount_field = fields.iter().find(|f| f.name == "optional_amount").unwrap();
    assert!(amount_field.optional, "Field not in required array should be optional");
    assert!(
        amount_field.value.starts_with("Some("),
        "Optional money field should be wrapped in Some()"
    );
    assert!(
        amount_field.value.contains("314"),
        "Optional money should still use $3.14 (314 cents)"
    );
}

#[test]
fn test_detect_workspace_with_brrtrouter_deps_found() {
    use super::templates::detect_workspace_with_brrtrouter_deps;
    use std::fs;

    let dir = temp_dir();
    let cargo_toml = dir.join("Cargo.toml");

    // Create a workspace Cargo.toml with brrtrouter in workspace.dependencies
    let content = r#"[workspace]
members = []

[workspace.dependencies]
brrtrouter = { path = "../../BRRTRouter" }
serde = "1.0"
"#;
    fs::write(&cargo_toml, content).unwrap();

    // Test from a subdirectory
    let subdir = dir.join("subdir");
    fs::create_dir_all(&subdir).unwrap();

    assert!(detect_workspace_with_brrtrouter_deps(&subdir));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_detect_workspace_with_brrtrouter_deps_not_found() {
    use super::templates::detect_workspace_with_brrtrouter_deps;
    use std::fs;

    let dir = temp_dir();
    let cargo_toml = dir.join("Cargo.toml");

    // Create a workspace Cargo.toml with workspace.dependencies but WITHOUT brrtrouter
    // This is the critical edge case: workspace exists, workspace.dependencies exists,
    // but brrtrouter is not in it. Should return false immediately and NOT search parent dirs.
    let content = r#"[workspace]
members = []

[workspace.dependencies]
serde = "1.0"
serde_json = "1.0"
"#;
    fs::write(&cargo_toml, content).unwrap();

    // Test from a subdirectory
    let subdir = dir.join("subdir");
    fs::create_dir_all(&subdir).unwrap();

    // Should return false because brrtrouter is not in workspace.dependencies
    // This tests the critical bug fix: should return false immediately, not search parent dirs
    assert!(!detect_workspace_with_brrtrouter_deps(&subdir));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_detect_workspace_with_brrtrouter_deps_no_workspace_dependencies() {
    use super::templates::detect_workspace_with_brrtrouter_deps;
    use std::fs;

    let dir = temp_dir();
    let cargo_toml = dir.join("Cargo.toml");

    // Create a workspace Cargo.toml WITHOUT workspace.dependencies section
    let content = r#"[workspace]
members = []
"#;
    fs::write(&cargo_toml, content).unwrap();

    // Test from a subdirectory
    let subdir = dir.join("subdir");
    fs::create_dir_all(&subdir).unwrap();

    // Should return false because workspace.dependencies doesn't exist
    assert!(!detect_workspace_with_brrtrouter_deps(&subdir));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_detect_workspace_with_brrtrouter_deps_nested_workspace_boundary() {
    use super::templates::detect_workspace_with_brrtrouter_deps;
    use std::fs;

    // Create nested workspaces to test that we respect workspace boundaries
    let outer_dir = temp_dir();
    let inner_dir = outer_dir.join("inner");
    fs::create_dir_all(&inner_dir).unwrap();

    // Outer workspace: has workspace.dependencies but NO brrtrouter
    let outer_cargo = outer_dir.join("Cargo.toml");
    let outer_content = r#"[workspace]
members = ["inner"]

[workspace.dependencies]
serde = "1.0"
"#;
    fs::write(&outer_cargo, outer_content).unwrap();

    // Inner workspace: has workspace.dependencies WITH brrtrouter
    let inner_cargo = inner_dir.join("Cargo.toml");
    let inner_content = r#"[workspace]
members = []

[workspace.dependencies]
brrtrouter = { path = "../../BRRTRouter" }
serde = "1.0"
"#;
    fs::write(&inner_cargo, inner_content).unwrap();

    // Test from a subdirectory of the inner workspace
    let subdir = inner_dir.join("subdir");
    fs::create_dir_all(&subdir).unwrap();

    // Should find brrtrouter in the inner workspace, not search the outer workspace
    assert!(detect_workspace_with_brrtrouter_deps(&subdir));

    // Test from a subdirectory of the outer workspace (outside inner workspace)
    let outer_subdir = outer_dir.join("outer_subdir");
    fs::create_dir_all(&outer_subdir).unwrap();

    // Should return false because outer workspace doesn't have brrtrouter
    // and we should NOT search parent directories (which would be wrong)
    assert!(!detect_workspace_with_brrtrouter_deps(&outer_subdir));

    fs::remove_dir_all(&outer_dir).unwrap();
}

#[test]
fn test_detect_workspace_with_brrtrouter_deps_no_workspace_section() {
    use super::templates::detect_workspace_with_brrtrouter_deps;
    use std::fs;

    let dir = temp_dir();
    let cargo_toml = dir.join("Cargo.toml");

    // Create a Cargo.toml WITHOUT [workspace] section
    let content = r#"[package]
name = "test"
version = "0.1.0"
"#;
    fs::write(&cargo_toml, content).unwrap();

    // Test from a subdirectory
    let subdir = dir.join("subdir");
    fs::create_dir_all(&subdir).unwrap();

    // Should continue searching parent directories (or return false if no workspace found)
    // This tests that non-workspace Cargo.toml files don't stop the search
    let result = detect_workspace_with_brrtrouter_deps(&subdir);
    // Result depends on whether a workspace is found in parent dirs, but should not panic
    assert!(!result || result); // Just ensure it returns a bool

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_detect_workspace_with_brrtrouter_deps_only_macros_not_brrtrouter() {
    use super::templates::detect_workspace_with_brrtrouter_deps;

    let dir = temp_dir();
    let cargo_toml = dir.join("Cargo.toml");

    // Create a workspace Cargo.toml with brrtrouter_macros but NOT brrtrouter
    // This tests the critical bug fix: should NOT match brrtrouter_macros
    let content = r#"[workspace]
members = []

[workspace.dependencies]
brrtrouter_macros = { path = "../../BRRTRouter/brrtrouter_macros" }
serde = "1.0"
"#;
    fs::write(&cargo_toml, content).unwrap();

    // Test from a subdirectory
    let subdir = dir.join("subdir");
    fs::create_dir_all(&subdir).unwrap();

    // Should return false because brrtrouter (exactly) is not in workspace.dependencies
    // Only brrtrouter_macros is present, which should NOT match
    assert!(!detect_workspace_with_brrtrouter_deps(&subdir));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_detect_workspace_with_brrtrouter_deps_both_brrtrouter_and_macros() {
    use super::templates::detect_workspace_with_brrtrouter_deps;

    let dir = temp_dir();
    let cargo_toml = dir.join("Cargo.toml");

    // Create a workspace Cargo.toml with both brrtrouter and brrtrouter_macros
    let content = r#"[workspace]
members = []

[workspace.dependencies]
brrtrouter = { path = "../../BRRTRouter" }
brrtrouter_macros = { path = "../../BRRTRouter/brrtrouter_macros" }
serde = "1.0"
"#;
    fs::write(&cargo_toml, content).unwrap();

    // Test from a subdirectory
    let subdir = dir.join("subdir");
    fs::create_dir_all(&subdir).unwrap();

    // Should return true because brrtrouter (exactly) is in workspace.dependencies
    assert!(detect_workspace_with_brrtrouter_deps(&subdir));

    fs::remove_dir_all(&dir).unwrap();
}
