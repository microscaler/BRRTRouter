//! Merge `Vary` response header field-values for application / gateway use.
//!
//! [`CorsMiddleware`](super::CorsMiddleware) sets CORS/PNA-related `Vary` tokens only. When a
//! response also varies on other request headers (e.g. `Accept-Encoding`, `Authorization`), call
//! [`merge_vary_field_value`] after composing the final header list (or in a gateway) so caches
//! see every axis the response depends on.

use std::collections::HashSet;

/// Merge an existing comma-separated `Vary` value with additional header field-name tokens.
///
/// - Tokens are compared **ASCII case-insensitively** for deduplication (HTTP field names).
/// - Order: existing tokens first (left to right), then `additional` tokens not already present.
/// - Whitespace after commas is normalized to `", "` in the output.
/// - If `existing` or `additional` contains `*` (after trim), returns `"*"` ([RFC 7231](https://datatracker.ietf.org/doc/html/rfc7231#section-7.1.4) — `Vary: *` disables caching for that response).
pub fn merge_vary_field_value(existing: Option<&str>, additional: &[&str]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut seen_lower: HashSet<String> = HashSet::new();

    let mut push = |t: &str| -> bool {
        let t = t.trim();
        if t.is_empty() {
            return false;
        }
        if t == "*" {
            return true;
        }
        let lower = t.to_ascii_lowercase();
        if seen_lower.insert(lower) {
            out.push(t.to_string());
        }
        false
    };

    if let Some(e) = existing {
        for part in e.split(',') {
            if push(part) {
                return "*".to_string();
            }
        }
    }

    for t in additional {
        if push(t) {
            return "*".to_string();
        }
    }

    out.join(", ")
}

#[cfg(test)]
mod tests {
    use super::merge_vary_field_value;

    #[test]
    fn merge_empty_additional_preserves_existing() {
        assert_eq!(
            merge_vary_field_value(Some("Origin, Accept-Encoding"), &[] as &[&str]),
            "Origin, Accept-Encoding"
        );
    }

    #[test]
    fn merge_dedupes_case_insensitively() {
        assert_eq!(
            merge_vary_field_value(Some("origin"), &["Origin", "Accept-Encoding"]),
            "origin, Accept-Encoding"
        );
    }

    #[test]
    fn merge_additional_only() {
        assert_eq!(
            merge_vary_field_value(None, &["Authorization"]),
            "Authorization"
        );
    }

    #[test]
    fn vary_star_short_circuits() {
        assert_eq!(merge_vary_field_value(Some("*"), &["Origin"]), "*");
        assert_eq!(merge_vary_field_value(Some("Origin"), &["*"]), "*");
    }
}
