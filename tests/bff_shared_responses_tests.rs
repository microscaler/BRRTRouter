//! Regression: a BFF spec merged by `bff generate-system` from member specs that share error
//! responses through `components.responses` loads and generates.
//!
//! `tests/fixtures/bff_shared_responses/openapi_bff.yaml` is the tool's output for the two member
//! specs beside it (the tooling test `test_bff_shared_components.py` checks it stays so).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use brrtrouter::generator::{generate_project_with_options, GenerationScope};
use brrtrouter::spec::load_spec;
use std::path::PathBuf;

fn merged_spec() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bff_shared_responses/openapi_bff.yaml")
}

#[test]
fn merged_spec_with_shared_component_responses_loads() {
    let (routes, _) = load_spec(merged_spec().to_str().unwrap()).expect("merged spec loads");
    let get_widget = routes
        .iter()
        .find(|r| r.path_pattern.ends_with("/widgets/{id}") && r.method == http::Method::GET)
        .expect("GET /widgets/{id}");
    let not_found = get_widget
        .responses
        .get(&404)
        .and_then(|m| m.get("application/json"))
        .and_then(|r| r.schema.as_ref())
        .expect("404 resolved through components.responses");
    assert_eq!(
        not_found["required"],
        serde_json::json!(["code", "message"])
    );
    let create = routes
        .iter()
        .find(|r| r.path_pattern.ends_with("/gadgets") && r.method == http::Method::POST)
        .expect("POST /gadgets");
    assert!(
        create.request_schema.is_some(),
        "requestBody resolved through components.requestBodies"
    );
}

#[test]
fn merged_spec_with_shared_component_responses_generates() {
    let dir = tempfile::tempdir().unwrap();
    let project = generate_project_with_options(
        &merged_spec(),
        Some(dir.path()),
        true,  // force
        false, // dry_run
        &GenerationScope::all(),
        None,
        None,
        None,
    )
    .expect("project generates from the merged spec");
    assert!(project
        .join("src")
        .join("handlers")
        .join("get_widgets.rs")
        .exists());
    assert!(project
        .join("src")
        .join("handlers")
        .join("create_gadgets.rs")
        .exists());
}
