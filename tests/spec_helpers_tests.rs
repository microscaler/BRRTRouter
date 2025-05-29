use brrtrouter::spec::{
    extract_parameters, extract_request_schema, extract_response_schema_and_example,
    resolve_schema_ref, ParameterLocation,
};
use oas3::OpenApiV3Spec;
use serde_json::json;

const SPEC: &str = r#"openapi: 3.1.0
info:
  title: API
  version: '1.0'
components:
  schemas:
    Foo:
      type: object
      properties:
        id: { type: string }
    Bar:
      type: object
      properties:
        count: { type: integer }
  parameters:
    IdParam:
      name: id
      in: path
      required: true
      schema: { type: string }
paths:
  /foo/{id}:
    parameters:
      - $ref: '#/components/parameters/IdParam'
    get:
      operationId: getFoo
      parameters:
        - name: debug
          in: query
          schema: { type: boolean }
      responses:
        '200':
          description: Ok
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Foo'
              examples:
                ex:
                  value:
                    id: 'abc'
  /bar:
    post:
      operationId: createBar
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/Bar'
      responses:
        '200':
          description: Ok
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Foo'
              examples:
                ex:
                  value:
                    id: 'xyz'
"#;

fn get_spec() -> OpenApiV3Spec {
    serde_yaml::from_str(SPEC).unwrap()
}

#[test]
fn test_resolve_schema_ref() {
    let spec = get_spec();
    let schema = resolve_schema_ref(&spec, "#/components/schemas/Foo").unwrap();
    let value = serde_json::to_value(schema).unwrap();
    assert_eq!(value["properties"]["id"]["type"], "string");
}

#[test]
fn test_extract_request_and_response() {
    let spec = get_spec();
    let bar_op = spec
        .paths
        .as_ref()
        .unwrap()
        .get("/bar")
        .unwrap()
        .post
        .as_ref()
        .unwrap();

    let req = extract_request_schema(&spec, bar_op).unwrap();
    assert_eq!(req["properties"]["count"]["type"], "integer");

    let (resp, example, all) = extract_response_schema_and_example(&spec, bar_op);
    assert_eq!(resp.unwrap()["properties"]["id"]["type"], "string");
    assert_eq!(example.unwrap(), json!({"id": "xyz"}));
    let meta = all.get(&200).unwrap().get("application/json").unwrap();
    assert_eq!(meta.example.as_ref().unwrap(), &json!({"id": "xyz"}));
}

#[test]
fn test_extract_parameters() {
    let spec = get_spec();
    let item = spec.paths.as_ref().unwrap().get("/foo/{id}").unwrap();
    let get_op = item.get.as_ref().unwrap();

    let mut params = extract_parameters(&spec, &item.parameters);
    params.extend(extract_parameters(&spec, &get_op.parameters));

    assert_eq!(params.len(), 2);

    let id_p = params.iter().find(|p| p.name == "id").unwrap();
    assert_eq!(id_p.location, ParameterLocation::Path);
    assert!(id_p.required);
    assert_eq!(id_p.schema.as_ref().unwrap()["type"], "string");

    let dbg_p = params.iter().find(|p| p.name == "debug").unwrap();
    assert_eq!(dbg_p.location, ParameterLocation::Query);
    assert!(!dbg_p.required);
}
