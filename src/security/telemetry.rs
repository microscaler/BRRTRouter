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
use std::time::{SystemTime, UNIX_EPOCH};

may::coroutine_local!(static REFUSAL: Cell<Option<&'static str>> = Cell::new(None));

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

/// Every reason the router and the bundled providers record; anything else counts as `other`.
pub const REASONS: [&str; 23] = [
    "missing_token",
    "invalid_token_format",
    "missing_kid",
    "key_not_found",
    "invalid_signature",
    "token_expired",
    "invalid_issuer",
    "invalid_audience",
    "missing_required_claim",
    "unsupported_algorithm",
    "invalid_token_type",
    "jwks_fetch_error",
    "insufficient_scope",
    "token_revoked",
    "stale_token_version",
    "token_status_unavailable",
    "invalid_token_status_claims",
    "invalid_security_scheme",
    "scheme_undefined",
    "provider_missing",
    "invalid_credentials",
    "invalid_api_key",
    "other",
];

/// Per-service refusal counts, one atomic per [`REASONS`] entry: no lock on the refusal path.
#[derive(Debug)]
pub struct RefusalCounters([AtomicU64; REASONS.len()]);

impl Default for RefusalCounters {
    fn default() -> Self {
        Self(std::array::from_fn(|_| AtomicU64::new(0)))
    }
}

impl RefusalCounters {
    /// Count one refused request under `reason` (`other` when it is not in [`REASONS`]).
    pub fn count(&self, reason: &str) {
        let i = REASONS
            .iter()
            .position(|r| *r == reason)
            .unwrap_or(REASONS.len() - 1);
        self.0[i].fetch_add(1, Ordering::Relaxed);
    }

    /// Non-zero counts, by reason.
    #[must_use]
    pub fn snapshot(&self) -> Vec<(&'static str, u64)> {
        REASONS
            .iter()
            .zip(self.0.iter())
            .map(|(r, n)| (*r, n.load(Ordering::Relaxed)))
            .filter(|(_, n)| *n > 0)
            .collect()
    }

    /// Append `brrtrouter_auth_failures_by_reason_total` to a Prometheus text body.
    pub fn render_prometheus(&self, body: &mut String) {
        body.push_str(
            "# HELP brrtrouter_auth_failures_by_reason_total Refused credentials by reason (token_expired, key_not_found, invalid_audience, insufficient_scope, ...)\n",
        );
        body.push_str("# TYPE brrtrouter_auth_failures_by_reason_total counter\n");
        for (reason, n) in self.snapshot() {
            let _ = writeln!(
                body,
                "brrtrouter_auth_failures_by_reason_total{{reason=\"{reason}\"}} {n}"
            );
        }
    }
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

/// Append the JWKS series to a Prometheus text body (nothing until a JWKS provider has refreshed).
pub fn render_jwks_prometheus(body: &mut String) {
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
    //! The refusal slot is take-once; counters are per instance and fold unknown reasons into `other`; the JWKS
    //! series render once a refresh has happened.
    use super::*;

    #[test]
    fn refusal_slot_is_taken_once() {
        set_refusal_reason("token_expired");
        set_refusal_reason("token_expired"); // the scope probe overwrites, it does not add
        assert_eq!(take_refusal_reason(), Some("token_expired"));
        assert_eq!(take_refusal_reason(), None);
    }

    #[test]
    fn counts_per_service_by_reason() {
        let a = RefusalCounters::default();
        let b = RefusalCounters::default();
        a.count("token_expired");
        a.count("token_expired");
        a.count("something_custom");
        b.count("key_not_found");
        assert_eq!(a.snapshot(), vec![("token_expired", 2), ("other", 1)]);
        assert_eq!(b.snapshot(), vec![("key_not_found", 1)]);
        let mut body = String::new();
        a.render_prometheus(&mut body);
        assert!(
            body.contains("brrtrouter_auth_failures_by_reason_total{reason=\"token_expired\"} 2")
        );
        assert!(!body.contains("reason=\"key_not_found\""));
    }

    #[test]
    fn renders_jwks_after_a_refresh() {
        jwks_refresh_succeeded(2);
        let mut body = String::new();
        render_jwks_prometheus(&mut body);
        assert!(body.contains("brrtrouter_jwks_keys 2"));
        assert!(body.contains("brrtrouter_jwks_refresh_total{outcome=\"ok\"}"));
    }
}
