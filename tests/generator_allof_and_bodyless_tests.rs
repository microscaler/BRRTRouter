//! Regression tests: `allOf` response schemas generate their merged fields, and a body-less 2xx response does not
//! fall back to an error response's schema.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use brrtrouter::generator::extract_fields;
use brrtrouter::spec::extract_response_schema_and_example;
use oas3::OpenApiV3Spec;
use serde_json::json;

const SPEC: &str = r#"
openapi: 3.1.0
info:
  title: Generic list API
  version: '1.0'
components:
  schemas:
    Pagination:
      type: object
      required: [total, page]
      properties:
        total: { type: integer }
        page: { type: integer }
        next_cursor: { type: string }
    Error:
      type: object
      required: [code, message]
      properties:
        code: { type: string }
        message: { type: string }
paths:
  /widgets:
    get:
      operationId: list_widgets
      responses:
        '200':
          description: A page of widgets
          content:
            application/json:
              schema:
                allOf:
                  - $ref: '#/components/schemas/Pagination'
                  - type: object
                    required: [items]
                    properties:
                      items:
                        type: array
                        items: { type: string }
  /widgets/{id}:
    delete:
      operationId: delete_widget
      parameters:
        - { name: id, in: path, required: true, schema: { type: string } }
      responses:
        '204':
          description: Deleted, no body
        '404':
          description: Not found
          content:
            application/json:
              schema: { $ref: '#/components/schemas/Error' }
"#;

fn spec() -> OpenApiV3Spec {
    serde_yaml::from_str(SPEC).unwrap()
}

#[test]
fn all_of_response_merges_members_into_one_struct() {
    let spec = spec();
    let op = spec
        .paths
        .as_ref()
        .unwrap()
        .get("/widgets")
        .unwrap()
        .get
        .as_ref()
        .unwrap();
    let (schema, _, _) = extract_response_schema_and_example(&spec, op);
    let fields = extract_fields(&schema.expect("200 response schema"));

    let mut names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec!["items", "next_cursor", "page", "total"],
        "all allOf members' properties"
    );

    let field = |n: &str| fields.iter().find(|f| f.name == n).unwrap();
    assert!(
        !field("items").optional,
        "items is required by the second member"
    );
    assert!(
        !field("total").optional,
        "total is required by the pagination member"
    );
    assert!(
        field("next_cursor").optional,
        "next_cursor is required by no member"
    );
    assert!(
        field("items").ty.starts_with("Vec<"),
        "items keeps its array type: {}",
        field("items").ty
    );
}

#[test]
fn nested_all_of_and_own_properties_are_merged() {
    let schema = json!({
        "allOf": [
            { "allOf": [ { "properties": { "a": { "type": "string" } }, "required": ["a"] } ] },
            { "properties": { "b": { "type": "integer" } } }
        ],
        "properties": { "c": { "type": "boolean" } },
        "required": ["c"]
    });
    let fields = extract_fields(&schema);
    let mut names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["a", "b", "c"]);
    assert!(fields.iter().find(|f| f.name == "b").unwrap().optional);
    assert!(!fields.iter().find(|f| f.name == "c").unwrap().optional);
}

#[test]
fn body_less_success_does_not_fall_back_to_the_error_schema() {
    let spec = spec();
    let op = spec
        .paths
        .as_ref()
        .unwrap()
        .get("/widgets/{id}")
        .unwrap()
        .delete
        .as_ref()
        .unwrap();
    let (schema, example, all) = extract_response_schema_and_example(&spec, op);
    assert!(
        schema.is_none(),
        "204 has no body: no default schema, not the 404 error schema: {schema:?}"
    );
    assert!(example.is_none());
    assert!(all.contains_key(&204), "the 204 is still recorded");
    assert!(all
        .get(&404)
        .and_then(|m| m.get("application/json"))
        .is_some());
}
