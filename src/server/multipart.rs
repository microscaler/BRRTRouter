//! Multipart/form-data parsing for inbound bodies (Story 12.6).
//!
//! **Policy (MVP A):**
//! - Text parts → JSON object fields (string / loose scalars).
//! - File parts (`filename=` present) → JSON object
//!   `{ "filename", "content_type", "size", "encoding": "omit"|"utf8" }`
//!   with optional `"content"` when UTF-8 text and under
//!   [`DEFAULT_MAX_FILE_PART_BYTES`]. Binary / oversized file bytes are omitted
//!   (size still recorded) — not a silent empty `{}` for the whole body.
//! - Missing boundary or structural parse failure → hard error (400).
//! - Never fabricate an empty object for a non-empty multipart body.

use serde_json::{json, Map, Value};

/// Default max octets kept for a single file part's in-memory content.
pub const DEFAULT_MAX_FILE_PART_BYTES: usize = 1024 * 1024;

/// Marker returned from [`crate::server::parse_request`] when multipart is malformed.
pub const MULTIPART_MALFORMED: &str = "multipart_malformed";

/// Marker when `Content-Type` lacks a `boundary=` parameter.
pub const MULTIPART_MISSING_BOUNDARY: &str = "multipart_missing_boundary";

/// Marker when a file part exceeds the per-part content budget (before omission).
pub const MULTIPART_FILE_TOO_LARGE: &str = "multipart_file_too_large";

/// Stable JSON `reason` for 400 multipart errors.
pub const REASON_MULTIPART_MALFORMED: &str = "multipart_malformed";
pub const REASON_MULTIPART_MISSING_BOUNDARY: &str = "multipart_missing_boundary";
pub const REASON_MULTIPART_FILE_TOO_LARGE: &str = "multipart_file_too_large";

/// Extract `boundary` from a Content-Type header value.
#[must_use]
pub fn extract_boundary(content_type: &str) -> Option<String> {
    for part in content_type.split(';').skip(1) {
        let part = part.trim();
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("boundary") {
            let raw = value.trim().trim_matches('"');
            if !raw.is_empty() {
                return Some(raw.to_string());
            }
        }
    }
    None
}

/// HTTP status for multipart parse markers (`None` if not a multipart error).
#[must_use]
pub fn multipart_error_status(err: &str) -> Option<u16> {
    match err {
        MULTIPART_MALFORMED | MULTIPART_MISSING_BOUNDARY => Some(400),
        MULTIPART_FILE_TOO_LARGE => Some(413),
        _ => None,
    }
}

/// JSON error body for multipart failures.
#[must_use]
pub fn multipart_error_json(err: &str) -> Value {
    let (title, reason) = match err {
        MULTIPART_MISSING_BOUNDARY => (
            "Missing multipart boundary",
            REASON_MULTIPART_MISSING_BOUNDARY,
        ),
        MULTIPART_FILE_TOO_LARGE => (
            "Multipart file part exceeds size limit",
            REASON_MULTIPART_FILE_TOO_LARGE,
        ),
        _ => ("Malformed multipart body", REASON_MULTIPART_MALFORMED),
    };
    json!({
        "error": title,
        "reason": reason,
    })
}

/// Parse `multipart/form-data` into a JSON object.
///
/// # Errors
///
/// Returns [`MULTIPART_MISSING_BOUNDARY`], [`MULTIPART_MALFORMED`], or
/// [`MULTIPART_FILE_TOO_LARGE`].
pub fn parse_multipart_form_data(
    raw: &[u8],
    content_type: &str,
    max_file_part_bytes: usize,
) -> Result<Value, String> {
    let Some(boundary) = extract_boundary(content_type) else {
        return Err(MULTIPART_MISSING_BOUNDARY.to_string());
    };
    parse_multipart_with_boundary(raw, &boundary, max_file_part_bytes)
}

