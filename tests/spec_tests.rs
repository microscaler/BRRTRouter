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

#[test]
fn test_load_spec_yaml_and_json() {
    // Create manual temp files (NamedTempFile doesn't work reliably in nextest)
    let yaml_path = std::env::temp_dir().join(format!(
        "spec_test_yaml_{}_{}.yaml",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::write(&yaml_path, YAML_SPEC.as_bytes()).unwrap();
    
    let (routes_yaml, slug_yaml) = load_spec(yaml_path.to_str().unwrap()).unwrap();

    // JSON spec
    let spec: OpenApiV3Spec = serde_yaml::from_str(YAML_SPEC).unwrap();
    let json_str = serde_json::to_string(&spec).unwrap();
    let json_path = std::env::temp_dir().join(format!(
        "spec_test_json_{}_{}.json",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::write(&json_path, json_str.as_bytes()).unwrap();
    
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
    assert!(route_y.responses.contains_key(&200));
    assert_eq!(route_y.example, route_j.example);
    assert_eq!(route_y.example_name, "test_api_example");
    
    // Manual cleanup
    let _ = std::fs::remove_file(&yaml_path);
    let _ = std::fs::remove_file(&json_path);
}

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
    use std::process::Command;
    
    // Create manual temp file (NamedTempFile doesn't work reliably in nextest)
    let temp_path = std::env::temp_dir().join(format!(
        "spec_test_bad_{}_{}.yaml",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::write(&temp_path, YAML_NO_OPID.as_bytes()).unwrap();
    
    let exe = env!("CARGO_BIN_EXE_spec_helper");
    let output = Command::new(exe)
        .arg(&temp_path)
        .output()
        .expect("run spec_helper");
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    
    // Manual cleanup
    let _ = std::fs::remove_file(&temp_path);
}

#[test]
fn test_unsupported_method_ignored() {
    use std::process::Command;
    
    // Create manual temp file (NamedTempFile doesn't work reliably in nextest)
    let temp_path = std::env::temp_dir().join(format!(
        "spec_test_unsup_{}_{}.yaml",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::write(&temp_path, YAML_UNSUPPORTED_METHOD.as_bytes()).unwrap();
    
    let exe = env!("CARGO_BIN_EXE_spec_helper");
    let output = Command::new(exe)
        .arg(&temp_path)
        .output()
        .expect("run spec_helper");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("routes: 0"));
    
    // Manual cleanup
    let _ = std::fs::remove_file(&temp_path);
}

const YAML_SSE: &str = r#"openapi: 3.1.0
info:
  title: SSE API
  version: '1.0'
paths:
  /events:
    get:
      operationId: stream
      x-sse: true
      responses:
        '200':
          description: OK
          content:
            text/event-stream: {}
"#;

#[test]
fn test_sse_flag_extracted() {
    let mut op = oas3::spec::Operation::default();
    op.extensions
        .insert("x-sse".to_string(), serde_json::Value::Bool(true));
    assert!(brrtrouter::spec::extract_sse_flag(&op));
}

#[test]
fn test_sse_spec_loading() {
    // Create manual temp file (NamedTempFile doesn't work reliably in nextest)
    let temp_path = std::env::temp_dir().join(format!(
        "spec_test_sse_{}_{}.yaml",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::write(&temp_path, YAML_SSE.as_bytes()).unwrap();
    
    // Load spec from the temp file
    let (routes, _schemes, _slug) = brrtrouter::load_spec_full(temp_path.to_str().unwrap()).unwrap();
    
    // Should have one route: GET /events
    assert_eq!(routes.len(), 1);
    
    let route = &routes[0];
    assert_eq!(route.method.as_str(), "GET");
    assert_eq!(route.path_pattern, "/events");
    assert_eq!(route.handler_name, "stream");
    
    // Most importantly: should have SSE flag set
    assert!(route.sse, "Route should be marked as SSE stream");
    
    // Manual cleanup
    let _ = std::fs::remove_file(&temp_path);
}
