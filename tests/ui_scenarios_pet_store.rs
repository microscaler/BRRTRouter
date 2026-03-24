//! Pet Store scenarios aligned with **sample-ui** manual checks and curl smoke tests.
//!
//! Run via the same Docker harness as `curl_integration_tests` (requires Docker + image build).
//! CI: `cargo llvm-cov nextest` includes this crate's integration tests.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

#[path = "curl_harness.rs"]
mod curl_harness;

use common::pet_store_e2e::{
    api_key_headers, run_http_with, HttpExchange, HttpOptions, PET_STORE_API_KEY,
};
use reqwest::blocking::multipart;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::time::Duration;

fn base() -> &'static str {
    curl_harness::base_url()
}

fn assert_success(ex: &HttpExchange, label: &str) {
    assert!(
        ex.success,
        "{label}: expected 2xx, got status={} body={} headers={}",
        ex.status, ex.body, ex.headers_dump
    );
}

fn assert_json_array_or_object(ex: &HttpExchange, label: &str) -> Value {
    assert_success(ex, label);
    let v: Value = serde_json::from_str(&ex.body).unwrap_or_else(|e| {
        panic!("{label}: invalid JSON: {e} body={}", ex.body);
    });
    assert!(
        v.is_array() || v.is_object(),
        "{label}: expected JSON array or object, got {v:?}"
    );
    v
}

#[test]
fn ui_list_pets() {
    let ex = run_http_with(
        &format!("{}/pets", base()),
        &HttpOptions {
            headers: api_key_headers(),
            ..Default::default()
        },
    );
    let v = assert_json_array_or_object(&ex, "GET /pets");
    assert!(v.is_array(), "GET /pets should return a JSON array");
}

#[test]
fn ui_create_pet() {
    let body = json!({ "name": "CI Pet" }).to_string();
    let ex = run_http_with(
        &format!("{}/pets", base()),
        &HttpOptions {
            method: Some("POST".into()),
            headers: api_key_headers(),
            data: Some(body),
            ..Default::default()
        },
    );
    assert_success(&ex, "POST /pets");
    let v: Value = serde_json::from_str(&ex.body).expect("POST /pets JSON");
    assert!(
        v.get("id").is_some() || v.get("status").is_some(),
        "POST /pets should return PetCreationResponse fields: {v}"
    );
}

#[test]
fn ui_get_pet_by_id() {
    let ex = run_http_with(
        &format!("{}/pets/12345", base()),
        &HttpOptions {
            headers: api_key_headers(),
            ..Default::default()
        },
    );
    let v = assert_json_array_or_object(&ex, "GET /pets/{id}");
    assert!(v.get("name").is_some(), "expected Pet object: {v}");
}

#[test]
fn ui_list_users() {
    let ex = run_http_with(
        &format!("{}/users", base()),
        &HttpOptions {
            headers: api_key_headers(),
            ..Default::default()
        },
    );
    assert_success(&ex, "GET /users");
    let v: Value = serde_json::from_str(&ex.body).expect("GET /users JSON");
    assert!(v.get("users").is_some(), "expected {{ users: [...] }}: {v}");
}

#[test]
fn ui_get_user_by_id() {
    let ex = run_http_with(
        &format!("{}/users/abc-123", base()),
        &HttpOptions {
            headers: api_key_headers(),
            ..Default::default()
        },
    );
    assert_success(&ex, "GET /users/{user_id}");
}

#[test]
fn ui_head_user() {
    let ex = run_http_with(
        &format!("{}/users/abc-123", base()),
        &HttpOptions {
            method: Some("HEAD".into()),
            headers: api_key_headers(),
            ..Default::default()
        },
    );
    assert_eq!(ex.status, 200, "HEAD /users/{{id}}: {:?}", ex);
}

