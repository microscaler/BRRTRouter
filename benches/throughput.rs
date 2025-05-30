use brrtrouter::{router::Router, spec::RouteMeta};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use http::Method;

fn example_spec() -> &'static str {
    r#"openapi: 3.1.0
info:
  title: Verb Zoo
  version: "1.0.0"
paths:
  "/":
    get:
      operationId: root_handler
      responses:
        "200": { description: OK }
  /zoo/animals:
    get:
      operationId: get_animals
      responses:
        "200": { description: OK }
    post:
      operationId: create_animal
      responses:
        "200": { description: OK }

  /zoo/animals/{id}:
    get:
      operationId: get_animal
      responses:
        "200": { description: OK }
    put:
      operationId: update_animal
      responses:
        "200": { description: OK }
    patch:
      operationId: patch_animal
      responses:
        "200": { description: OK }
    delete:
      operationId: delete_animal
      responses:
        "200": { description: OK }

  /zoo/health:
    head:
      operationId: health_check
      responses:
        "200": { description: OK }
    options:
      operationId: supported_ops
      responses:
        "200": { description: OK }
    trace:
      operationId: trace_route
      responses:
        "200": { description: OK }
"#
}

fn parse_spec(yaml: &str) -> Vec<RouteMeta> {
    let spec = serde_yaml::from_str(yaml).expect("failed to parse YAML spec");
    brrtrouter::spec::load_spec_from_spec(spec).expect("failed to load spec")
}

fn bench_route_throughput(c: &mut Criterion) {
    let routes = parse_spec(example_spec());
    let router = Router::new(routes);
    c.bench_function("route_match", |b| {
        b.iter(|| {
            let res = router.route(Method::GET, "/zoo/animals/123");
            black_box(res);
        })
    });
}

criterion_group!(benches, bench_route_throughput);
criterion_main!(benches);
