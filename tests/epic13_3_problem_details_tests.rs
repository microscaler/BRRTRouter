//! Epic 13.3 — RFC 7807 Problem Details fixtures + catalog checks.
//!
//! Wire Content-Type is covered by `server::response::tests::test_write_json_error`
//! (lib). This file locks catalog + JSON shape for P1–P6 / N1–N3.

use brrtrouter::http::problem::{
    body_too_large_problem, multipart_problem, parameter_validation_problem, Problem,
    PROBLEM_CONTENT_TYPE, TYPE_MULTIPART_MISSING_BOUNDARY, TYPE_PARAMETER_VALIDATION_FAILED,
    TYPE_REQUEST_BODY_TOO_LARGE,
};
use brrtrouter::server::body_limit::body_too_large_json;
use brrtrouter::server::multipart::{multipart_error_json, MULTIPART_MISSING_BOUNDARY};
use brrtrouter::server::param_validation::{param_validation_error_json, ParamFieldError};
use serde_json::json;

#[test]
fn epic13_3_p1_param_validation_problem_json() {
    let fields = [ParamFieldError {
        name: "q".into(),
        location: "query".into(),
        error: "required".into(),
    }];
    let v = param_validation_error_json(&fields);
    assert_eq!(v["status"], 400);
    assert_eq!(v["type"], TYPE_PARAMETER_VALIDATION_FAILED);
    assert!(v["title"].is_string());
    assert!(v["detail"].is_string());
    assert!(v["fields"].is_array());
    assert_eq!(PROBLEM_CONTENT_TYPE, "application/problem+json");
}

#[test]
fn epic13_3_p2_body_too_large_problem_json() {
    let v = body_too_large_json("Request body exceeds configured maximum size");
    assert_eq!(v["status"], 413);
    assert_eq!(v["type"], TYPE_REQUEST_BODY_TOO_LARGE);
    assert_eq!(v["reason"], "request_body_too_large");
}

#[test]
fn epic13_3_p3_multipart_missing_boundary() {
    let v = multipart_error_json(MULTIPART_MISSING_BOUNDARY);
    assert_eq!(v["status"], 400);
    assert_eq!(v["type"], TYPE_MULTIPART_MISSING_BOUNDARY);
    assert_eq!(v["reason"], "multipart_missing_boundary");
}

#[test]
fn epic13_3_p4_fields_array_shape() {
    let fields = [ParamFieldError {
        name: "id".into(),
        location: "path".into(),
        error: "required".into(),
    }];
    let v = param_validation_error_json(&fields);
    let arr = v["fields"].as_array().unwrap();
    assert_eq!(arr[0]["name"], "id");
    assert_eq!(arr[0]["in"], "path");
}

#[test]
fn epic13_3_p5_reason_extension() {
    assert_eq!(
        body_too_large_problem("x").to_value()["reason"],
        "request_body_too_large"
    );
    assert_eq!(
        multipart_problem(MULTIPART_MISSING_BOUNDARY).to_value()["reason"],
        "multipart_missing_boundary"
    );
}

#[test]
fn epic13_3_p6_catalog_lists_p1_p3_types() {
    let catalog = include_str!("../docs/PROBLEM_DETAILS.md");
    assert!(catalog.contains("parameter-validation-failed"));
    assert!(catalog.contains("request-body-too-large"));
    assert!(catalog.contains("multipart-missing-boundary"));
    assert!(catalog.contains("application/problem+json"));
}

#[test]
fn epic13_3_n2_status_required() {
    let v = parameter_validation_problem(json!([]), "x").to_value();
    assert!(v.get("status").is_some());
}

#[test]
fn epic13_3_n3_handler_response_error_uses_problem_content_type() {
    let resp = brrtrouter::dispatcher::HandlerResponse::error(401, "Unauthorized");
    let ct = resp.get_header("content-type").unwrap_or("");
    assert!(
        ct.starts_with(PROBLEM_CONTENT_TYPE),
        "framework HandlerResponse::error must use problem+json, got {ct}"
    );
    assert_eq!(resp.body["status"], 401);
    assert!(resp.body.get("type").is_some());
}

#[test]
fn epic13_3_n1_success_bodies_remain_json_in_docs() {
    let catalog = include_str!("../docs/PROBLEM_DETAILS.md");
    assert!(
        !catalog
            .to_lowercase()
            .contains("success responses use problem"),
        "must not claim success bodies are problem+json"
    );
    // Auth success path still uses application/json via write_handler_response default.
    let _ = Problem::from_status_detail(200, "x");
}