#[test]
fn ui_options_user_preflight_allowed_origin() {
    // Must match an origin in examples/pet_store/config/config.yaml (cors.origins).
    let mut headers = api_key_headers();
    headers.push(("Origin".into(), "http://localhost:3000".into()));
    headers.push(("Access-Control-Request-Method".into(), "GET".into()));
    let ex = run_http_with(
        &format!("{}/users/abc-123", base()),
        &HttpOptions {
            method: Some("OPTIONS".into()),
            headers,
            ..Default::default()
        },
    );
    assert!(
        ex.status == 200 || ex.status == 204,
        "OPTIONS preflight: status={} body={}",
        ex.status,
        ex.body
    );
}

#[test]
fn ui_delete_user() {
    let ex = run_http_with(
        &format!("{}/users/abc-123", base()),
        &HttpOptions {
            method: Some("DELETE".into()),
            headers: api_key_headers(),
            ..Default::default()
        },
    );
    assert_success(&ex, "DELETE /users/{id}");
}

#[test]
fn ui_list_user_posts() {
    let ex = run_http_with(
        &format!("{}/users/abc-123/posts", base()),
        &HttpOptions {
            headers: api_key_headers(),
            ..Default::default()
        },
    );
    let v = assert_json_array_or_object(&ex, "GET /users/.../posts");
    assert!(v.is_array(), "expected JSON array of posts: {v}");
}

#[test]
fn ui_get_post_by_id() {
    let ex = run_http_with(
        &format!("{}/users/abc-123/posts/post1", base()),
        &HttpOptions {
            headers: api_key_headers(),
            ..Default::default()
        },
    );
    assert_success(&ex, "GET /users/.../posts/{post_id}");
}

#[test]
fn ui_admin_settings() {
    let ex = run_http_with(
        &format!("{}/admin/settings", base()),
        &HttpOptions {
            headers: api_key_headers(),
            ..Default::default()
        },
    );
    assert_success(&ex, "GET /admin/settings");
}

#[test]
fn ui_get_item_by_id() {
    let ex = run_http_with(
        &format!("{}/items/item-001", base()),
        &HttpOptions {
            headers: api_key_headers(),
            ..Default::default()
        },
    );
    assert_success(&ex, "GET /items/{id}");
}

#[test]
fn ui_post_item_create_or_update() {
    let body = json!({ "name": "CI Item" }).to_string();
    let ex = run_http_with(
        &format!("{}/items/item-001", base()),
        &HttpOptions {
            method: Some("POST".into()),
            headers: api_key_headers(),
            data: Some(body),
            ..Default::default()
        },
    );
    assert_success(&ex, "POST /items/{id}");
}

#[test]
fn ui_sse_events_stream() {
    let ex = run_http_with(
        &format!("{}/events", base()),
        &HttpOptions {
            headers: api_key_headers(),
            max_time_ms: Some(3000),
            ..Default::default()
        },
    );
    assert_eq!(ex.status, 200, "GET /events: {:?}", ex.headers_dump);
    assert!(
        ex.headers_dump.to_lowercase().contains("text/event-stream")
            || ex.body.contains("event:")
            || ex.body.contains("data:"),
        "expected SSE markers or event-stream: body_len={} headers={}",
        ex.body.len(),
        ex.headers_dump
    );
}

#[test]
fn ui_download_file_metadata_json() {
    let ex = run_http_with(
        &format!("{}/download/550e8400-e29b-41d4-a716-446655440000", base()),
        &HttpOptions {
            headers: api_key_headers(),
            ..Default::default()
        },
    );
    assert_success(&ex, "GET /download/{id}");
    let v: Value = serde_json::from_str(&ex.body).expect("download JSON");
    assert!(
        v.get("id").is_some() && v.get("url").is_some(),
        "expected id+url: {v}"
    );
}

#[test]
fn ui_form_urlencoded() {
    let ex = run_http_with(
        &format!("{}/form", base()),
        &HttpOptions {
            method: Some("POST".into()),
            headers: {
                let mut h = api_key_headers();
                h.push((
                    "Content-Type".into(),
                    "application/x-www-form-urlencoded".into(),
                ));
                h
            },
            data: Some("name=ci&age=30".into()),
            ..Default::default()
        },
    );
    assert_success(&ex, "POST /form (urlencoded)");
    let v: Value = serde_json::from_str(&ex.body).expect("form response JSON");
    assert_eq!(v.get("ok"), Some(&json!(true)));
}

