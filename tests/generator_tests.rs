use brrtrouter::generator::{
    extract_fields, is_named_type, parameter_to_field, process_schema_type,
    rust_literal_for_example, schema_to_type, to_camel_case, FieldDef, TypeDefinition,
};
use brrtrouter::spec::{ParameterLocation, ParameterMeta};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn test_to_camel_case() {
    assert_eq!(to_camel_case("my_type"), "MyType");
    assert_eq!(to_camel_case("example"), "Example");
}

#[test]
fn test_is_named_type() {
    assert!(is_named_type("Foo"));
    assert!(!is_named_type("String"));
    assert!(is_named_type("Vec<Foo>"));
    assert!(!is_named_type("Vec<String>"));
}

#[test]
fn test_schema_to_type() {
    assert_eq!(schema_to_type(&json!({"type": "string"})), "String");
    assert_eq!(schema_to_type(&json!({"type": "integer"})), "i32");
    assert_eq!(
        schema_to_type(&json!({"type": "array", "items": {"type": "number"}})),
        "Vec<f64>"
    );
    assert_eq!(
        schema_to_type(&json!({"$ref": "#/components/schemas/item"})),
        "Item"
    );
    assert_eq!(
        schema_to_type(&json!({"type": "array", "items": {"$ref": "#/components/schemas/item"}})),
        "Vec<Item>"
    );
}

#[test]
fn test_extract_fields() {
    let schema = json!({
        "type": "object",
        "required": ["id"],
        "properties": {
            "id": {"type": "string"},
            "age": {"type": "integer"}
        }
    });
    let fields = extract_fields(&schema);
    assert_eq!(fields.len(), 2);
    let id = fields.iter().find(|f| f.name == "id").unwrap();
    assert_eq!(id.ty, "String");
    assert!(!id.optional);
    assert_eq!(id.value, "\"example\".to_string()");
    let age = fields.iter().find(|f| f.name == "age").unwrap();
    assert_eq!(age.ty, "i32");
    assert!(age.optional);
    assert_eq!(age.value, "Some(42)");
}

#[test]
fn test_process_schema_type_and_parameter_to_field() {
    let mut types: HashMap<String, TypeDefinition> = HashMap::new();
    let schema = json!({
        "type": "object",
        "properties": { "flag": {"type": "boolean"} }
    });
    process_schema_type("sample", &schema, &mut types);
    let ty = types.get("Sample").expect("type inserted");
    assert_eq!(ty.name, "Sample");
    assert_eq!(ty.fields.len(), 1);

    let param = ParameterMeta {
        name: "flag".to_string(),
        location: ParameterLocation::Query,
        required: false,
        schema: Some(json!({"type": "boolean"})),
        style: None,
        explode: None,
    };
    let field = parameter_to_field(&param);
    assert_eq!(field.name, "flag");
    assert_eq!(field.ty, "bool");
    assert!(field.optional);
    assert_eq!(field.value, "Some(true)");
}

#[test]
fn test_rust_literal_for_example() {
    let mut field = FieldDef {
        name: "count".to_string(),
        original_name: "count".to_string(),
        ty: "i32".to_string(),
        optional: false,
        value: "0".to_string(),
    };
    let lit = rust_literal_for_example(&field, &json!(3));
    assert_eq!(lit, "3");

    field.optional = true;
    field.ty = "String".to_string();
    let lit = rust_literal_for_example(&field, &json!("foo"));
    assert_eq!(lit, "Some(\"foo\".to_string())");
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
fn test_schema_to_type_complex() {
    assert_eq!(
        schema_to_type(&json!({"type": "object"})),
        "serde_json::Value"
    );
    let nested = json!({
        "type": "array",
        "items": {"type": "array", "items": {"type": "string"}}
    });
    assert_eq!(schema_to_type(&nested), "Vec<Vec<String>>");
}

#[test]
fn test_extract_fields_with_arrays_and_refs() {
    let schema = json!({
        "type": "object",
        "required": ["names", "pet"],
        "properties": {
            "names": {"type": "array", "items": {"type": "string"}},
            "pet": {"$ref": "#/components/schemas/pet"},
            "maybe": {"type": "integer"}
        }
    });
    let fields = extract_fields(&schema);
    assert_eq!(fields.len(), 3);
    let names = fields.iter().find(|f| f.name == "names").unwrap();
    assert_eq!(names.ty, "Vec<String>");
    assert!(!names.optional);
    assert_eq!(names.value, "vec![]");
    let pet = fields.iter().find(|f| f.name == "pet").unwrap();
    assert_eq!(pet.ty, "Pet");
    assert!(!pet.optional);
    assert_eq!(pet.value, "Default::default()");
    let maybe = fields.iter().find(|f| f.name == "maybe").unwrap();
    assert_eq!(maybe.ty, "i32");
    assert!(maybe.optional);
    assert_eq!(maybe.value, "Some(42)");
}

#[test]
fn test_parameter_to_field_variants() {
    let required = ParameterMeta {
        name: "id".to_string(),
        location: ParameterLocation::Path,
        required: true,
        schema: None,
        style: None,
        explode: None,
    };
    let f1 = parameter_to_field(&required);
    assert_eq!(f1.name, "id");
    assert_eq!(f1.ty, "String");
    assert!(!f1.optional);
    assert_eq!(f1.value, "\"example\".to_string()");

    let referenced = ParameterMeta {
        name: "pet".to_string(),
        location: ParameterLocation::Query,
        required: false,
        schema: Some(json!({"$ref": "#/components/schemas/pet"})),
        style: None,
        explode: None,
    };
    let f2 = parameter_to_field(&referenced);
    assert_eq!(f2.name, "pet");
    assert_eq!(f2.ty, "Pet");
    assert!(f2.optional);
    assert_eq!(f2.value, "Some(Default::default())");
}
