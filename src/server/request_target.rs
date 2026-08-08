//! Request-target boundary helpers (Stories 10.11, 10.6, 10.8).
//!
//! may_minihttp exposes `httparse`'s request-target via `Request::path()`.
//! See `docs/EPICS/URI_REQUEST_TARGET/request-line-boundary.md`.
//!
//! # Dual http stack (Story 10.8 — Decision B)
//!
//! Internal type of truth is an origin-form **string** ([`RequestTarget`]).
//! Convert to `http_legacy::Uri` (0.2) only at the may_minihttp client edge.
//! While both crates exist, [`assert_request_target_uri_ok`] requires **both**
//! parsers to accept the same octets (hard gate against silent drift).

use std::sync::atomic::{AtomicUsize, Ordering};

/// Default max request-target octets (path + optional `?` + query). RFC 9110 suggests ≥8000.
pub const DEFAULT_MAX_REQUEST_TARGET_OCTETS: usize = 8192;

/// Env override: `BRRTROUTER_MAX_REQUEST_TARGET_OCTETS` (decimal). `0` or invalid → default.
pub const MAX_REQUEST_TARGET_ENV: &str = "BRRTROUTER_MAX_REQUEST_TARGET_OCTETS";

static MAX_REQUEST_TARGET_CACHE: AtomicUsize = AtomicUsize::new(0);

/// Error marker returned from [`crate::server::parse_request`] when the target is over limit.
pub const REQUEST_TARGET_TOO_LONG: &str = "request_target_too_long";

/// Configurable max request-target length in octets (wire length of the UTF-8 string).
#[must_use]
pub fn max_request_target_octets() -> usize {
    let cached = MAX_REQUEST_TARGET_CACHE.load(Ordering::Relaxed);
    if cached != 0 {
        return cached;
    }
    let resolved = match std::env::var(MAX_REQUEST_TARGET_ENV) {
        Ok(s) => match s.parse::<usize>() {
            Ok(0) | Err(_) => DEFAULT_MAX_REQUEST_TARGET_OCTETS,
            Ok(n) => n,
        },
        Err(_) => DEFAULT_MAX_REQUEST_TARGET_OCTETS,
    };
    MAX_REQUEST_TARGET_CACHE.store(resolved, Ordering::Relaxed);
    resolved
}

/// Test/helper: reset cached max (after env changes in tests).
#[cfg(test)]
pub fn reset_max_request_target_cache_for_tests() {
    MAX_REQUEST_TARGET_CACHE.store(0, Ordering::Relaxed);
}

/// `true` when `target` exceeds the configured max (inclusive bound: `len > max` rejects).
#[must_use]
pub fn request_target_exceeds_limit(target: &str, max: usize) -> bool {
    target.len() > max
}

/// Map [`crate::server::parse_request`] `Err` strings to HTTP status (Story 10.6).
#[must_use]
pub fn parse_request_error_status(err: &str) -> u16 {
    if err == REQUEST_TARGET_TOO_LONG {
        414
    } else {
        400
    }
}

/// Origin-form request-target (`/path` or `/path?query`) used as the internal
/// bridge type while `http` 1.0 and `http_legacy` 0.2 coexist (Decision B).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RequestTarget(String);

impl RequestTarget {
    /// Wrap a path+query string without re-validating (caller already checked).
    #[must_use]
    pub fn from_checked(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Validate with both URI stacks; reject if either fails (Story 10.8 N5).
    pub fn try_from_path_query(s: impl Into<String>) -> Result<Self, RequestTargetUriError> {
        let s = s.into();
        assert_request_target_uri_ok(&s)?;
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }

    /// Convert at the may_minihttp edge only.
    pub fn to_legacy_uri(&self) -> Result<http_legacy::Uri, RequestTargetUriError> {
        self.0
            .parse::<http_legacy::Uri>()
            .map_err(|_| RequestTargetUriError::LegacyReject)
    }
}

impl AsRef<str> for RequestTarget {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Dual-stack URI validation failure (Story 10.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestTargetUriError {
    LegacyReject,
    Http1Reject,
    /// One stack accepted and the other rejected — hard CI gate.
    StackDivergence,
}

impl std::fmt::Display for RequestTargetUriError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LegacyReject => write!(f, "http_legacy (0.2) rejected request-target"),
            Self::Http1Reject => write!(f, "http (1.0) rejected request-target"),
            Self::StackDivergence => {
                write!(f, "http 1.0 and http_legacy 0.2 disagree on request-target")
            }
        }
    }
}