fn parse_multipart_with_boundary(
    raw: &[u8],
    boundary: &str,
    max_file_part_bytes: usize,
) -> Result<Value, String> {
    let delim = format!("--{boundary}");
    let delim_bytes = delim.as_bytes();
    let close = format!("--{boundary}--");
    let close_bytes = close.as_bytes();

    // Split on boundary lines. Tolerate optional leading preamble.
    let mut parts: Vec<&[u8]> = Vec::new();
    let mut rest = raw;
    loop {
        if let Some(idx) = find_subslice(rest, delim_bytes) {
            let after = &rest[idx + delim_bytes.len()..];
            // After boundary: optional `--` (closing) or CRLF
            if after.starts_with(b"--") {
                break;
            }
            let after = strip_leading_crlf(after);
            if let Some(next) = find_subslice(after, delim_bytes) {
                let mut chunk = &after[..next];
                chunk = strip_trailing_crlf(chunk);
                if !chunk.is_empty() {
                    parts.push(chunk);
                }
                rest = &after[next..];
            } else if let Some(next) = find_subslice(after, close_bytes) {
                let mut chunk = &after[..next];
                chunk = strip_trailing_crlf(chunk);
                if !chunk.is_empty() {
                    parts.push(chunk);
                }
                break;
            } else {
                // Final part without explicit close — take remainder.
                let chunk = strip_trailing_crlf(after);
                if !chunk.is_empty() {
                    parts.push(chunk);
                }
                break;
            }
        } else if rest.is_empty() && parts.is_empty() {
            return Err(MULTIPART_MALFORMED.to_string());
        } else {
            break;
        }
    }

    if parts.is_empty() && !raw.is_empty() {
        // Non-empty body but no parsable parts.
        return Err(MULTIPART_MALFORMED.to_string());
    }

    let mut map = Map::new();
    for part in parts {
        let (headers, body) = split_part_headers_body(part)?;
        let (name, filename, part_ct) = parse_part_disposition(&headers)?;
        if name.is_empty() {
            return Err(MULTIPART_MALFORMED.to_string());
        }
        if let Some(fname) = filename {
            if body.len() > max_file_part_bytes {
                return Err(MULTIPART_FILE_TOO_LARGE.to_string());
            }
            let mut file_obj = Map::new();
            file_obj.insert("filename".into(), Value::String(fname));
            let ct = part_ct.unwrap_or_else(|| "application/octet-stream".into());
            let treat_as_text = ct.starts_with("text/")
                || ct.contains("json")
                || ct == "application/xml"
                || body.iter().all(|b| b.is_ascii());
            file_obj.insert("content_type".into(), Value::String(ct));
            file_obj.insert("size".into(), json!(body.len()));
            match std::str::from_utf8(body) {
                Ok(text) if treat_as_text => {
                    file_obj.insert("encoding".into(), Value::String("utf8".into()));
                    file_obj.insert("content".into(), Value::String(text.to_string()));
                }
                _ => {
                    file_obj.insert("encoding".into(), Value::String("omit".into()));
                }
            }
            map.insert(name, Value::Object(file_obj));
        } else {
            let text = std::str::from_utf8(body)
                .map_err(|_| MULTIPART_MALFORMED.to_string())?
                .to_string();
            map.insert(name, loose_scalar(&text));
        }
    }

    Ok(Value::Object(map))
}

fn loose_scalar(s: &str) -> Value {
    if s == "true" {
        return Value::Bool(true);
    }
    if s == "false" {
        return Value::Bool(false);
    }
    if let Ok(n) = s.parse::<i64>() {
        return json!(n);
    }
    if let Ok(n) = s.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return Value::Number(num);
        }
    }
    Value::String(s.to_string())
}

fn split_part_headers_body(part: &[u8]) -> Result<(String, &[u8]), String> {
    let sep = find_subslice(part, b"\r\n\r\n")
        .or_else(|| find_subslice(part, b"\n\n"))
        .ok_or_else(|| MULTIPART_MALFORMED.to_string())?;
    let (hdr, rest) = if part[sep..].starts_with(b"\r\n\r\n") {
        (&part[..sep], &part[sep + 4..])
    } else {
        (&part[..sep], &part[sep + 2..])
    };
    let headers = std::str::from_utf8(hdr).map_err(|_| MULTIPART_MALFORMED.to_string())?;
    Ok((headers.to_string(), rest))
}