#[test]
fn ui_upload_multipart() {
    let url = format!("{}/upload", base());
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
            .mime_str("image/png")
            .expect("mime")
            .file_name("ci.png"),
    );
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("client");
    let resp = client
        .post(&url)
        .header("X-API-Key", PET_STORE_API_KEY)
        .multipart(form)
        .send()
        .expect("multipart send");
    assert!(
        resp.status().is_success(),
        "POST /upload: status={} body={}",
        resp.status(),
        resp.text().unwrap_or_default()
    );
}

#[test]
fn ui_label_style_path() {
    let ex = run_http_with(
        &format!("{}/labels/red", base()),
        &HttpOptions {
            headers: api_key_headers(),
            ..Default::default()
        },
    );
    assert_success(&ex, "GET /labels/{color}");
    let v: Value = serde_json::from_str(&ex.body).expect("labels JSON");
    assert_eq!(v.get("color"), Some(&json!("red")));
}

#[test]
fn ui_matrix_style_path() {
    // Matrix serialization for `coords` (see OpenAPI `style: matrix`).
    let ex = run_http_with(
        &format!("{}/matrix;coords=1,2,3", base()),
        &HttpOptions {
            headers: api_key_headers(),
            ..Default::default()
        },
    );
    assert_success(&ex, "GET /matrix;coords=...");
    let v: Value = serde_json::from_str(&ex.body).expect("matrix JSON");
    assert!(
        v.get("coords").is_some(),
        "expected coords in body: {v} status={}",
        ex.status
    );
}

#[test]
fn ui_search_complex_query() {
    let ex = run_http_with(
        &format!("{}/search?tags=a|b&filters%5Bx%5D=y", base()),
        &HttpOptions {
            headers: {
                let mut h = api_key_headers();
                h.push((
                    "X-Trace-Id".into(),
                    "550e8400-e29b-41d4-a716-446655440000".into(),
                ));
                h
            },
            ..Default::default()
        },
    );
    assert_success(&ex, "GET /search");
    let v: Value = serde_json::from_str(&ex.body).expect("search JSON");
    assert!(v.get("results").is_some(), "expected results: {v}");
}

#[test]
fn ui_secure_endpoint_bearer() {
    let mut headers = api_key_headers();
    headers.push((
        "Authorization".into(),
        "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
            .into(),
    ));
    let ex = run_http_with(
        &format!("{}/secure", base()),
        &HttpOptions {
            headers,
            ..Default::default()
        },
    );
    assert_success(&ex, "GET /secure");
}

#[test]
fn ui_register_webhook() {
    let body = json!({ "url": "https://example.com/hook" }).to_string();
    let ex = run_http_with(
        &format!("{}/webhooks", base()),
        &HttpOptions {
            method: Some("POST".into()),
            headers: api_key_headers(),
            data: Some(body),
            ..Default::default()
        },
    );
    assert_success(&ex, "POST /webhooks");
    let v: Value = serde_json::from_str(&ex.body).expect("webhook JSON");
    assert!(
        v.get("subscription_id").is_some(),
        "expected subscription_id: {v}"
    );
}

#[test]
fn ui_cors_invalid_origin_is_403_with_json_error() {
    let mut headers = api_key_headers();
    headers.push(("Origin".into(), "https://evil.example".into()));
    let ex = run_http_with(
        &format!("{}/pets", base()),
        &HttpOptions {
            headers,
            ..Default::default()
        },
    );
    assert_eq!(ex.status, 403, "expected 403 for bad Origin: {:?}", ex);
    let v: Value = serde_json::from_str(&ex.body).expect("403 JSON body");
    assert!(
        v.get("error").is_some() || v.get("detail").is_some(),
        "403 body should be ProblemDetails-like JSON, got {v}"
    );
}
