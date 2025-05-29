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
