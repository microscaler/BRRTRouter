//! OpenAPI `style` / `explode` query rebuild (Story 10.9).
//!
//! See [`docs/EPICS/URI_REQUEST_TARGET/openapi-style-explode-matrix.md`].

use crate::http::uri_encode::encode_query_component;
use crate::router::ParamVec;

/// Supported outbound query serialization modes for proxy rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryRebuildStyle {
    /// `form` + `explode=true` (OpenAPI query default): `id=1&id=2`.
    FormExplode,
    /// `form` + `explode=false`: `id=1,2`.
    FormNoExplode,
}

/// Fail-closed error when a style/explode combo is not supported for rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryStyleError {
    /// Named style is not implemented for proxy query rebuild.
    Unsupported { style: String },
}

impl std::fmt::Display for QueryStyleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported { style } => {
                write!(
                    f,
                    "unsupported OpenAPI query style for proxy rebuild: {style}"
                )
            }
        }
    }
}

impl std::error::Error for QueryStyleError {}

/// Resolve OpenAPI query `style` / `explode` to a supported rebuild mode.
///
/// - `style` omitted or `"form"` → form
/// - `explode` omitted → `true` (OpenAPI default for query `form`)
/// - anything else → [`QueryStyleError::Unsupported`]
pub fn query_rebuild_style(
    style: Option<&str>,
    explode: Option<bool>,
) -> Result<QueryRebuildStyle, QueryStyleError> {
    let style_norm = style.map(|s| s.trim().to_ascii_lowercase());
    match style_norm.as_deref() {
        None | Some("") | Some("form") => {
            if explode.unwrap_or(true) {
                Ok(QueryRebuildStyle::FormExplode)
            } else {
                Ok(QueryRebuildStyle::FormNoExplode)
            }
        }
        Some(other) => Err(QueryStyleError::Unsupported {
            style: other.to_string(),
        }),
    }
}

/// Encode query params for the given rebuild style (`?…` or empty).
#[must_use]
pub fn encode_query_styled(query_params: &ParamVec, style: QueryRebuildStyle) -> String {
    match style {
        QueryRebuildStyle::FormExplode => encode_query_form_explode(query_params),
        QueryRebuildStyle::FormNoExplode => encode_query_form_no_explode(query_params),
    }
}

/// `form` + explode: one `k=v` per ParamVec entry (preserves duplicates).
#[must_use]
pub fn encode_query_form_explode(query_params: &ParamVec) -> String {
    if query_params.is_empty() {
        return String::new();
    }
    let mut qs = String::from("?");
    for (i, (k, v)) in query_params.iter().enumerate() {
        if i > 0 {
            qs.push('&');
        }
        qs.push_str(encode_query_component(k.as_ref()).as_ref());
        qs.push('=');
        qs.push_str(encode_query_component(v.as_ref()).as_ref());
    }
    qs
}

