use brrrouter::{
    router::{RouteMatch, Router},
    spec::{ RouteMeta},
};

use http::Method;

fn example_spec() -> &'static str {
    r#"
openapi: 3.1.0
info:
  title: Verb Zoo
  version: "1.0.0"
paths:
  /zoo/animals:
    get:
      x-handler-get: get_animals
      responses:
        "200": { description: OK }
    post:
      x-handler-create: create_animal
      responses:
        "200": { description: OK }

  /zoo/animals/{id}:
    get:
      x-handler-get: get_animal
      responses:
        "200": { description: OK }
    put:
      x-handler-update: update_animal
      responses:
        "200": { description: OK }
    patch:
      x-handler-patch: patch_animal
      responses:
        "200": { description: OK }
    delete:
      x-handler-delete: delete_animal
      responses:
        "200": { description: OK }

  /zoo/health:
    head:
      x-handler-head: health_check
      responses:
        "200": { description: OK }
    options:
      x-handler-options: supported_ops
      responses:
        "200": { description: OK }
    trace:
      x-handler-trace: trace_route
      responses:
        "200": { description: OK }
"#
}

fn parse_spec(yaml: &str) -> Vec<RouteMeta> {
    let spec = serde_yaml::from_str(yaml).expect("failed to parse YAML spec");
    brrrouter::spec::load_spec_from_spec(spec, false).expect("failed to load spec")
}

// Internal version of load_spec that takes a spec object directly
// Add this to `spec.rs` if not already present
#[allow(dead_code)]
pub fn load_spec_from_spec(spec_wrapper: oas3::OpenApiV3Spec) -> anyhow::Result<Vec<RouteMeta>> {
    brrrouter::spec::load_spec_from_spec(spec_wrapper, false)
}

#[test]
fn test_router_with_all_http_verbs() {
    let routes = parse_spec(example_spec());
    let router = Router::new(routes);

    let cases = vec![
        (Method::GET, "/zoo/animals", "get_animals"),
        (Method::POST, "/zoo/animals", "create_animal"),
        (Method::GET, "/zoo/animals/123", "get_animal"),
        (Method::PUT, "/zoo/animals/123", "update_animal"),
        (Method::PATCH, "/zoo/animals/123", "patch_animal"),
        (Method::DELETE, "/zoo/animals/123", "delete_animal"),
        (Method::HEAD, "/zoo/health", "health_check"),
        (Method::OPTIONS, "/zoo/health", "supported_ops"),
        (Method::TRACE, "/zoo/health", "trace_route"),
        (Method::GET, "/unknown", "<none>"),
    ];

    for (method, path, expected_handler) in cases {
        let result = router.route(method.clone(), path);
        match result {
            Some(RouteMatch { route, path_params: _ }) => {
                println!("✅ {} {} → {}", method, path, route.handler_name);
                assert_eq!(
                    route.handler_name, expected_handler,
                    "Handler mismatch for {} {}: expected '{}', got '{}'",
                    method, path, expected_handler, route.handler_name
                );
            }
            None => {
                println!("❌ {} {} → no match", method, path);
                assert_eq!(expected_handler, "<none>");
            }
        }
    }
}
