//! Shared HTTP client for pet_store Docker E2E tests.
//!
//! Used by `curl_integration_tests.rs` and `ui_scenarios_pet_store.rs` so UI / curl scenarios
//! stay aligned and regressions are caught in CI (`cargo llvm-cov nextest`).
#![allow(dead_code)]
// Each integration test crate imports a different subset; keep the full API without warnings.

use reqwest::blocking::{Client, Response};
use reqwest::Method;
use std::time::Duration;

/// Default API key from `examples/pet_store/config/config.yaml` (`ApiKeyHeader` → `test123`).
pub const PET_STORE_API_KEY: &str = "test123";

#[derive(Default, Clone, Debug)]
pub struct HttpOptions {
    pub connect_timeout_ms: Option<u64>,
    pub max_time_ms: Option<u64>,
    pub headers: Vec<(String, String)>,
    pub method: Option<String>,
    pub data: Option<String>,
}

/// One HTTP exchange (status, body, header dump for failures).
#[derive(Debug, Clone)]
pub struct HttpExchange {
    pub status: u16,
    pub success: bool,
    pub body: String,
    pub headers_dump: String,
}

pub fn api_key_headers() -> Vec<(String, String)> {
    vec![("X-API-Key".to_string(), PET_STORE_API_KEY.to_string())]
}

pub fn run_http_with(url: &str, opts: &HttpOptions) -> HttpExchange {
    let mut client_builder = Client::builder();
    if let Some(ct) = opts.connect_timeout_ms {
        client_builder = client_builder.connect_timeout(Duration::from_millis(ct));
    }
    let client = match client_builder.build() {
        Ok(c) => c,
        Err(e) => {
            return HttpExchange {
                status: 0,
                success: false,
                body: String::new(),
                headers_dump: format!("client build error: {e}"),
            };
        }
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
            let status = r.status().as_u16();
            let success = r.status().is_success();
            let headers_str = {
                let mut h = String::new();
                h.push_str(&format!("HTTP/1.1 {}\n", r.status()));
                for (k, v) in r.headers() {
                    h.push_str(&format!("{}: {}\n", k, v.to_str().unwrap_or("<bin>")));
                }
                h
            };
            let body = r.text().unwrap_or_default();
            HttpExchange {
                status,
                success,
                body,
                headers_dump: headers_str,
            }
        }
        Err(e) => HttpExchange {
            status: 0,
            success: false,
            body: String::new(),
            headers_dump: format!("request error: {e}"),
        },
    }
}

pub fn run_http(url: &str) -> HttpExchange {
    run_http_with(url, &HttpOptions::default())
}
