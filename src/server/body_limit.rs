//! Inbound request-body size limits (Story 12.2).
//!
//! Hard caps apply **before handler dispatch**:
//! - Global ceiling from [`MAX_REQUEST_BODY_ENV`] (default [`DEFAULT_MAX_REQUEST_BODY_OCTETS`]).
//! - Per-route ceiling from `RouteMeta.estimated_request_body_bytes`
//!   (schema heuristic and/or `x-brrtrouter-body-size-bytes`), never above global.
//!
//! Prefer `Content-Length` when present; otherwise cap the read stream.

use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Default global max inbound body octets (16 MiB).
pub const DEFAULT_MAX_REQUEST_BODY_OCTETS: usize = 16 * 1024 * 1024;

/// Env override: `BRRTROUTER_MAX_REQUEST_BODY_OCTETS` (decimal). `0` or invalid → default.
pub const MAX_REQUEST_BODY_ENV: &str = "BRRTROUTER_MAX_REQUEST_BODY_OCTETS";

/// Error marker returned from [`crate::server::parse_request`] when the body exceeds the global max.
pub const REQUEST_BODY_TOO_LARGE: &str = "request_body_too_large";

/// Stable `reason` field in 413 JSON bodies.
pub const REASON_REQUEST_BODY_TOO_LARGE: &str = "request_body_too_large";

static MAX_REQUEST_BODY_CACHE: AtomicUsize = AtomicUsize::new(0);

/// Configurable global max inbound body length in octets.
#[must_use]
pub fn max_inbound_body_octets() -> usize {
    let cached = MAX_REQUEST_BODY_CACHE.load(Ordering::Relaxed);
    if cached != 0 {
        return cached;
    }
    let resolved = match std::env::var(MAX_REQUEST_BODY_ENV) {
        Ok(s) => match s.parse::<usize>() {
            Ok(0) | Err(_) => DEFAULT_MAX_REQUEST_BODY_OCTETS,
            Ok(n) => n,
        },
        Err(_) => DEFAULT_MAX_REQUEST_BODY_OCTETS,
    };
    MAX_REQUEST_BODY_CACHE.store(resolved, Ordering::Relaxed);
    resolved
}

/// Reset cached max (after env changes in tests). Safe to call in integration tests.
pub fn reset_max_inbound_body_cache_for_tests() {
    MAX_REQUEST_BODY_CACHE.store(0, Ordering::Relaxed);
}

/// Effective hard cap for a matched route: `min(global, route_estimate)` when estimate is set.
#[must_use]
pub fn effective_inbound_body_limit(route_estimate: Option<usize>) -> usize {
    let global = max_inbound_body_octets();
    match route_estimate {
        Some(r) => global.min(r),
        None => global,
    }
}

/// `true` when `octets` exceeds the limit (inclusive bound: `octets > max` rejects).
#[must_use]
pub fn body_exceeds_limit(octets: usize, max: usize) -> bool {
    octets > max
}

/// Parse `Content-Length` for limit checks.
///
/// Returns `Err(())` when the header is present but hostile/unusable (non-decimal or over `max`).
/// Returns `Ok(None)` when the header is absent.
#[must_use]
pub fn content_length_for_limit(
    content_length_header: Option<&str>,
    max: usize,
) -> Result<Option<usize>, ()> {
    let Some(raw) = content_length_header else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(());
    }
    let Ok(n) = trimmed.parse::<u64>() else {
        return Err(());
    };
    if n > max as u64 {
        return Err(());
    }
    Ok(Some(n as usize))
}

/// Stable 413 Problem Details body (RFC 7807; Epic 13.3).
#[must_use]
pub fn body_too_large_json(message: &str) -> Value {
    crate::http::problem::body_too_large_problem(message).to_value()
}

/// Map parse_request / early reject markers to HTTP status when body-related.
#[must_use]
pub fn body_limit_error_status(err: &str) -> Option<u16> {
    if err == REQUEST_BODY_TOO_LARGE {
        Some(413)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn body_limit_positive_p1_under_global() {
        assert!(!body_exceeds_limit(100, 1000));
        assert!(!body_exceeds_limit(1000, 1000));
    }

    #[test]
    fn body_limit_positive_p3_exact_at_limit_accepted() {
        // Inclusive: equal to max is accepted.
        assert!(!body_exceeds_limit(4096, 4096));
        assert_eq!(content_length_for_limit(Some("4096"), 4096), Ok(Some(4096)));
    }

    #[test]
    fn body_limit_positive_p4_vendor_raises_route_cap() {
        let _g = env_lock().lock().unwrap();
        std::env::set_var(MAX_REQUEST_BODY_ENV, "16777216");
        reset_max_inbound_body_cache_for_tests();
        assert_eq!(
            effective_inbound_body_limit(Some(5 * 1024 * 1024)),
            5 * 1024 * 1024
        );
        std::env::remove_var(MAX_REQUEST_BODY_ENV);
        reset_max_inbound_body_cache_for_tests();
    }

    #[test]
    fn body_limit_negative_n1_over_global_cl() {
        assert!(body_exceeds_limit(1001, 1000));
        assert_eq!(content_length_for_limit(Some("1001"), 1000), Err(()));
    }

    #[test]
    fn body_limit_negative_n2_over_route_cap() {
        let _g = env_lock().lock().unwrap();
        std::env::set_var(MAX_REQUEST_BODY_ENV, "10000");
        reset_max_inbound_body_cache_for_tests();
        assert_eq!(effective_inbound_body_limit(Some(100)), 100);
        assert!(body_exceeds_limit(
            101,
            effective_inbound_body_limit(Some(100))
        ));
        std::env::remove_var(MAX_REQUEST_BODY_ENV);
        reset_max_inbound_body_cache_for_tests();
    }

    #[test]
    fn body_limit_negative_n4_hostile_huge_cl() {
        assert_eq!(
            content_length_for_limit(Some("9999999999999999999"), 1024),
            Err(())
        );
        assert_eq!(
            content_length_for_limit(Some("not-a-number"), 1024),
            Err(())
        );
    }

    #[test]
    fn body_limit_negative_n6_error_json_shape() {
        let v = body_too_large_json("too big");
        assert_eq!(v["title"], "Payload Too Large");
        assert_eq!(v["reason"], REASON_REQUEST_BODY_TOO_LARGE);
        assert_eq!(v["detail"], "too big");
        assert_eq!(v["status"], 413);
        // Legacy aliases
        assert_eq!(v["message"], "too big");
    }

    #[test]
    fn body_limit_error_status_maps_413() {
        assert_eq!(body_limit_error_status(REQUEST_BODY_TOO_LARGE), Some(413));
        assert_eq!(body_limit_error_status("other"), None);
    }
}
