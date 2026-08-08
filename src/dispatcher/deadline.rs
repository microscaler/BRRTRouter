//! Handler / request deadline policy (Epic 13.6).
//!
//! When enabled, the dispatcher waits for a handler reply with
//! [`may::sync::mpsc::Receiver::recv_timeout`]. On timeout the client receives
//! **504** and the late reply (if any) is dropped when the reply channel is
//! closed — best-effort; we do not cancel user CPU mid-flight.
//!
//! ## Config
//!
//! - Env: `BRRTR_HANDLER_DEADLINE_MS` — `0` or unset → disabled (legacy wait).
//! - `AppConfig.http.handler_deadline_ms` (same semantics).
//! - Per-route OpenAPI `x-brrtrouter-deadline-ms` — see [`resolve_deadline`].
//!
//! ## Global ceiling
//!
//! When a global deadline is set, it is a **ceiling**: a route override may only
//! shorten it. When global is disabled, a route override applies as an absolute
//! deadline for that route.

use std::time::Duration;

/// Env var for the global handler wait deadline (milliseconds).
pub const HANDLER_DEADLINE_ENV: &str = "BRRTR_HANDLER_DEADLINE_MS";

/// Stable reason / problem extension for deadline 504s.
pub const REASON_HANDLER_DEADLINE_EXCEEDED: &str = "handler_deadline_exceeded";

/// Human detail for timeout responses (no stack traces / secrets).
pub const HANDLER_DEADLINE_DETAIL: &str = "Handler deadline exceeded";

/// Resolve effective deadline from global + optional per-route override.
///
/// - `0` ms values are treated as **disabled** (None).
/// - With global set, route can only **shorten** (`min`).
/// - With global unset, route alone enables a deadline for that route.
#[must_use]
pub fn resolve_deadline(global_ms: Option<u64>, route_ms: Option<u64>) -> Option<Duration> {
    let global = nonzero_ms(global_ms);
    let route = nonzero_ms(route_ms);
    match (global, route) {
        (None, None) => None,
        (Some(g), None) => Some(g),
        (None, Some(r)) => Some(r),
        (Some(g), Some(r)) => Some(r.min(g)),
    }
}

fn nonzero_ms(ms: Option<u64>) -> Option<Duration> {
    match ms {
        Some(0) | None => None,
        Some(n) => Some(Duration::from_millis(n)),
    }
}

/// Read global deadline from the environment (`None` = disabled).
#[must_use]
pub fn handler_deadline_from_env() -> Option<u64> {
    match std::env::var(HANDLER_DEADLINE_ENV) {
        Ok(s) => match s.trim().parse::<u64>() {
            Ok(0) => None,
            Ok(n) => Some(n),
            Err(_) => None,
        },
        Err(_) => None,
    }
}

/// Build the 504 Problem Details / HandlerResponse for a deadline miss.
#[must_use]
pub fn deadline_exceeded_response() -> crate::dispatcher::HandlerResponse {
    crate::http::problem::Problem::new(
        crate::http::problem::TYPE_GATEWAY_TIMEOUT,
        "Gateway Timeout",
        504,
    )
    .detail(HANDLER_DEADLINE_DETAIL)
    .reason(REASON_HANDLER_DEADLINE_EXCEEDED)
    .into_handler_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_disabled_when_both_absent() {
        assert!(resolve_deadline(None, None).is_none());
        assert!(resolve_deadline(Some(0), Some(0)).is_none());
    }

    #[test]
    fn resolve_global_only() {
        assert_eq!(
            resolve_deadline(Some(1000), None),
            Some(Duration::from_millis(1000))
        );
    }

    #[test]
    fn resolve_route_only() {
        assert_eq!(
            resolve_deadline(None, Some(250)),
            Some(Duration::from_millis(250))
        );
    }

    #[test]
    fn resolve_route_cannot_exceed_global_ceiling() {
        assert_eq!(
            resolve_deadline(Some(100), Some(500)),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            resolve_deadline(Some(500), Some(100)),
            Some(Duration::from_millis(100))
        );
    }

    #[test]
    fn n2_zero_is_disabled() {
        assert!(resolve_deadline(Some(0), None).is_none());
    }

    #[test]
    fn deadline_response_is_504_problem() {
        let r = deadline_exceeded_response();
        assert_eq!(r.status, 504);
        assert_eq!(r.body["status"], 504);
        assert_eq!(r.body["reason"], REASON_HANDLER_DEADLINE_EXCEEDED);
        let s = r.body.to_string();
        assert!(!s.contains("stack"));
        assert!(!s.contains("secret"));
    }
}
