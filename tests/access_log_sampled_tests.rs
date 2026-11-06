#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unsafe_code,
    clippy::disallowed_methods
)]
//! `BRRTR_ACCESS_LOG=sample:0.5` with a slow threshold no request reaches: of consecutive requests exactly every
//! other one is logged, in full (headers included), and the unsampled ones are not logged at all. The span keeps
//! every field by default.

mod access_log_util;
mod common;
use access_log_util::*;

#[test]
fn half_the_requests_are_logged_in_full_and_the_rest_not_at_all() {
    std::env::set_var("BRRTR_ACCESS_LOG", "sample:0.5");
    std::env::set_var("BRRTR_ACCESS_LOG_SLOW_MS", "60000");
    std::env::remove_var("BRRTR_REQUEST_SPAN");
    install();
    let (handle, addr) = start();

    for _ in 0..4 {
        assert_eq!(get(addr, "/health"), 200);
    }
    // Whatever request the count started on, four consecutive ones at 0.5 are two sampled ones. Wait for both, then
    // give a stray third (which would be a bug) time to appear.
    let _ = completed_for("/health", 2);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let events = completed_for("/health", 2);
    handle.stop();

    assert_eq!(events.len(), 2, "{events:?}");
    for e in &events {
        assert!(
            e.contains_key("request_headers"),
            "a sampled request carries its headers: {e:?}"
        );
        assert!(
            e.get("request_headers")
                .is_some_and(|h| h.contains("x-probe")),
            "{e:?}"
        );
        assert!(!e.contains_key("sampled"), "{e:?}");
    }

    let spans = captured().lock().unwrap().span_fields.clone();
    assert!(
        spans.iter().all(|f| f.iter().any(|n| n == "header_count")),
        "full spans: {spans:?}"
    );
}
