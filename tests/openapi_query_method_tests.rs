//! Story 11.2 — OpenAPI QUERY operations (load + generator body fields).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use brrtrouter::generator::{extract_fields, write_handler};
use brrtrouter::http::{is_query_method, method_query};
use brrtrouter::router::Router;
use brrtrouter::spec::{load_spec, promote_query_operations, QUERY_OPERATION_EXTENSION};
use serde_json::json;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/openapi_query_method.yaml")
}

/// P1 / P5 / P6 — fixture (docs declaration path) loads; GET + QUERY both registered.
#[test]
fn query_method_positive_loads_fixture_and_registers_both() {
    let (routes, _) = load_spec(fixture_path().to_str().unwrap()).expect("P1 load");
    let query = routes
        .iter()
        .find(|r| r.handler_name.as_ref() == "query_search")
        .expect("QUERY route");
    let get = routes
        .iter()
        .find(|r| r.handler_name.as_ref() == "get_search")
        .expect("GET route");

    assert!(is_query_method(&query.method), "P1 QUERY method");
    assert_eq!(get.method, http::Method::GET);
    assert_eq!(query.path_pattern.as_ref(), "/search");
    assert_eq!(get.path_pattern.as_ref(), "/search");

    let router = Router::new(routes.clone());
    assert!(
        router.route(method_query(), "/search").is_some(),
        "P5 QUERY registered"
    );
    assert!(
        router.route(http::Method::GET, "/search").is_some(),
        "P5 GET"
    );

    let via_ext = routes
        .iter()
        .find(|r| r.handler_name.as_ref() == "query_via_extension")
        .expect("P6 extension declaration");
    assert!(is_query_method(&via_ext.method));
}

/// P2 — generated handler Request includes body schema fields (typed body).
#[test]
fn query_method_positive_generated_handler_receives_body_fields() {
    let (routes, _) = load_spec(fixture_path().to_str().unwrap()).unwrap();
    let route = routes
        .iter()
        .find(|r| r.handler_name.as_ref() == "query_search")
        .unwrap();
    let schema = route.request_schema.as_ref().expect("body schema");
    let fields = extract_fields(schema);
    assert!(
        fields.iter().any(|f| f.original_name == "q"),
        "expected q from JSON body schema: {fields:?}"
    );

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("query_search.rs");
    write_handler(
        &path,
        "query_search",
        &fields,
        &[],
        &BTreeSet::new(),
        &route.parameters,
        false,
        false,
        false,
        true,
    )
    .unwrap();
    let src = std::fs::read_to_string(&path).unwrap();
    assert!(src.contains("pub struct Request"), "{src}");
    assert!(src.contains("\"q\""), "{src}");
    assert!(
        src.contains("if let Some(body) = req.body"),
        "handler merges request body into Request: {src}"
    );
}

/// P3 / P4 — JSON schema present; form media type listed for 415 enforcement.
#[test]
fn query_method_positive_json_and_form_content_types() {
    let (routes, _) = load_spec(fixture_path().to_str().unwrap()).unwrap();
    let route = routes
        .iter()
        .find(|r| r.handler_name.as_ref() == "query_search")
        .unwrap();

    let schema = route.request_schema.as_ref().unwrap();
    let compiled = jsonschema::validator_for(schema).expect("compile schema");
    assert!(
        compiled.is_valid(&json!({"q": "hello", "limit": 10})),
        "P3 valid JSON body"
    );
    assert!(
        route
            .request_content_types
            .iter()
            .any(|ct| ct == "application/x-www-form-urlencoded"),
        "P4 form media type recorded: {:?}",
        route.request_content_types
    );
}

/// N1 — required body flagged for runtime 400 path.
#[test]
fn query_method_negative_required_body_flagged() {
    let (routes, _) = load_spec(fixture_path().to_str().unwrap()).unwrap();
    let route = routes
        .iter()
        .find(|r| r.handler_name.as_ref() == "query_search")
        .unwrap();
    assert!(
        route.request_body_required,
        "N1 request_body_required must be true (service returns 400 when body missing)"
    );
}

/// N2 — invalid body fails schema; no panic.
#[test]
fn query_method_negative_invalid_body_rejected() {
    let (routes, _) = load_spec(fixture_path().to_str().unwrap()).unwrap();
    let route = routes
        .iter()
        .find(|r| r.handler_name.as_ref() == "query_search")
        .unwrap();
    let schema = route.request_schema.as_ref().unwrap();
    let compiled = jsonschema::validator_for(schema).unwrap();
    assert!(
        !compiled.is_valid(&json!({"limit": 1})),
        "N2 missing required q"
    );
}

/// N3 — unsupported tooling path documented; promote preserves QUERY (no silent drop).
#[test]
fn query_method_negative_unsupported_path_not_silently_dropped() {
    let mut v = json!({
        "paths": {
            "/s": {
                "query": { "operationId": "q", "responses": { "200": { "description": "ok" } } },
                "search": { "operationId": "not_query" }
            }
        }
    });
    promote_query_operations(&mut v).unwrap();
    assert!(
        v["paths"]["/s"].get(QUERY_OPERATION_EXTENSION).is_some(),
        "N3 QUERY promoted before strip"
    );
    // Legacy unknown verbs remain stripped — documented in declaring-query-operations.md
}

/// N4 / N6 — malformed / duplicate QUERY → load error.
#[test]
fn query_method_negative_malformed_and_duplicate() {
    let mut bad = json!({ "paths": { "/s": { "query": 42 } } });
    assert!(promote_query_operations(&mut bad).is_err(), "N4");

    let mut dup = json!({
        "paths": {
            "/s": {
                "query": { "operationId": "a" },
                "x-brrtrouter-query": { "operationId": "b" }
            }
        }
    });
    assert!(promote_query_operations(&mut dup).is_err(), "N6");
}

/// N7 — body size estimate present (existing limit policy applies at runtime).
#[test]
fn query_method_negative_body_estimate_present() {
    let (routes, _) = load_spec(fixture_path().to_str().unwrap()).unwrap();
    let route = routes
        .iter()
        .find(|r| r.handler_name.as_ref() == "query_search")
        .unwrap();
    assert!(
        route.estimated_request_body_bytes.is_some(),
        "N7 estimate feeds existing body limits"
    );
}

/// N8 — every QUERY route from a valid spec has a handler name (generator gate).
#[test]
fn query_method_negative_handlers_always_named() {
    let (routes, _) = load_spec(fixture_path().to_str().unwrap()).unwrap();
    let query_routes: Vec<_> = routes
        .iter()
        .filter(|r| is_query_method(&r.method))
        .collect();
    assert!(!query_routes.is_empty());
    for r in query_routes {
        assert!(
            !r.handler_name.is_empty(),
            "N8 no half handler without operationId/x-handler"
        );
    }
}
