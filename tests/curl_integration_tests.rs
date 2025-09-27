use reqwest::blocking::{Client, Response};
use reqwest::Method;
use std::time::Duration;
#[path = "curl_harness.rs"]
mod curl_harness;

#[derive(Default, Clone, Debug)]
struct HttpOptions {
    connect_timeout_ms: Option<u64>,
    max_time_ms: Option<u64>,
    headers: Vec<(String, String)>,
    method: Option<String>,
    data: Option<String>,
}

fn run_http_with(url: &str, opts: &HttpOptions) -> (bool, String, String) {
    let mut client_builder = Client::builder();
    if let Some(ct) = opts.connect_timeout_ms {
        client_builder = client_builder.connect_timeout(Duration::from_millis(ct));
    }
    let client = match client_builder.build() {
        Ok(c) => c,
        Err(e) => return (false, String::new(), format!("client build error: {e}")),
    };
    let method = opts
        .method
        .as_deref()
        .and_then(|m| m.parse::<Method>().ok())
        .unwrap_or(Method::GET);
    let mut req = client.request(method, url);
    for (name, val) in &opts.headers {
        req = req.header(name, val);
    }
    if let Some(d) = &opts.data {
        req = req.body(d.clone());
    }
    if let Some(mt) = opts.max_time_ms {
        req = req.timeout(Duration::from_millis(mt));
    }
    let resp: Result<Response, _> = req.send();
    match resp {
        Ok(r) => {
            let status_ok = r.status().is_success();
            let headers_str = {
                let mut h = String::new();
                h.push_str(&format!("HTTP/1.1 {}\n", r.status()));
                for (k, v) in r.headers() {
                    h.push_str(&format!("{}: {}\n", k, v.to_str().unwrap_or("<bin>")));
                }
                h
            };
            let body = r.text().unwrap_or_default();
            (status_ok, body, headers_str)
        }
        Err(e) => (false, String::new(), format!("request error: {e}")),
    }
}

fn run_http(url: &str) -> (bool, String, String) {
    run_http_with(url, &HttpOptions::default())
}

#[test]
fn curl_health_works() {
    let url = format!("{}/health", curl_harness::base_url());
    let (ok, _body, headers) = run_http(&url);
    assert!(ok, "GET /health failed: headers=\n{}", headers);
}

#[test]
fn curl_openapi_yaml_served() {
    let url = format!("{}/openapi.yaml", curl_harness::base_url());
    let (ok, body, headers) = run_http(&url);
    assert!(ok, "GET /openapi.yaml failed: headers=\n{}", headers);
    assert!(body.contains("openapi: 3.1.0"));
}

#[test]
fn curl_docs_html_served() {
    let url = format!("{}/docs", curl_harness::base_url());
    let (ok, body, headers) = run_http(&url);
    assert!(ok, "GET /docs failed: headers=\n{}", headers);
    assert!(body.contains("SwaggerUIBundle"));
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
    let (ok, body, headers) =
        run_http_with(&format!("{}/metrics", curl_harness::base_url()), &opts);
    assert!(ok, "GET /metrics failed: headers=\n{}", headers);
    assert!(body.contains("brrtrouter_requests_total"));
    assert!(body.contains("brrtrouter_top_level_requests_total"));
    assert!(body.contains("brrtrouter_auth_failures_total"));
    assert!(body.contains("brrtrouter_request_latency_seconds"));
}

#[test]
fn curl_auth_api_key_unauthorized_then_authorized() {
    // Without API key should be 401
    let url = format!("{}/pets", curl_harness::base_url());
    let (ok_no_key, _body_no_key, headers_no_key) = run_http(&url);
    assert!(!ok_no_key, "GET /pets without key should fail: headers=\n{}", headers_no_key);

    // With API key should be 200
    let opts = HttpOptions {
        headers: vec![("X-API-Key".to_string(), "test123".to_string())],
        ..Default::default()
    };
    let (ok_with_key, _body_with_key, headers_with_key) =
        run_http_with(&format!("{}/pets", curl_harness::base_url()), &opts);
    assert!(ok_with_key, "GET /pets with key failed: headers=\n{}", headers_with_key);
}

#[test]
fn curl_static_index_html_served() {
    // The container ships a static index only; verify it's served.
    let (ok, body, headers) = run_http(&format!("{}/index.html", curl_harness::base_url()));
    assert!(ok, "GET /index.html failed: headers=\n{}", headers);
    assert!(body.contains("It works!"));
}
