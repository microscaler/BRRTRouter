//! Response validation mode (ADR 006 §1): `BRRTR_RESPONSE_VALIDATION=full|sample:<rate>|off`.
//!
//! - `full` (default): every response with a schema is checked; a mismatch answers 500 (the behaviour BRRTRouter has
//!   always had), is logged and counted.
//! - `sample:<rate>` (0 < rate ≤ 1): every `1/rate`-th response is checked; a mismatch is logged and counted and the
//!   response goes out unchanged.
//! - `off`: nothing is checked.
//!
//! An operation with `x-brrtrouter-validate-response: true` is always checked in `full`. Counters, rendered on
//! `/metrics`: `brrtrouter_response_validation_total{handler,result="ok"|"mismatch"}` for checked responses and
//! `brrtrouter_response_validation_skipped_total{handler}` for responses with a schema the mode did not check.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, OnceLock};

/// How responses are checked against their OpenAPI schema.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResponseValidation {
    /// Check every response; a mismatch is a 500.
    Full,
    /// Check this fraction of responses (0 < rate ≤ 1); a mismatch is logged and counted, not failed.
    Sample(f64),
    /// Check nothing.
    Off,
}

impl ResponseValidation {
    /// Parse `full`, `off` or `sample:<rate>`; `None` for anything else.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_ascii_lowercase();
        match s.as_str() {
            "full" | "on" | "" => Some(Self::Full),
            "off" | "none" => Some(Self::Off),
            _ => {
                let rate: f64 = s.strip_prefix("sample:")?.trim().parse().ok()?;
                if rate > 0.0 && rate <= 1.0 {
                    Some(if rate >= 1.0 {
                        Self::Full
                    } else {
                        Self::Sample(rate)
                    })
                } else if rate == 0.0 {
                    Some(Self::Off)
                } else {
                    None
                }
            }
        }
    }

    /// The process's mode, read once from `BRRTR_RESPONSE_VALIDATION` (default and fallback: `full`).
    #[must_use]
    pub fn global() -> Self {
        static MODE: OnceLock<ResponseValidation> = OnceLock::new();
        *MODE.get_or_init(|| match std::env::var("BRRTR_RESPONSE_VALIDATION") {
            Err(_) => Self::Full,
            Ok(v) => Self::parse(&v).unwrap_or_else(|| {
                tracing::warn!(
                    value = %v,
                    "BRRTR_RESPONSE_VALIDATION is not full, off or sample:<rate>; using full"
                );
                Self::Full
            }),
        })
    }

    /// Whether this response of `handler` is checked. `Sample` keeps a count per handler and checks that handler's
    /// evenly spread responses (each time `ceil(n·rate)` steps, starting with its first): every route gets the rate,
    /// whatever the interleaving of routes, and there is no RNG on the hot path.
    #[must_use]
    pub fn should_check(self, handler: &Arc<str>) -> bool {
        match self {
            Self::Full => true,
            Self::Off => false,
            Self::Sample(rate) => {
                let n = {
                    let mut m = SAMPLE_COUNTS
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    let c = m.entry(Arc::clone(handler)).or_insert(0);
                    let n = *c;
                    *c += 1;
                    n
                } as f64;
                ((n + 1.0) * rate).ceil() > (n * rate).ceil()
            }
        }
    }

    /// Whether a mismatch fails the request (500) rather than being logged and counted only.
    #[must_use]
    pub fn enforces(self) -> bool {
        matches!(self, Self::Full)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    Ok,
    Mismatch,
    Skipped,
}

static COUNTS: Mutex<BTreeMap<(Arc<str>, Outcome), u64>> = Mutex::new(BTreeMap::new());

/// Responses seen per handler, for `Sample`.
static SAMPLE_COUNTS: Mutex<BTreeMap<Arc<str>, u64>> = Mutex::new(BTreeMap::new());

fn bump(handler: &Arc<str>, outcome: Outcome) {
    let mut m = COUNTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *m.entry((Arc::clone(handler), outcome)).or_insert(0) += 1;
}

