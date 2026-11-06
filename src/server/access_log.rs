//! Telemetry toggles for the per-request record: the INFO access log ("Request completed") and the `http_request`
//! span. Defaults keep the full record; a service opts into the cheaper modes by environment.
//!
//! `BRRTR_ACCESS_LOG=full|sample:<rate>|off` (the grammar of `BRRTR_RESPONSE_VALIDATION`):
//!
//! - `full` (default): every request is logged with its redacted request and response headers.
//! - `sample:<rate>` (0 < rate ≤ 1): evenly spread requests at that rate are logged in full, decided when the request
//!   arrives (a process-wide count, no RNG). The others skip both header redaction passes and their JSON formatting.
//! - `off`: no request is logged in full.
//!
//! Whatever the mode, a request that answers 5xx or 429 (our own back-pressure), or takes at least
//! `BRRTR_ACCESS_LOG_SLOW_MS` (default 100) milliseconds, is always logged: failures and slow requests never disappear
//! from the log. When such a request was not sampled its headers were never captured, so the event carries
//! `sampled = false` and no header fields. Other 4xx are not forced: scanners' 401s and 404s would swamp it.
//!
//! `BRRTR_REQUEST_SPAN=full|lean`: `lean` drops the span fields another field already carries or the access log
//! records (`method` and `path`, duplicated by `http.request.method` and `http.route`; `header_count`;
//! `stack_used_kb`). It keeps the OpenTelemetry HTTP fields, `tenant_id` (when the access log is sampled the span is
//! the only per-request record left, and traces are filtered by tenant first), `status`, `duration_ms` and the
//! `brrtrouter.*` failure reasons. The router always creates the span: whether it is exported is the trace
//! exporter's sampler's decision, not the router's.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

/// Which requests the access log records in full (with headers).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccessLog {
    /// Every request, with headers (the default).
    Full,
    /// This fraction of requests (0 < rate < 1), with headers; the rest only when forced ([`must_log`]).
    Sample(f64),
    /// No request in full; forced requests only.
    Off,
}

impl AccessLog {
    /// Parse `full`, `off` or `sample:<rate>`; `None` for anything else.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_ascii_lowercase();
        match s.as_str() {
            "full" | "on" | "" => Some(Self::Full),
            "off" | "none" => Some(Self::Off),
            _ => {
                let rate: f64 = s.strip_prefix("sample:")?.trim().parse().ok()?;
                if !rate.is_finite() {
                    None
                } else if rate >= 1.0 {
                    (rate == 1.0).then_some(Self::Full)
                } else if rate > 0.0 {
                    Some(Self::Sample(rate))
                } else if rate == 0.0 {
                    Some(Self::Off)
                } else {
                    None
                }
            }
        }
    }

    /// The process's mode, read once from `BRRTR_ACCESS_LOG` (default and fallback: `full`).
    #[must_use]
    pub fn global() -> Self {
        static MODE: OnceLock<AccessLog> = OnceLock::new();
        *MODE.get_or_init(|| match std::env::var("BRRTR_ACCESS_LOG") {
            Err(_) => Self::Full,
            Ok(v) => Self::parse(&v).unwrap_or_else(|| {
                tracing::warn!(value = %v, "BRRTR_ACCESS_LOG is not full, off or sample:<rate>; using full");
                Self::Full
            }),
        })
    }

    /// Whether the request now arriving for `key` (its path) is recorded in full. `Sample` takes evenly spread
    /// requests (each time `ceil(n·rate)` steps, starting with the first) from one of [`BUCKETS`] counts, chosen by
    /// the key's hash: one process-wide count would let a steady interleaving of routes leave a route out entirely
    /// (A, B, A, B at `sample:0.5` logs every A and no B). The counts are fixed, so memory does not grow with paths.
    #[must_use]
    pub fn sample_next(self, key: &str) -> bool {
        static SEEN: [AtomicU64; BUCKETS] = [const { AtomicU64::new(0) }; BUCKETS];
        match self {
            Self::Full => true,
            Self::Off => false,
            Self::Sample(rate) => takes(SEEN[bucket(key)].fetch_add(1, Ordering::Relaxed), rate),
        }
    }
}

/// How many independent sampling counts [`AccessLog::sample_next`] keeps.
pub const BUCKETS: usize = 64;

/// The count a key samples from: FNV-1a of its bytes (no allocation, stable across runs).
fn bucket(key: &str) -> usize {
    let h = key.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    (h % BUCKETS as u64) as usize
}

/// Whether the `n`th request (from 0) is one of the evenly spread `rate` share.
fn takes(n: u64, rate: f64) -> bool {
    let n = n as f64;
    ((n + 1.0) * rate).ceil() > (n * rate).ceil()
}

/// Requests at least this slow are always logged: `BRRTR_ACCESS_LOG_SLOW_MS`, default 100 (read once).
#[must_use]
pub fn slow_ms() -> u64 {
    static SLOW: OnceLock<u64> = OnceLock::new();
    *SLOW.get_or_init(|| {
        std::env::var("BRRTR_ACCESS_LOG_SLOW_MS")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(100)
    })
}