fn parse_part_disposition(
    headers: &str,
) -> Result<(String, Option<String>, Option<String>), String> {
    let mut name = None;
    let mut filename = None;
    let mut content_type = None;
    for line in headers.lines() {
        let line = line.trim();
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("content-disposition:") {
            for token in line.split(';').skip(1) {
                let token = token.trim();
                let Some((k, v)) = token.split_once('=') else {
                    continue;
                };
                if k.trim().eq_ignore_ascii_case("name") {
                    name = Some(unquote(v));
                } else if k.trim().eq_ignore_ascii_case("filename") {
                    filename = Some(unquote(v));
                }
            }
        } else if lower.starts_with("content-type:") {
            content_type = Some(
                line.split_once(':')
                    .map(|(_, v)| v.trim().to_string())
                    .unwrap_or_default(),
            );
        }
    }
    let name = name.ok_or_else(|| MULTIPART_MALFORMED.to_string())?;
    Ok((name, filename, content_type))
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').to_string()
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn strip_leading_crlf(b: &[u8]) -> &[u8] {
    if b.starts_with(b"\r\n") {
        &b[2..]
    } else if b.starts_with(b"\n") {
        &b[1..]
    } else {
        b
    }
}

fn strip_trailing_crlf(b: &[u8]) -> &[u8] {
    if b.ends_with(b"\r\n") {
        &b[..b.len() - 2]
    } else if b.ends_with(b"\n") {
        &b[..b.len() - 1]
    } else {
        b
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn sample_multipart(boundary: &str, body: &str) -> Vec<u8> {
        // body is inner parts without outer wrapper
        format!("--{boundary}\r\n{body}--{boundary}--\r\n").into_bytes()
    }

    #[test]
    fn multipart_p1_text_fields() {
        let boundary = "AaB03x";
        let inner = concat!(
            "Content-Disposition: form-data; name=\"name\"\r\n\r\n",
            "Ada\r\n",
            "--AaB03x\r\n",
            "Content-Disposition: form-data; name=\"age\"\r\n\r\n",
            "36\r\n",
        );
        let raw = sample_multipart(boundary, inner);
        let ct = format!("multipart/form-data; boundary={boundary}");
        let v = parse_multipart_form_data(&raw, &ct, DEFAULT_MAX_FILE_PART_BYTES).unwrap();
        assert_eq!(v["name"], "Ada");
        assert_eq!(v["age"], 36);
    }

    #[test]
    fn multipart_p4_boundary_quoted() {
        let boundary = "----WebKit";
        let inner = "Content-Disposition: form-data; name=\"a\"\r\n\r\nx\r\n";
        let raw = sample_multipart(boundary, inner);
        let ct = format!("multipart/form-data; boundary=\"{boundary}\"");
        let v = parse_multipart_form_data(&raw, &ct, DEFAULT_MAX_FILE_PART_BYTES).unwrap();
        assert_eq!(v["a"], "x");
    }

    #[test]
    fn multipart_p6_file_part_policy() {
        let boundary = "b";
        let inner = concat!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"note.txt\"\r\n",
            "Content-Type: text/plain\r\n\r\n",
            "hello\r\n",
        );
        let raw = sample_multipart(boundary, inner);
        let ct = "multipart/form-data; boundary=b";
        let v = parse_multipart_form_data(raw.as_slice(), ct, DEFAULT_MAX_FILE_PART_BYTES).unwrap();
        assert_eq!(v["file"]["filename"], "note.txt");
        assert_eq!(v["file"]["encoding"], "utf8");
        assert_eq!(v["file"]["content"], "hello");
        assert_eq!(v["file"]["size"], 5);
    }

    #[test]
    fn multipart_n4_no_empty_object_for_nonempty() {
        let boundary = "z";
        let inner = "Content-Disposition: form-data; name=\"k\"\r\n\r\nv\r\n";
        let raw = sample_multipart(boundary, inner);
        let v = parse_multipart_form_data(
            &raw,
            "multipart/form-data; boundary=z",
            DEFAULT_MAX_FILE_PART_BYTES,
        )
        .unwrap();
        assert!(v.as_object().unwrap().len() >= 1);
        assert_ne!(v, json!({}));
    }

    #[test]
    fn multipart_n6_missing_boundary() {
        let err = parse_multipart_form_data(b"x", "multipart/form-data", 100).unwrap_err();
        assert_eq!(err, MULTIPART_MISSING_BOUNDARY);
    }

    #[test]
    fn multipart_n3_malformed() {
        let err =
            parse_multipart_form_data(b"not-multipart", "multipart/form-data; boundary=abc", 100)
                .unwrap_err();
        assert_eq!(err, MULTIPART_MALFORMED);
    }

    #[test]
    fn multipart_n5_file_too_large() {
        let boundary = "b";
        let big = "x".repeat(50);
        let inner = format!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"big.bin\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n\
             {big}\r\n"
        );
        let raw = sample_multipart(boundary, &inner);
        let err =
            parse_multipart_form_data(&raw, "multipart/form-data; boundary=b", 10).unwrap_err();
        assert_eq!(err, MULTIPART_FILE_TOO_LARGE);
    }
}
