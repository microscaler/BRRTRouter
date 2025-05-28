use brrtrouter::spec::RouteMeta;

fn parameter_spec() -> &'static str {
    r#"openapi: 3.1.0
info:
  title: Param Test
  version: '1.0.0'
paths:
  /items/{id}:
    get:
      operationId: get_item
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
        - name: verbose
          in: query
          required: false
          schema:
            type: boolean
      responses:
        '200': { description: OK }
"#
}

fn parse_spec(yaml: &str) -> Vec<RouteMeta> {
    let spec = serde_yaml::from_str(yaml).expect("failed to parse YAML spec");
    brrtrouter::spec::load_spec_from_spec(spec, false).expect("failed to load spec")
}

#[test]
fn test_parameter_meta() {
    let routes = parse_spec(parameter_spec());
    let meta = routes.iter().find(|r| r.handler_name == "get_item").expect("route not found");
    assert_eq!(meta.parameters.len(), 2);
    assert!(meta.parameters.iter().any(|p| p.name == "id" && p.location == "path" && p.required));
    assert!(meta.parameters.iter().any(|p| p.name == "verbose" && p.location == "query" && !p.required));
}
