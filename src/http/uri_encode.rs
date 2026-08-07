//! Component-specific URI encoders for outbound request-target rebuild (Story 10.4).
//!
//! # Decode / encode asymmetry
//!
//! | Direction | Mechanism | Space |
//! |-----------|-----------|-------|
//! | **Inbound query** | WHATWG form-urlencoded (`parse_query_params`) | `+` and `%20` → space |
//! | **Outbound rebuild** | these encoders | space → **`%20` only** (never `+`) |
//!
//! Path segments use RFC 3986 percent-encoding (not form-urlencoded): a logical
//! `+` encodes as `%2B`, never as a space.
//!
//! # Unreserved set (RFC 3986 §2.3)
//!
//! `A-Z a-z 0-9 - . _ ~` are left unencoded. All other octets (including
//! reserved delimiters `& = ? # /` and UTF-8) are percent-encoded.
//!
//! Prefer these APIs over calling `urlencoding::encode` directly from proxy code.

use std::borrow::Cow;

/// Percent-encode a single path segment for outbound URI composition.
///
/// OpenAPI path parameters are single segments: encoded `/` becomes `%2F` and
/// must not introduce an extra delimiter when substituted into a template.
#[must_use]
pub fn encode_path_segment(value: &str) -> Cow<'_, str> {
    urlencoding::encode(value)
}

