//! Path segment percent-decoding (RFC 3986).
//!
//! Story 10.3 — inbound path params are decoded here before handlers see them.
//! This is **not** form-urlencoded: `+` is a literal plus, never a space.

/// Decode a single path segment with RFC 3986 percent-decoding.
///
/// # Policy (aligned with Story 10.2 “no panic”, path-specific for `+`)
///
/// - `%20` → space; UTF-8 multi-byte sequences decode to Unicode
/// - `+` is **not** converted to space (unlike query form-urlencoded)
/// - Truncated / illegal `%` sequences: left as literal text when
///   `urlencoding::decode` succeeds with pass-through; on UTF-8 errors,
///   fall back to lossy decode (`U+FFFD`)
/// - Decode **once** — `%2520` becomes `%20`, not a space
///
/// Never panics.
#[must_use]
pub fn decode_path_segment(segment: &str) -> String {
    match urlencoding::decode(segment) {
        Ok(decoded) => decoded.into_owned(),
        Err(_) => {
            // Invalid UTF-8 after percent-decoding: lossy replacement.
            percent_decode_lossy(segment)
        }
    }
}

fn percent_decode_lossy(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_path_segment_positive_p1_space() {
        assert_eq!(decode_path_segment("Western%20Cape"), "Western Cape");
    }

    #[test]
    fn decode_path_segment_positive_p2_accented() {
        assert_eq!(decode_path_segment("C%C3%B4te"), "Côte");
    }

    #[test]
    fn decode_path_segment_positive_p3_unreserved() {
        assert_eq!(decode_path_segment("simple-id._~"), "simple-id._~");
    }

    #[test]
    fn decode_path_segment_positive_p4_encoded_slash() {
        assert_eq!(decode_path_segment("a%2Fb"), "a/b");
    }

    #[test]
    fn decode_path_segment_positive_p5_cjk() {
        assert_eq!(decode_path_segment("%E6%9D%B1%E4%BA%AC"), "東京");
    }

    #[test]
    fn decode_path_segment_negative_n1_plus_not_space() {
        assert_eq!(decode_path_segment("Western+Cape"), "Western+Cape");
    }

    #[test]
    fn decode_path_segment_negative_n2_truncated_percent() {
        assert_eq!(decode_path_segment("%"), "%");
        assert_eq!(decode_path_segment("%2"), "%2");
    }

    #[test]
    fn decode_path_segment_negative_n3_illegal_hex() {
        assert_eq!(decode_path_segment("%GG"), "%GG");
    }

    #[test]
    fn decode_path_segment_negative_n4_encoded_dotdot() {
        assert_eq!(decode_path_segment("%2E%2E"), "..");
    }

    #[test]
    fn decode_path_segment_negative_n5_decode_once() {
        assert_eq!(decode_path_segment("%2520"), "%20");
    }

    #[test]
    fn decode_path_segment_negative_n6_nul() {
        assert_eq!(decode_path_segment("a%00b"), "a\0b");
    }

    #[test]
    fn decode_path_segment_negative_n7_overlong_or_invalid_utf8() {
        // Lone %FF is not valid UTF-8 → lossy U+FFFD, no panic
        assert_eq!(decode_path_segment("%FF"), "\u{FFFD}");
    }

    #[test]
    fn decode_path_segment_negative_n8_control_tab() {
        assert_eq!(decode_path_segment("a%09b"), "a\tb");
    }
}
