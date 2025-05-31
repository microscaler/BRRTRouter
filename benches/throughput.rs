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

  /zoo/animals/{id}/toys/{toy_id}:
    get:
      operationId: animal_toy
      responses:
        "200": { description: OK }

  /zoo/{category}/animals/{id}/habitats/{habitat_id}/sections/{section_id}:
    get:
      operationId: habitat_section
      responses:
        "200": { description: OK }

  /inventory/{warehouse_id}/feeds/{feed_id}/items/{item_id}/batches/{batch_id}:
    post:
      operationId: post_item_batch
      responses:
        "200": { description: OK }

  /complex/{a}/{b}/{c}/{d}/{e}/{f}/{g}/{h}/{i}:
    get:
      operationId: complex_many_params
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
        let test_paths = [
            (Method::GET, "/zoo/animals/123"),
            (Method::GET, "/zoo/animals/123/toys/456"),
            (Method::GET, "/zoo/cats/animals/123/habitats/88/sections/5"),
            (Method::POST, "/inventory/1/feeds/2/items/3/batches/4"),
            (Method::GET, "/complex/1/2/3/4/5/6/7/8/9"),
        ];
        b.iter(|| {
            for (method, path) in test_paths.iter() {
                let res = router.route(method.clone(), path);
                black_box(&res);
            }
        })
    });
}

criterion_group!(benches, bench_route_throughput);
criterion_main!(benches);
