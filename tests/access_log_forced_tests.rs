#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]
//! `BRRTR_ACCESS_LOG=off` with `BRRTR_ACCESS_LOG_SLOW_MS=0`: no request is sampled, yet every one is at least that
//! slow, so each is still logged, without the headers it never captured. `BRRTR_REQUEST_SPAN=lean` drops the
//! duplicate span fields and keeps the tenant.

mod access_log_util;
mod common;
use access_log_util::*;

#[test]
fn forced_requests_are_logged_without_headers_and_the_lean_span_keeps_the_tenant() {
    std::env::set_var("BRRTR_ACCESS_LOG", "off");
    std::env::set_var("BRRTR_ACCESS_LOG_SLOW_MS", "0");
    std::env::set_var("BRRTR_REQUEST_SPAN", "lean");
    install();
    let (handle, addr) = start();

    assert_eq!(get(addr, "/health"), 200);
    let events = completed_for("/health", 1);
    handle.stop();

    assert!(!events.is_empty(), "a forced request is logged");
    for e in &events {
        assert_eq!(e.get("sampled").map(String::as_str), Some("false"), "{e:?}");
        assert!(
            !e.contains_key("request_headers"),
            "headers were never captured: {e:?}"
        );
        assert!(!e.contains_key("response_headers"), "{e:?}");
        assert_eq!(e.get("status").map(String::as_str), Some("200"), "{e:?}");
    }

    let spans = captured().lock().unwrap().span_fields.clone();
    assert!(!spans.is_empty(), "an http_request span was created");
    for fields in &spans {
        for gone in ["method", "path", "header_count", "stack_used_kb"] {
            assert!(
                !fields.iter().any(|f| f == gone),
                "lean span has no {gone}: {fields:?}"
            );
        }
        for kept in [
            "tenant_id",
            "http.request.method",
            "http.route",
            "url.path",
            "status",
            "duration_ms",
        ] {
            assert!(
                fields.iter().any(|f| f == kept),
                "lean span keeps {kept}: {fields:?}"
            );
        }
    }
}