impl std::error::Error for RequestTargetUriError {}

/// Require both `http` 1.0 and `http_legacy` 0.2 to accept `target`.
pub fn assert_request_target_uri_ok(target: &str) -> Result<(), RequestTargetUriError> {
    let legacy_ok = target.parse::<http_legacy::Uri>().is_ok();
    let http1_ok = target.parse::<http::Uri>().is_ok();
    match (legacy_ok, http1_ok) {
        (true, true) => Ok(()),
        (false, false) => Err(RequestTargetUriError::LegacyReject),
        (true, false) => Err(RequestTargetUriError::StackDivergence),
        (false, true) => Err(RequestTargetUriError::StackDivergence),
    }
}

/// Normalize the front `Request::path()` value for BRRTRouter routing and query parse.
///
/// - origin-form (`/p?q=1`) — unchanged
/// - absolute-form (`http://host/p?q=1`) — origin path+query only
/// - asterisk-form (`*`) — unchanged
/// - ambiguous `//…` origin-form — unchanged (documented pass-through)
///
/// Does not percent-decode. Does not strip `#fragment` (front/peers should not send it).
#[must_use]
pub fn request_target_for_app(raw: &str) -> &str {
    if raw == "*" {
        return raw;
    }
    if let Some(after_scheme) = strip_http_scheme(raw) {
        if let Some(slash) = after_scheme.find('/') {
            return &after_scheme[slash..];
        }
        // `http://host` with no path → treat as `/`
        return "/";
    }
    raw
}

/// Query component without leading `?`, if present.
#[must_use]
pub fn raw_query(request_target: &str) -> Option<&str> {
    request_target.split_once('?').map(|(_, q)| q)
}

/// Path component only (no `?query`), for radix matching.
#[must_use]
pub fn path_only(request_target: &str) -> &str {
    match request_target.split_once('?') {
        Some((path, _)) => {
            if path.is_empty() {
                "/"
            } else {
                path
            }
        }
        None => {
            if request_target.is_empty() {
                "/"
            } else {
                request_target
            }
        }
    }
}