/// A checked response matched its schema.
pub fn record_ok(handler: &Arc<str>) {
    bump(handler, Outcome::Ok);
}

/// A checked response did not match its schema.
pub fn record_mismatch(handler: &Arc<str>) {
    bump(handler, Outcome::Mismatch);
}

/// A response with a schema that the mode did not check.
pub fn record_skipped(handler: &Arc<str>) {
    bump(handler, Outcome::Skipped);
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Append the counters in Prometheus text format (nothing before the first response with a schema).
pub fn render_prometheus(body: &mut String) {
    let m = COUNTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if m.is_empty() {
        return;
    }
    body.push_str("# HELP brrtrouter_response_validation_total Responses checked against their OpenAPI schema\n");
    body.push_str("# TYPE brrtrouter_response_validation_total counter\n");
    for ((h, o), n) in m.iter() {
        let result = match o {
            Outcome::Ok => "ok",
            Outcome::Mismatch => "mismatch",
            Outcome::Skipped => continue,
        };
        let _ = writeln!(
            body,
            "brrtrouter_response_validation_total{{handler=\"{}\",result=\"{result}\"}} {n}",
            esc(h)
        );
    }
    body.push_str("# HELP brrtrouter_response_validation_skipped_total Responses with a schema the validation mode did not check\n");
    body.push_str("# TYPE brrtrouter_response_validation_skipped_total counter\n");
    for ((h, o), n) in m.iter() {
        if *o == Outcome::Skipped {
            let _ = writeln!(
                body,
                "brrtrouter_response_validation_skipped_total{{handler=\"{}\"}} {n}",
                esc(h)
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_modes() {
        assert_eq!(
            ResponseValidation::parse("full"),
            Some(ResponseValidation::Full)
        );
        assert_eq!(
            ResponseValidation::parse(" OFF "),
            Some(ResponseValidation::Off)
        );
        assert_eq!(
            ResponseValidation::parse("sample:0.1"),
            Some(ResponseValidation::Sample(0.1))
        );
        assert_eq!(
            ResponseValidation::parse("sample:1"),
            Some(ResponseValidation::Full)
        );
        assert_eq!(
            ResponseValidation::parse("sample:0"),
            Some(ResponseValidation::Off)
        );
        assert_eq!(ResponseValidation::parse("sample:2"), None);
        assert_eq!(ResponseValidation::parse("sample:x"), None);
        assert_eq!(ResponseValidation::parse("sometimes"), None);
    }

    #[test]
    fn sample_checks_its_rate_for_every_handler() {
        let m = ResponseValidation::Sample(0.5);
        let (a, b): (Arc<str>, Arc<str>) = (Arc::from("sample_a"), Arc::from("sample_b"));
        // Strictly alternating routes: a single shared counter would check only one of them.
        let (mut ca, mut cb) = (0, 0);
        for _ in 0..1_000 {
            ca += usize::from(m.should_check(&a));
            cb += usize::from(m.should_check(&b));
        }
        assert_eq!((ca, cb), (500, 500));
        let q: Arc<str> = Arc::from("sample_q");
        let checked = (0..10_000)
            .filter(|_| ResponseValidation::Sample(0.25).should_check(&q))
            .count();
        assert_eq!(checked, 2_500);
        assert!(!m.enforces() && ResponseValidation::Full.enforces());
        assert!(!ResponseValidation::Off.should_check(&a));
    }

    #[test]
    fn renders_counts() {
        let h: Arc<str> = Arc::from("render_test_handler");
        record_ok(&h);
        record_mismatch(&h);
        record_skipped(&h);
        let mut out = String::new();
        render_prometheus(&mut out);
        assert!(out.contains(
            "brrtrouter_response_validation_total{handler=\"render_test_handler\",result=\"ok\"} 1"
        ));
        assert!(out.contains(
            "brrtrouter_response_validation_total{handler=\"render_test_handler\",result=\"mismatch\"} 1"
        ));
        assert!(out.contains(
            "brrtrouter_response_validation_skipped_total{handler=\"render_test_handler\"} 1"
        ));
    }
}