/// Whether a request that was not sampled must still be logged: it answered 5xx or 429, or took at least `slow_ms`.
/// A request that never got a status (the connection closed first) is forced only by its duration.
#[must_use]
pub fn must_log(status: Option<u16>, duration_ms: u64, slow_ms: u64) -> bool {
    matches!(status, Some(s) if s >= 500 || s == 429) || duration_ms >= slow_ms
}

/// Which fields the `http_request` span carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestSpan {
    /// Every field (the default).
    Full,
    /// Without the fields another field or the access log already carries (see the module docs).
    Lean,
}

impl RequestSpan {
    /// Parse `full` or `lean`; `None` for anything else.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "full" | "" => Some(Self::Full),
            "lean" => Some(Self::Lean),
            _ => None,
        }
    }

    /// The process's mode, read once from `BRRTR_REQUEST_SPAN` (default and fallback: `full`).
    #[must_use]
    pub fn global() -> Self {
        static MODE: OnceLock<RequestSpan> = OnceLock::new();
        *MODE.get_or_init(|| match std::env::var("BRRTR_REQUEST_SPAN") {
            Err(_) => Self::Full,
            Ok(v) => Self::parse(&v).unwrap_or_else(|| {
                tracing::warn!(value = %v, "BRRTR_REQUEST_SPAN is not full or lean; using full");
                Self::Full
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_access_log_modes() {
        assert_eq!(AccessLog::parse("full"), Some(AccessLog::Full));
        assert_eq!(AccessLog::parse(" FULL "), Some(AccessLog::Full));
        assert_eq!(AccessLog::parse(""), Some(AccessLog::Full));
        assert_eq!(AccessLog::parse("off"), Some(AccessLog::Off));
        assert_eq!(AccessLog::parse("sample:0.1"), Some(AccessLog::Sample(0.1)));
        assert_eq!(
            AccessLog::parse("sample: 0.25"),
            Some(AccessLog::Sample(0.25))
        );
        assert_eq!(AccessLog::parse("sample:1"), Some(AccessLog::Full));
        assert_eq!(AccessLog::parse("sample:0"), Some(AccessLog::Off));
        for bad in [
            "sample:1.5",
            "sample:-0.1",
            "sample:x",
            "sample:NaN",
            "sample:inf",
            "some",
            "sample",
        ] {
            assert_eq!(AccessLog::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn sampling_takes_evenly_spread_requests_at_the_rate() {
        for (rate, n, want) in [
            (0.1, 1000, 100),
            (0.25, 400, 100),
            (0.5, 10, 5),
            (0.01, 1000, 10),
        ] {
            let taken: Vec<u64> = (0..n).filter(|&i| takes(i, rate)).collect();
            assert_eq!(taken.len(), want, "rate {rate}");
            assert_eq!(taken[0], 0, "the first request is always taken");
        }
        // Evenly spread: at 0.1, exactly one in every ten consecutive requests.
        assert!((0..100)
            .step_by(10)
            .all(|s| (s..s + 10).filter(|&i| takes(i, 0.1)).count() == 1));
        assert!(AccessLog::Full.sample_next("/a"));
        assert!(!AccessLog::Off.sample_next("/a"));
    }

    #[test]
    fn interleaved_routes_are_each_sampled_at_the_rate() {
        // Two paths in different buckets, alternating: one process-wide count at 0.5 would take every request
        // of one and none of the other.
        let (a, b) = ("/interleave-a", "/interleave-b");
        assert_ne!(bucket(a), bucket(b));
        let mode = AccessLog::Sample(0.5);
        let (mut got_a, mut got_b) = (0, 0);
        for _ in 0..20 {
            got_a += usize::from(mode.sample_next(a));
            got_b += usize::from(mode.sample_next(b));
        }
        assert_eq!((got_a, got_b), (10, 10));
    }

    #[test]
    fn failures_back_pressure_and_slow_requests_are_always_logged() {
        assert!(must_log(Some(500), 1, 100));
        assert!(must_log(Some(503), 1, 100));
        assert!(must_log(Some(429), 1, 100));
        assert!(must_log(Some(200), 100, 100));
        assert!(must_log(None, 250, 100));
        assert!(!must_log(Some(200), 99, 100));
        assert!(!must_log(Some(401), 3, 100));
        assert!(!must_log(Some(404), 3, 100));
        assert!(!must_log(None, 3, 100));
    }

    #[test]
    fn parses_the_span_modes() {
        assert_eq!(RequestSpan::parse("full"), Some(RequestSpan::Full));
        assert_eq!(RequestSpan::parse("Lean"), Some(RequestSpan::Lean));
        assert_eq!(RequestSpan::parse(""), Some(RequestSpan::Full));
        assert_eq!(RequestSpan::parse("minimal"), None);
    }
}
