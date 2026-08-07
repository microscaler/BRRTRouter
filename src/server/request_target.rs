//! Request-target boundary helpers (Story 10.11).
//!
//! may_minihttp exposes `httparse`'s request-target via `Request::path()`.
//! See `docs/EPICS/URI_REQUEST_TARGET/request-line-boundary.md`.

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
}
