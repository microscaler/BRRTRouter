//! RFC 10008 `Accept-Query` helpers (Story 11.4).
//!
//! Servers may advertise media types accepted in QUERY request bodies via the
//! `Accept-Query` response (or negotiation) header. This module formats and
//! parses that header without panicking on malformed input.

/// Canonical header name (HTTP field names are case-insensitive).
pub const ACCEPT_QUERY_HEADER: &str = "Accept-Query";

/// Format an `Accept-Query` header value from media-type tokens.
///
/// Empty input yields an empty string (callers should omit the header).
#[must_use]
pub fn format_accept_query(media_types: &[&str]) -> String {
    media_types
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Parse an `Accept-Query` header value into media-type tokens.
///
/// Malformed tokens (empty after trim, or containing ASCII CTL / space inside
/// the type token before parameters) are skipped — never panic (Story 11.4 N3/N7).
#[must_use]
pub fn parse_accept_query(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in value.split(',') {
        // Reject CTL in the raw segment before trim (trim would hide trailing CR/LF).
        if part.bytes().any(|b| b < 0x20 || b == 0x7f) {
            continue;
        }
        let token = part.trim();
        if token.is_empty() {
            continue;
        }
        if !media_type_token_ok(token) {
            continue;
        }
        out.push(token.to_string());
    }
    out
}

fn media_type_token_ok(token: &str) -> bool {
    // Allow `type/subtype` optionally followed by `;param=value` (parameters may
    // contain spaces around `=`). Reject bare CTL and empty type/subtype.
    if token.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return false;
    }
    let type_part = token.split(';').next().unwrap_or("").trim();
    let mut pieces = type_part.splitn(2, '/');
    match (pieces.next(), pieces.next()) {
        (Some(t), Some(s)) => {
            !t.is_empty() && !s.is_empty() && !t.contains(' ') && !s.contains(' ')
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_query_positive_p2_format() {
        let v = format_accept_query(&["application/json", "application/x-www-form-urlencoded"]);
        assert_eq!(v, "application/json, application/x-www-form-urlencoded");
    }

    #[test]
    fn accept_query_positive_parse_roundtrip() {
        let v = format_accept_query(&["application/json", "text/plain"]);
        let parsed = parse_accept_query(&v);
        assert_eq!(parsed, vec!["application/json", "text/plain"]);
    }

    #[test]
    fn accept_query_negative_n3_malformed_skipped() {
        let parsed = parse_accept_query("application/json, , bad, text/plain; charset=utf-8");
        assert_eq!(
            parsed,
            vec![
                "application/json".to_string(),
                "text/plain; charset=utf-8".to_string()
            ]
        );
        let with_ctl = parse_accept_query("application/json\n, text/plain");
        assert_eq!(with_ctl, vec!["text/plain"]);
    }

    #[test]
    fn accept_query_negative_n7_empty_no_panic() {
        assert!(parse_accept_query("").is_empty());
        assert!(format_accept_query(&[]).is_empty());
        assert!(format_accept_query(&["", "  "]).is_empty());
    }
}