/// `form` + non-explode: group by key (first-seen order), comma-join encoded values.
#[must_use]
pub fn encode_query_form_no_explode(query_params: &ParamVec) -> String {
    if query_params.is_empty() {
        return String::new();
    }
    let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
    for (k, v) in query_params {
        let key = k.as_ref();
        if let Some((_, vals)) = groups.iter_mut().find(|(gk, _)| *gk == key) {
            vals.push(v.as_str());
        } else {
            groups.push((key, vec![v.as_str()]));
        }
    }
    let mut qs = String::from("?");
    for (i, (k, vals)) in groups.iter().enumerate() {
        if i > 0 {
            qs.push('&');
        }
        qs.push_str(encode_query_component(k).as_ref());
        qs.push('=');
        for (j, v) in vals.iter().enumerate() {
            if j > 0 {
                qs.push(',');
            }
            qs.push_str(encode_query_component(v).as_ref());
        }
    }
    qs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{resolve_downstream_target, resolve_path_template};
    use crate::server::request::parse_query_params;
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
            .unwrap_or_else(|e| panic!("legacy Uri rejected {path:?}: {e}"));
        path.parse::<http::Uri>()
            .unwrap_or_else(|e| panic!("http 1.0 Uri rejected {path:?}: {e}"));
    }

    // --- Positive ---

    #[test]
    fn openapi_query_positive_p1_form_explode_array() {
        let q = params(&[("id", "1"), ("id", "2")]);
        let style = query_rebuild_style(Some("form"), Some(true)).unwrap();
        assert_eq!(style, QueryRebuildStyle::FormExplode);
        let qs = encode_query_styled(&q, style);
        assert_eq!(qs, "?id=1&id=2");
        assert_uri_ok(&format!("/p{qs}"));
    }

    #[test]
    fn openapi_query_positive_p2_form_no_explode_array() {
        let q = params(&[("id", "1"), ("id", "2")]);
        let style = query_rebuild_style(Some("form"), Some(false)).unwrap();
        assert_eq!(style, QueryRebuildStyle::FormNoExplode);
        let qs = encode_query_styled(&q, style);
        assert_eq!(qs, "?id=1,2");
        assert_uri_ok(&format!("/p{qs}"));
    }

    #[test]
    fn openapi_query_positive_p3_duplicate_keys_roundtrip() {
        let inbound = "/p?a=1&a=2";
        let parsed = parse_query_params(inbound);
        let rebuilt = resolve_path_template("/p", &ParamVec::new(), &parsed);
        assert_eq!(rebuilt, "/p?a=1&a=2");
        let again = parse_query_params(&rebuilt);
        let pairs: Vec<_> = again
            .iter()
            .map(|(k, v)| (k.as_ref().to_string(), v.clone()))
            .collect();
        assert_eq!(
            pairs,
            vec![("a".into(), "1".into()), ("a".into(), "2".into())]
        );
    }

    #[test]
    fn openapi_query_positive_p4_path_simple_scalar() {
        let path = params(&[("id", "abc")]);
        let t = resolve_path_template("/items/{id}", &path, &ParamVec::new());
        assert_eq!(t, "/items/abc");
        assert_uri_ok(&t);
    }

    #[test]
    fn openapi_query_positive_p5_scalar_query_form() {
        let q = params(&[("k", "v")]);
        let qs = encode_query_styled(&q, QueryRebuildStyle::FormExplode);
        assert_eq!(qs, "?k=v");
        assert_uri_ok(&format!("/p{qs}"));
    }

    #[test]
    fn openapi_query_positive_p6_object_form_explode_unsupported_documented() {
        // Object form explode is unsupported for proxy rebuild (matrix).
        let err = query_rebuild_style(Some("deepObject"), Some(true)).unwrap_err();
        assert!(matches!(err, QueryStyleError::Unsupported { .. }));
    }

    #[test]
    fn openapi_query_positive_p7_default_style_when_omitted() {
        let style = query_rebuild_style(None, None).unwrap();
        assert_eq!(style, QueryRebuildStyle::FormExplode);
    }

    #[test]
    fn openapi_query_positive_p8_spaces_under_form_encoded() {
        let q = params(&[("country", "South Africa")]);
        let qs = encode_query_styled(&q, QueryRebuildStyle::FormExplode);
        assert_eq!(qs, "?country=South%20Africa");
        assert!(!qs.contains(' '));
        assert_uri_ok(&format!("/p{qs}"));
    }

    // --- Negative ---

    #[test]
    fn openapi_query_negative_n1_unsupported_style() {
        for style in [
            "matrix",
            "label",
            "pipeDelimited",
            "spaceDelimited",
            "deepObject",
        ] {
            let err = query_rebuild_style(Some(style), Some(true)).unwrap_err();
            assert!(
                matches!(err, QueryStyleError::Unsupported { .. }),
                "expected unsupported for {style}"
            );
        }
    }

    #[test]
    fn openapi_query_negative_n2_deep_object_fail_closed() {
        let err = query_rebuild_style(Some("deepObject"), Some(true)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "unsupported OpenAPI query style for proxy rebuild: deepobject"
        );
    }

    #[test]
    fn openapi_query_negative_n3_explode_false_only_for_form() {
        // explode=false with unsupported style still fails closed (not silent form).
        let err = query_rebuild_style(Some("deepObject"), Some(false)).unwrap_err();
        assert!(matches!(err, QueryStyleError::Unsupported { .. }));
    }

    #[test]
    fn openapi_query_negative_n4_losing_duplicates_forbidden_on_explode() {
        let q = params(&[("a", "1"), ("a", "2")]);
        let qs = encode_query_form_explode(&q);
        assert_eq!(qs.matches("a=").count(), 2, "must not flatten duplicates");
        assert!(qs.contains("a=1") && qs.contains("a=2"));
    }

    #[test]
    fn openapi_query_negative_n5_empty_array_omits_query() {
        let qs = encode_query_styled(&ParamVec::new(), QueryRebuildStyle::FormExplode);
        assert_eq!(qs, "");
        let qs2 = encode_query_styled(&ParamVec::new(), QueryRebuildStyle::FormNoExplode);
        assert_eq!(qs2, "");
    }

    #[test]
    fn openapi_query_negative_n6_null_optional_omit_policy() {
        // Null/optional → absent from ParamVec → omit (no invented empty key).
        let q = params(&[("keep", "1")]);
        let qs = encode_query_form_explode(&q);
        assert_eq!(qs, "?keep=1");
        assert!(!qs.contains("gone"));
    }

    #[test]
    fn openapi_query_negative_n7_path_array_style_not_in_query_api() {
        // Path array styles are out of query_rebuild_style; simple is path-only.
        let err = query_rebuild_style(Some("simple"), None).unwrap_err();
        assert!(matches!(err, QueryStyleError::Unsupported { .. }));
    }

    #[test]
    fn openapi_query_negative_n8_reserved_chars_encoded_in_join() {
        let q = params(&[("id", "a&b"), ("id", "c=d")]);
        let qs = encode_query_form_no_explode(&q);
        assert_eq!(qs, "?id=a%26b,c%3Dd");
        assert_uri_ok(&format!("/p{qs}"));
        // Explode form also encodes.
        let qs2 = encode_query_form_explode(&q);
        assert!(qs2.contains("a%26b") && qs2.contains("c%3Dd"));
        assert_uri_ok(&format!("/p{qs2}"));
    }

    #[test]
    fn openapi_query_passthrough_still_preserves_explode_octets() {
        let q = params(&[("id", "1"), ("id", "2")]);
        let raw = "id=1&id=2";
        let t = resolve_downstream_target("/p", &ParamVec::new(), &q, Some(raw)).unwrap();
        assert_eq!(t, "/p?id=1&id=2");
    }
}