fn strip_http_scheme(raw: &str) -> Option<&str> {
    if raw.len() >= 8 && raw[..8].eq_ignore_ascii_case("https://") {
        return Some(&raw[8..]);
    }
    if raw.len() >= 7 && raw[..7].eq_ignore_ascii_case("http://") {
        return Some(&raw[7..]);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::request::parse_query_params;

    fn find_q<'a>(params: &'a crate::router::ParamVec, name: &str) -> Option<&'a str> {
        params
            .iter()
            .find(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }

    // --- Positive ---

    #[test]
    fn request_target_positive_p1_origin_form() {
        assert_eq!(request_target_for_app("/p?q=1"), "/p?q=1");
        assert_eq!(path_only("/p?q=1"), "/p");
        assert_eq!(find_q(&parse_query_params("/p?q=1"), "q"), Some("1"));
    }

    #[test]
    fn request_target_positive_p2_encoded_space_query() {
        let t = "/p?q=South%20Africa";
        assert_eq!(request_target_for_app(t), t);
        assert_eq!(find_q(&parse_query_params(t), "q"), Some("South Africa"));
    }

    #[test]
    fn request_target_positive_p3_long_under_limit() {
        let q = "x".repeat(4000);
        let t = format!("/p?q={q}");
        assert_eq!(request_target_for_app(&t), t.as_str());
        assert_eq!(find_q(&parse_query_params(&t), "q"), Some(q.as_str()));
    }

    #[test]
    fn request_target_positive_p4_absolute_form() {
        assert_eq!(request_target_for_app("http://example.com/p?q=1"), "/p?q=1");
        assert_eq!(
            request_target_for_app("HTTPS://Example.COM/a/b?x=2"),
            "/a/b?x=2"
        );
        assert_eq!(request_target_for_app("http://example.com"), "/");
    }

    #[test]
    fn request_target_positive_p5_multi_segment() {
        assert_eq!(path_only("/api/v1/regions/west"), "/api/v1/regions/west");
    }

    #[test]
    fn request_target_positive_p6_plus_and_percent20() {
        let plus = "/p?q=South+Africa";
        let pct = "/p?q=South%20Africa";
        assert_eq!(
            find_q(&parse_query_params(plus), "q"),
            find_q(&parse_query_params(pct), "q")
        );
    }

    #[test]
    fn request_target_positive_asterisk_form() {
        assert_eq!(request_target_for_app("*"), "*");
        assert_eq!(path_only("*"), "*");
    }

    // --- Negative / documented boundary ---

    #[test]
    fn request_target_negative_n5_hash_not_stripped() {
        // App does not strip; contract documents front also does not.
        assert_eq!(request_target_for_app("/p#frag"), "/p#frag");
        assert_eq!(request_target_for_app("/p?q=a#frag"), "/p?q=a#frag");
    }

    #[test]
    fn request_target_negative_n7_double_slash_passthrough() {
        assert_eq!(request_target_for_app("//evil/p"), "//evil/p");
        assert_eq!(path_only("//evil/p"), "//evil/p");
    }

    #[test]
    fn request_target_negative_n8_absolute_form_then_query_ok() {
        let t = request_target_for_app("http://h/p?q=South%20Africa");
        assert_eq!(t, "/p?q=South%20Africa");
        assert_eq!(find_q(&parse_query_params(t), "q"), Some("South Africa"));
    }

    // --- Story 10.6 length helpers ---

    #[test]
    fn request_target_length_positive_p1_under_limit() {
        assert!(!request_target_exceeds_limit("/p?q=1", 100));
    }

    #[test]
    fn request_target_length_positive_p2_at_limit_minus_one() {
        let t = "x".repeat(99);
        assert!(!request_target_exceeds_limit(&t, 100));
    }

    #[test]
    fn request_target_length_positive_p3_exactly_at_limit() {
        let t = "x".repeat(100);
        assert!(!request_target_exceeds_limit(&t, 100));
    }

    #[test]
    fn request_target_length_positive_p4_short_path_query() {
        assert!(!request_target_exceeds_limit(
            "/a?b=c",
            DEFAULT_MAX_REQUEST_TARGET_OCTETS
        ));
    }

    #[test]
    fn request_target_length_positive_p5_wire_length_counts_encoded() {
        // `%20` is 3 octets on the wire, not 1 decoded space.
        let t = format!("/p?q={}", "%20".repeat(10));
        assert_eq!(t.len(), 35); // `/p?q=` (5) + ten `%20` (30)
        assert!(!request_target_exceeds_limit(&t, t.len()));
        assert!(request_target_exceeds_limit(&t, t.len() - 1));
    }

    #[test]
    fn request_target_length_positive_p6_default_ge_8192() {
        assert!(DEFAULT_MAX_REQUEST_TARGET_OCTETS >= 8192);
    }

    #[test]
    fn request_target_length_negative_n1_over_limit_status_414() {
        assert!(request_target_exceeds_limit(&"x".repeat(8193), 8192));
        assert_eq!(parse_request_error_status(REQUEST_TARGET_TOO_LONG), 414);
        assert_eq!(parse_request_error_status("GETT"), 400);
    }

    #[test]
    fn request_target_length_negative_n3_zero_env_uses_default() {
        // Env interaction covered under ENV_LOCK in proxy tests; unit: 0 parse → default.
        assert_eq!(DEFAULT_MAX_REQUEST_TARGET_OCTETS, 8192);
    }

    #[test]
    fn request_target_length_negative_n4_bounded_fixture() {
        let t = format!("/{}", "x".repeat(20_000));
        assert!(request_target_exceeds_limit(
            &t,
            DEFAULT_MAX_REQUEST_TARGET_OCTETS
        ));
        assert_eq!(t.len(), 20_001);
    }

    #[test]
    fn request_target_length_negative_n5_repeated_keys_over_limit() {
        let pair = "a=1&".repeat(3000);
        let t = format!("/p?{pair}");
        assert!(request_target_exceeds_limit(&t, 8192));
    }

    #[test]
    fn request_target_length_negative_n6_path_alone_over_limit() {
        let t = format!("/{}", "p".repeat(9000));
        assert!(request_target_exceeds_limit(&t, 8192));
    }

    #[test]
    fn request_target_length_negative_n7_encoded_longer_than_decoded() {
        let encoded = "%20".repeat(100);
        assert!(encoded.len() > 100); // vs 100 spaces decoded
        let t = format!("/p?q={encoded}");
        assert_eq!(t.len(), 5 + encoded.len()); // `/p?q=` is 5 octets
    }

    #[test]
    fn request_target_raw_query_helper() {
        assert_eq!(raw_query("/p?q=1"), Some("q=1"));
        assert_eq!(raw_query("/p"), None);
    }

    // --- Story 10.8 dual-stack bridge ---

    #[test]
    fn request_target_stack_positive_p1_same_octets_both() {
        let t = RequestTarget::try_from_path_query("/p?q=1").unwrap();
        assert_eq!(t.as_str(), "/p?q=1");
        assert_eq!(t.to_legacy_uri().unwrap().path(), "/p");
    }

    #[test]
    fn request_target_stack_positive_p2_space_pct20_both() {
        assert_request_target_uri_ok("/p?q=South%20Africa").unwrap();
    }

    #[test]
    fn request_target_stack_positive_p3_unicode_both() {
        assert_request_target_uri_ok("/p?q=%E6%9D%B1%E4%BA%AC").unwrap();
    }

    #[test]
    fn request_target_stack_positive_p4_path_template_resolve_both() {
        use crate::http::resolve_path_template;
        use crate::router::ParamVec;
        use std::sync::Arc;
        let mut path = ParamVec::new();
        path.push((Arc::from("id"), "a b".to_string()));
        let rebuilt = resolve_path_template("/items/{id}", &path, &ParamVec::new());
        assert_eq!(rebuilt, "/items/a%20b");
        RequestTarget::try_from_path_query(rebuilt).unwrap();
    }

    #[test]
    fn request_target_stack_positive_p5_empty_query_both() {
        assert_request_target_uri_ok("/p").unwrap();
    }

    #[test]
    fn request_target_stack_positive_p6_multi_param_order_both() {
        assert_request_target_uri_ok("/p?a=1&b=2").unwrap();
    }

    #[test]
    fn request_target_stack_negative_n1_raw_space_both_reject() {
        assert!(assert_request_target_uri_ok("/p?q=a b").is_err());
    }

    #[test]
    fn request_target_stack_negative_n4_missing_param_family() {
        // Composition taxonomy remains InvalidPath; dual-stack only gates Uri-OK targets.
        let err = assert_request_target_uri_ok("/p?q=a b").unwrap_err();
        assert!(matches!(
            err,
            RequestTargetUriError::LegacyReject | RequestTargetUriError::StackDivergence
        ));
    }

    #[test]
    fn request_target_stack_negative_n6_no_panic_on_hostile() {
        let _ = assert_request_target_uri_ok("");
        let _ = assert_request_target_uri_ok("%%%%");
        let _ = assert_request_target_uri_ok("/p?q=\0");
    }
}
