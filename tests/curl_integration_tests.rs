#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

#[path = "curl_harness.rs"]
mod curl_harness;

use common::pet_store_e2e::{run_http, run_http_with, HttpOptions};

#[test]
fn curl_health_works() {
    let url = format!("{}/health", curl_harness::base_url());
    let ex = run_http(&url);
    assert!(ex.success, "GET /health failed: {}", ex.headers_dump);
}

#[test]
fn curl_openapi_yaml_served() {
    let url = format!("{}/openapi.yaml", curl_harness::base_url());
    let ex = run_http(&url);
    assert!(ex.success, "GET /openapi.yaml failed: {}", ex.headers_dump);
    assert!(ex.body.contains("openapi: 3.1.0"));
}

#[test]
fn curl_docs_html_served() {
    let url = format!("{}/docs", curl_harness::base_url());
    let ex = run_http(&url);
    assert!(ex.success, "GET /docs failed: {}", ex.headers_dump);
    assert!(ex.body.contains("SwaggerUIBundle"));
}

#[test]
fn curl_metrics_exposes_prometheus() {
    // Hit a routed endpoint once so counters increment
    let _ = run_http(&format!("{}/pets", curl_harness::base_url()));
    let opts = HttpOptions {
        connect_timeout_ms: Some(3000),
        max_time_ms: Some(4000),
        ..Default::default()
    };
    let ex = run_http_with(&format!("{}/metrics", curl_harness::base_url()), &opts);
    assert!(ex.success, "GET /metrics failed: {}", ex.headers_dump);
    assert!(ex.body.contains("brrtrouter_requests_total"));
    assert!(ex.body.contains("brrtrouter_top_level_requests_total"));
    assert!(ex.body.contains("brrtrouter_auth_failures_total"));
    assert!(ex.body.contains("brrtrouter_request_latency_seconds"));
}

#[test]
fn curl_auth_api_key_unauthorized_then_authorized() {
    // Without API key should be 401
    let url = format!("{}/pets", curl_harness::base_url());
    let ex = run_http(&url);
    assert!(
        !ex.success,
        "GET /pets without key should fail: {}",
        ex.headers_dump
    );

    // With API key should be 200
    let opts = HttpOptions {
        headers: vec![("X-API-Key".to_string(), "test123".to_string())],
        ..Default::default()
    };
    let ex = run_http_with(&format!("{}/pets", curl_harness::base_url()), &opts);
    assert!(ex.success, "GET /pets with key failed: {}", ex.headers_dump);
}

#[test]
fn curl_static_index_html_served() {
    // The container ships a static index. This could be either:
    // 1. Simple "It works!" HTML (default static_site/index.html)
    // 2. SolidJS Pet Store Dashboard (if sample-ui has been built)
    let ex = run_http(&format!("{}/index.html", curl_harness::base_url()));
    assert!(ex.success, "GET /index.html failed: {}", ex.headers_dump);

    // Accept either the simple static HTML or the Pet Store Dashboard
    let is_simple_html = ex.body.contains("It works!");
    let is_pet_store_dashboard = ex.body.contains("Pet Store")
        || ex.body.contains("pet-store")
        || ex.body.contains("BRRTRouter");

    assert!(
        is_simple_html || is_pet_store_dashboard,
        "Expected either simple 'It works!' HTML or Pet Store Dashboard, got body snippet: {}",
        &ex.body[..ex.body.len().min(200)]
    );
}
