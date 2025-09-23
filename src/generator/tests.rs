use super::*;
use serde_json::json;
use std::collections::HashSet;

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
