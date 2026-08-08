//! Story 12.3 — components.requestBodies / responses / pathItems `$ref`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use brrtrouter::spec::load_spec;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// P1 — requestBody `$ref` → request_schema present.
#[test]
fn refs_positive_p1_request_body_ref() {
    let (routes, _) = load_spec(fixture("openapi_component_refs.yaml").to_str().unwrap()).unwrap();
    let create = routes
        .iter()
        .find(|r| r.path_pattern.as_ref() == "/items" && r.method == http::Method::POST)
        .expect("POST /items");
    assert!(create.request_schema.is_some(), "P1 request_schema");
    assert!(create.request_body_required, "P1 required from component");
    assert!(
        create.request_schema.as_ref().unwrap()["properties"]["name"]["type"] == "string",
        "P1 schema fields"
    );
}

/// P2 — response `$ref` → response schema present.
#[test]
fn refs_positive_p2_response_ref() {
    let (routes, _) = load_spec(fixture("openapi_component_refs.yaml").to_str().unwrap()).unwrap();
    let create = routes
        .iter()
        .find(|r| r.path_pattern.as_ref() == "/items" && r.method == http::Method::POST)
        .unwrap();
    assert!(create.response_schema.is_some(), "P2");
    let resp = create
        .responses
        .get(&200)
        .and_then(|m| m.get("application/json"));
    assert!(
        resp.and_then(|r| r.schema.as_ref()).is_some(),
        "P2 responses map"
    );
}

/// P3 — nested schema `$ref` inside resolved body expanded.
#[test]
fn refs_positive_p3_nested_schema_ref() {
    let (routes, _) = load_spec(fixture("openapi_component_refs.yaml").to_str().unwrap()).unwrap();
    let create = routes
        .iter()
        .find(|r| r.path_pattern.as_ref() == "/items")
        .unwrap();
    let schema = create.request_schema.as_ref().unwrap();
    assert_eq!(
        schema["properties"]["nested"]["properties"]["n"]["type"], "integer",
        "P3 nested expanded"
    );
}

/// P4 — components.pathItems + path `$ref` registers route.
#[test]
fn refs_positive_p4_path_item_ref() {
    let (routes, _) = load_spec(fixture("openapi_component_refs.yaml").to_str().unwrap()).unwrap();
    assert!(
        routes
            .iter()
            .any(|r| r.path_pattern.as_ref() == "/items/listed" && r.method == http::Method::GET),
        "P4 pathItems route"
    );
}

/// P5 — mixed inline + ref ops.
#[test]
fn refs_positive_p5_mixed() {
    let (routes, _) = load_spec(fixture("openapi_component_refs.yaml").to_str().unwrap()).unwrap();
    assert!(routes.iter().any(|r| r.path_pattern.as_ref() == "/items"));
    assert!(routes
        .iter()
        .any(|r| r.path_pattern.as_ref() == "/items/mixed"));
    assert!(routes
        .iter()
        .any(|r| r.path_pattern.as_ref() == "/items/listed"));
}

/// P6 — pet_store regression.
#[test]
fn refs_positive_p6_pet_store() {
    let (routes, _) = load_spec("examples/openapi.yaml").unwrap();
    assert!(routes.len() > 5);
    assert!(routes.iter().any(|r| r.request_schema.is_some()));
}

/// N1 — dangling requestBody `$ref` → load error (no silent empty).
#[test]
fn refs_negative_n1_dangling_request_body() {
    let err = load_spec(
        fixture("openapi_dangling_request_body_ref.yaml")
            .to_str()
            .unwrap(),
    )
    .expect_err("N1 must fail");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("UnresolvableRequestBodyRef")
            || msg.contains("requestBody")
            || msg.contains("validation"),
        "N1 message: {msg}"
    );
}

/// N4 — external HTTP pathItem `$ref` rejected.
#[test]
fn refs_negative_n4_external_path_item() {
    use brrtrouter::spec::resolve_path_item_ref;
    let spec: oas3::OpenApiV3Spec = serde_yaml::from_str(
        r#"
openapi: 3.1.0
info: { title: t, version: "1" }
paths: {}
"#,
    )
    .unwrap();
    let err = resolve_path_item_ref(&spec, "https://example.com/paths.yaml#/Foo", 0).unwrap_err();
    assert!(
        err.contains("external") || err.contains("unsupported"),
        "{err}"
    );
}
