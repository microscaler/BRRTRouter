//! Why credentials are refused, and whether the JWKS keyset is fresh: counters for `/metrics`.
//!
//! # Refusal reasons
//!
//! A provider that refuses a credential records why with [`set_refusal_reason`] (for example `token_expired`,
//! `key_not_found`, `invalid_audience`). The router takes that reason once per refused request with
//! [`take_refusal_reason`], counts it in `brrtrouter_auth_failures_by_reason_total{reason}` and puts it in its
//! `Authentication failed` log line. The slot is coroutine-local, so a reason never leaks into another request,
//! and the router's second, scope-probing validation of the same request overwrites the slot rather than
//! counting twice.
//!
//! # JWKS
//!
//! `brrtrouter_jwks_refresh_total{outcome}`, `brrtrouter_jwks_keys` and
//! `brrtrouter_jwks_last_success_timestamp_seconds` say whether consumers can still validate a newly rotated key.
//! A process has one JWKS bearer provider in practice; with several, the gauges show the last one refreshed.
//!
//! Reasons are fixed `snake_case` tokens chosen by code, never request data, so the label set stays small.

use std::cell::Cell;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

may::coroutine_local!(static REFUSAL: Cell<Option<&'static str>> = Cell::new(None));

static REFUSALS: Mutex<Vec<(&'static str, u64)>> = Mutex::new(Vec::new());
static JWKS_REFRESH_OK: AtomicU64 = AtomicU64::new(0);
static JWKS_REFRESH_FAILED: AtomicU64 = AtomicU64::new(0);
static JWKS_KEYS: AtomicU64 = AtomicU64::new(0);
static JWKS_LAST_SUCCESS: AtomicU64 = AtomicU64::new(0);

/// Record why the current request's credential was refused. Use a fixed `snake_case` token.
pub fn set_refusal_reason(reason: &'static str) {
    REFUSAL.with(|slot| slot.set(Some(reason)));
}

/// Take, and clear, the reason recorded for the current request.
#[must_use]
pub fn take_refusal_reason() -> Option<&'static str> {
    REFUSAL.with(Cell::take)
}

/// Count one refused request under `reason`.
pub fn count_refusal(reason: &'static str) {
    let mut refusals = REFUSALS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match refusals.iter_mut().find(|(r, _)| *r == reason) {
        Some((_, n)) => *n += 1,
        None => refusals.push((reason, 1)),
    }
}

/// Refusal counts so far, by reason.
#[must_use]
pub fn refusals() -> Vec<(&'static str, u64)> {
    REFUSALS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

pub(crate) fn jwks_refresh_succeeded(keys: usize) {
    JWKS_REFRESH_OK.fetch_add(1, Ordering::Relaxed);
    JWKS_KEYS.store(keys as u64, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    JWKS_LAST_SUCCESS.store(now, Ordering::Relaxed);
}

pub(crate) fn jwks_refresh_failed() {
    JWKS_REFRESH_FAILED.fetch_add(1, Ordering::Relaxed);
}

/// Append the refusal and JWKS series to a Prometheus text body.
pub fn render_prometheus(body: &mut String) {
    body.push_str(
        "# HELP brrtrouter_auth_failures_by_reason_total Refused credentials by reason (token_expired, key_not_found, invalid_audience, insufficient_scope, ...)\n",
    );
    body.push_str("# TYPE brrtrouter_auth_failures_by_reason_total counter\n");
    for (reason, n) in refusals() {
        let _ = writeln!(
            body,
            "brrtrouter_auth_failures_by_reason_total{{reason=\"{reason}\"}} {n}"
        );
    }
    let ok = JWKS_REFRESH_OK.load(Ordering::Relaxed);
    let failed = JWKS_REFRESH_FAILED.load(Ordering::Relaxed);
    if ok + failed == 0 {
        return; // no JWKS provider in this process
    }
    body.push_str("# HELP brrtrouter_jwks_refresh_total JWKS refreshes by outcome\n");
    body.push_str("# TYPE brrtrouter_jwks_refresh_total counter\n");
    let _ = writeln!(body, "brrtrouter_jwks_refresh_total{{outcome=\"ok\"}} {ok}");
    let _ = writeln!(
        body,
        "brrtrouter_jwks_refresh_total{{outcome=\"failed\"}} {failed}"
    );
    body.push_str("# HELP brrtrouter_jwks_keys Usable keys in the last JWKS document loaded\n");
    body.push_str("# TYPE brrtrouter_jwks_keys gauge\n");
    let _ = writeln!(
        body,
        "brrtrouter_jwks_keys {}",
        JWKS_KEYS.load(Ordering::Relaxed)
    );
    body.push_str(
        "# HELP brrtrouter_jwks_last_success_timestamp_seconds Unix time of the last successful JWKS refresh\n",
    );
    body.push_str("# TYPE brrtrouter_jwks_last_success_timestamp_seconds gauge\n");
    let _ = writeln!(
        body,
        "brrtrouter_jwks_last_success_timestamp_seconds {}",
        JWKS_LAST_SUCCESS.load(Ordering::Relaxed)
    );
}

#[cfg(test)]
mod tests {
    //! The refusal slot is take-once, counts accumulate per reason, and the rendered text carries both the
    //! refusal and JWKS series once a refresh has happened.
    use super::*;

    #[test]
    fn refusal_slot_is_taken_once() {
        set_refusal_reason("token_expired");
        set_refusal_reason("token_expired"); // the scope probe overwrites, it does not add
        assert_eq!(take_refusal_reason(), Some("token_expired"));
        assert_eq!(take_refusal_reason(), None);
    }

    #[test]
    fn counts_and_renders_by_reason() {
        count_refusal("test_reason_a");
        count_refusal("test_reason_a");
        count_refusal("test_reason_b");
        let got = refusals();
        assert!(got.contains(&("test_reason_a", 2)));
        assert!(got.contains(&("test_reason_b", 1)));
        jwks_refresh_succeeded(2);
        let mut body = String::new();
        render_prometheus(&mut body);
        assert!(
            body.contains("brrtrouter_auth_failures_by_reason_total{reason=\"test_reason_a\"} 2")
        );
        assert!(body.contains("brrtrouter_jwks_keys 2"));
        assert!(body.contains("brrtrouter_jwks_refresh_total{outcome=\"ok\"}"));
    }
}
