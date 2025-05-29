use brrtrouter::{load_spec, spec::ParameterLocation};
use http::Method;
use oas3::OpenApiV3Spec;

const YAML_SPEC: &str = r#"openapi: 3.1.0
info:
  title: Test API
  version: "1.0.0"
components:
  schemas:
    Item:
      type: object
      properties:
        id: { type: string }
        name: { type: string }
  parameters:
    IdParam:
      name: id
      in: path
      required: true
      schema: { type: string }
paths:
  /items/{id}:
    put:
      operationId: update_item
      parameters:
        - $ref: '#/components/parameters/IdParam'
        - name: debug
          in: query
          required: false
          schema: { type: boolean }
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/Item'
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Item'
              examples:
                example:
                  value:
                    id: '123'
                    name: 'Widget'
"#;

fn write_temp(content: &str, ext: &str) -> std::path::PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "spec_test_{}_{}.{}",
        std::process::id(),
        nanos,
        ext
    ));
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn test_load_spec_yaml_and_json() {
    // YAML spec
    let yaml_path = write_temp(YAML_SPEC, "yaml");
    let (routes_yaml, slug_yaml) = load_spec(yaml_path.to_str().unwrap()).unwrap();

    // JSON spec
    let spec: OpenApiV3Spec = serde_yaml::from_str(YAML_SPEC).unwrap();
    let json_str = serde_json::to_string(&spec).unwrap();
    let json_path = write_temp(&json_str, "json");
    let (routes_json, slug_json) = load_spec(json_path.to_str().unwrap()).unwrap();

    assert_eq!(slug_yaml, "test_api");
    assert_eq!(slug_yaml, slug_json);
    assert_eq!(routes_yaml.len(), 1);
    assert_eq!(routes_json.len(), 1);

    let route_y = &routes_yaml[0];
    let route_j = &routes_json[0];

    assert_eq!(route_y.method, Method::PUT);
    assert_eq!(route_y.method, route_j.method);
    assert_eq!(route_y.path_pattern, "/items/{id}");
    assert_eq!(route_y.handler_name, "update_item");
    assert_eq!(route_y.handler_name, route_j.handler_name);
    assert_eq!(route_y.parameters.len(), 2);
    assert_eq!(route_y.parameters.len(), route_j.parameters.len());

    let p_id = &route_y.parameters[0];
    assert_eq!(p_id.name, "id");
    assert_eq!(p_id.location, ParameterLocation::Path);
    assert!(p_id.required);
    assert!(p_id.schema.is_some());

    let p_dbg = &route_y.parameters[1];
    assert_eq!(p_dbg.name, "debug");
    assert_eq!(p_dbg.location, ParameterLocation::Query);

    assert!(route_y.request_schema.is_some());
    assert!(route_y.response_schema.is_some());
    assert!(route_y.example.is_some());
    assert_eq!(route_y.example, route_j.example);
    assert_eq!(route_y.example_name, "test_api_example");
}

use std::process::Command;

const YAML_NO_OPID: &str = r#"openapi: 3.1.0
info:
  title: Bad API
  version: '1.0.0'
paths:
  /foo:
    get:
      responses:
        '200': { description: OK }
"#;

const YAML_UNSUPPORTED_METHOD: &str = r#"openapi: 3.1.0
info:
  title: Bad API
  version: '1.0.0'
paths:
  /foo:
    connect:
      operationId: connect_foo
      responses:
        '200': { description: OK }
"#;

#[test]
fn test_missing_operation_id_exits() {
    let path = write_temp(YAML_NO_OPID, "yaml");
    let exe = env!("CARGO_BIN_EXE_spec_helper");
    let output = Command::new(exe)
        .arg(path.to_str().unwrap())
        .output()
        .expect("run spec_helper");
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn test_unsupported_method_ignored() {
    let path = write_temp(YAML_UNSUPPORTED_METHOD, "yaml");
    let exe = env!("CARGO_BIN_EXE_spec_helper");
    let output = Command::new(exe)
        .arg(path.to_str().unwrap())
        .output()
        .expect("run spec_helper");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("routes: 0"));
}