/// Percent-encode a query key or value for outbound URI composition.
///
/// Spaces become `%20` (never `+`). Reserved delimiters in values are encoded
/// so they cannot split or truncate the query string.
#[must_use]
pub fn encode_query_component(value: &str) -> Cow<'_, str> {
    urlencoding::encode(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::resolve_path_template;
    use crate::router::ParamVec;
    use http_legacy::Uri;
    use std::sync::Arc;

    fn params(pairs: &[(&str, &str)]) -> ParamVec {
        let mut p = ParamVec::new();
        for (k, v) in pairs {
            p.push((Arc::from(*k), (*v).to_string()));
        }
        p
    }

    fn assert_uri_ok(path: &str) {
        path.parse::<Uri>()
            .unwrap_or_else(|e| panic!("URI rejected {path:?}: {e}"));
    }

    fn legacy_unencoded_query(template: &str, query: &[(&str, &str)]) -> String {
        let mut path = template.to_string();
        if !query.is_empty() {
            path.push('?');
            for (i, (k, v)) in query.iter().enumerate() {
                if i > 0 {
                    path.push('&');
                }
                path.push_str(k);
                path.push('=');
                path.push_str(v);
            }
        }
        path
    }

    // --- Positive ---

    #[test]
    fn encode_query_positive_p1_space_percent20() {
        assert_eq!(encode_query_component("South Africa"), "South%20Africa");
        let path = resolve_path_template(
            "/api/v1/locations/provinces",
            &ParamVec::new(),
            &params(&[("country", "South Africa")]),
        );
        assert_eq!(path, "/api/v1/locations/provinces?country=South%20Africa");
        assert_uri_ok(&path);
    }

    #[test]
    fn encode_query_positive_p2_accented() {
        let enc = encode_query_component("Côte");
        assert!(enc.contains('%'), "expected pct-encoding, got {enc}");
        assert!(!enc.contains('ô'));
        assert_uri_ok(&format!("/p?q={enc}"));
    }

    #[test]
    fn encode_path_positive_p3_space() {
        assert_eq!(encode_path_segment("Western Cape"), "Western%20Cape");
        let path = resolve_path_template(
            "/regions/{region}",
            &params(&[("region", "Western Cape")]),
            &ParamVec::new(),
        );
        assert_eq!(path, "/regions/Western%20Cape");
        assert_uri_ok(&path);
    }

    #[test]
    fn encode_query_positive_p4_plus_is_percent2b() {
        assert_eq!(encode_query_component("+"), "%2B");
        assert_eq!(encode_query_component("a+b"), "a%2Bb");
        let path = resolve_path_template("/p", &ParamVec::new(), &params(&[("q", "a+b")]));
        assert_eq!(path, "/p?q=a%2Bb");
        assert!(!path.contains("q=a+b"));
        assert_uri_ok(&path);
    }

    #[test]
    fn encode_query_positive_p5_unreserved() {
        assert_eq!(encode_query_component("-._~"), "-._~");
        assert_eq!(encode_path_segment("abc-._~9"), "abc-._~9");
    }

    #[test]
    fn encode_query_positive_p6_empty_value() {
        let path = resolve_path_template("/p", &ParamVec::new(), &params(&[("k", "")]));
        assert_eq!(path, "/p?k=");
        assert_uri_ok(&path);
    }

    #[test]
    fn encode_query_positive_p7_multi_param_order() {
        let path = resolve_path_template(
            "/p",
            &ParamVec::new(),
            &params(&[("a", "1"), ("b", "2"), ("c", "3")]),
        );
        assert_eq!(path, "/p?a=1&b=2&c=3");
        assert_uri_ok(&path);
    }

    #[test]
    fn encode_path_positive_p8_path_only() {
        let path =
            resolve_path_template("/items/{id}", &params(&[("id", "abc")]), &ParamVec::new());
        assert_eq!(path, "/items/abc");
        assert_uri_ok(&path);
    }

    // --- Negative ---

    #[test]
    fn encode_negative_n1_raw_space_fails_uri() {
        let legacy = legacy_unencoded_query("/p", &[("k", "South Africa")]);
        assert!(legacy.parse::<Uri>().is_err());
        let fixed =
            resolve_path_template("/p", &ParamVec::new(), &params(&[("k", "South Africa")]));
        assert_uri_ok(&fixed);
    }

    #[test]
    fn encode_negative_n2_raw_ampersand() {
        assert_eq!(encode_query_component("a&b=c"), "a%26b%3Dc");
        let legacy = legacy_unencoded_query("/p", &[("k", "a&b=c")]);
        // Unencoded `&` invents an extra pair when re-parsed (`k=a` and `b=c`).
        assert_eq!(crate::server::request::parse_query_params(&legacy).len(), 2);
        let fixed = resolve_path_template("/p", &ParamVec::new(), &params(&[("k", "a&b=c")]));
        assert_eq!(fixed, "/p?k=a%26b%3Dc");
        assert_uri_ok(&fixed);
    }

    #[test]
    fn encode_negative_n3_raw_equals() {
        assert!(encode_query_component("x=y").contains("%3D"));
        let fixed = resolve_path_template("/p", &ParamVec::new(), &params(&[("k", "x=y")]));
        assert_eq!(fixed, "/p?k=x%3Dy");
        assert_uri_ok(&fixed);
    }

    #[test]
    fn encode_negative_n4_raw_hash() {
        assert_eq!(encode_query_component("x#frag"), "x%23frag");
        let legacy = legacy_unencoded_query("/p", &[("k", "x#frag")]);
        assert!(legacy.contains('#'));
        let fixed = resolve_path_template("/p", &ParamVec::new(), &params(&[("k", "x#frag")]));
        assert!(!fixed.contains('#'));
        assert_uri_ok(&fixed);
    }

    #[test]
    fn encode_negative_n5_raw_question_in_path() {
        assert_eq!(encode_path_segment("a?b"), "a%3Fb");
        let path =
            resolve_path_template("/items/{id}", &params(&[("id", "a?b")]), &ParamVec::new());
        assert_eq!(path, "/items/a%3Fb");
        assert_uri_ok(&path);
    }

    #[test]
    fn encode_negative_n6_controls() {
        let legacy = legacy_unencoded_query("/p", &[("k", "a\tb\nc")]);
        assert!(legacy.parse::<Uri>().is_err());
        let fixed = resolve_path_template("/p", &ParamVec::new(), &params(&[("k", "a\tb\nc")]));
        assert_uri_ok(&fixed);
    }

    #[test]
    fn encode_negative_n7_missing_path_param_no_panic() {
        // Placeholder left unsubstituted — composition does not panic.
        let path = resolve_path_template("/items/{id}", &ParamVec::new(), &ParamVec::new());
        assert_eq!(path, "/items/{id}");
    }

    #[test]
    fn encode_negative_n8_proxy_has_no_raw_urlencoding_encode() {
        let src = include_str!("proxy.rs");
        assert!(
            !src.contains("urlencoding::encode"),
            "proxy.rs must use encode_path_segment / encode_query_component only"
        );
    }
}
