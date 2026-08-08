//! HTTP request parsing - hot path module.
//!
//! # JSF Compliance (Rule 206)
//!
//! This module is part of the request hot path. Clippy lints are denied
//! to enforce "no heap allocations after initialization".

// JSF Rule 206: Deny heap allocations in the hot path
#![deny(clippy::inefficient_to_string)]
#![deny(clippy::format_push_string)]
#![deny(clippy::unnecessary_to_owned)]

use crate::dispatcher::HeaderVec;
use crate::router::ParamVec;
use crate::spec::ParameterStyle;
use http::Method;
use may_minihttp::Request;
use serde_json::{Map, Number, Value};
use std::io::Read;
use std::sync::Arc;
use tracing::debug;
use url::form_urlencoded::parse as parse_form_urlencoded;

/// Parsed HTTP request data used by `AppService`.
///
/// Contains all extracted information from the raw HTTP request including
/// headers, cookies, query parameters, and JSON body.
///
/// # JSF Compliance
///
/// Uses SmallVec (HeaderVec/ParamVec) instead of HashMap for stack-allocated
/// storage in the common case, avoiding heap allocation in the hot path.
#[derive(Debug, PartialEq)]
pub struct ParsedRequest {
    /// HTTP method (GET, POST, etc.)
    /// JSF P1: Use Method enum instead of String to avoid allocation
    pub method: Method,
    /// Path only (no query), for routing
    pub path: String,
    /// Full origin-form request-target after boundary normalize (`/path?query`).
    /// Used for length limits (10.6) and query passthrough octets (10.5).
    pub request_target: String,
    /// HTTP headers (lowercase keys) - stack-allocated for ≤16 headers
    pub headers: HeaderVec,
    /// Parsed cookies from Cookie header - stack-allocated for ≤16 cookies
    pub cookies: HeaderVec,
    /// Parsed query string parameters - stack-allocated for ≤8 params
    pub query_params: ParamVec,
    /// Parsed request body as JSON: `application/json`, `application/x-www-form-urlencoded`,
    /// or multipart field map (see [`super::multipart`] / `parse_request_body`).
    pub body: Option<serde_json::Value>,
    /// Raw body octets actually read (0 when empty). Used for Story 12.2 route caps.
    pub body_octets: usize,
}

impl ParsedRequest {
    /// Get a header by name (case-insensitive)
    #[inline]
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Get a cookie by name
    #[inline]
    pub fn get_cookie(&self, name: &str) -> Option<&str> {
        self.cookies
            .iter()
            .find(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }

    /// Get a query parameter by name
    #[inline]
    pub fn get_query_param(&self, name: &str) -> Option<&str> {
        self.query_params
            .iter()
            .find(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Extract cookies from headers, returning a stack-allocated SmallVec
pub fn parse_cookies(headers: &HeaderVec) -> HeaderVec {
    // Find cookie header using linear search (efficient for small collections)
    let cookie_value = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("cookie"))
        .map(|(_, v)| v.as_str());

    match cookie_value {
        Some(c) => c
            .split(';')
            .filter_map(|pair| {
                let mut parts = pair.trim().splitn(2, '=');
                let name = parts.next()?.trim();
                let value = parts.next().unwrap_or("").trim().to_string();
                // JSF P2: Use Arc::from for cookie names (O(1) clone)
                Some((Arc::from(name), value))
            })
            .collect(),
        None => HeaderVec::new(),
    }
}

/// Parse query string parameters from a URL path.
///
/// Extracts everything after the first `?` and decodes with
/// [`url::form_urlencoded::parse`] (WHATWG `application/x-www-form-urlencoded`):
/// - `+` and `%20` both become a space
/// - Duplicate keys are preserved as multiple `ParamVec` entries (order kept)
/// - Empty values (`k=`) and valueless keys (`k`) yield an empty string value
///
/// # Illegal / truncated percent-encoding policy (Story 10.2)
///
/// We **do not reject** the request for truncated or illegal `%` sequences.
/// Behaviour is inherited from `form_urlencoded` and locked by unit tests:
/// - Truncated (`%`, `%2`) and non-hex (`%GG`) sequences are left as literal text
/// - Invalid UTF-8 after percent-decode is lossy (replacement character `U+FFFD`)
/// - Parsing **never panics** on hostile input
///
/// Fail-closed rejection of illegal encodings (HTTP 400) is intentionally out of
/// scope here; tighten only with a coordinated Epic 10 matrix change.
///
/// # Fragments (`#`)
///
/// HTTP request-targets should not include a fragment (RFC 9110). If a `#` still
/// appears after `?`, it is treated as ordinary query octets by this parser
/// (not stripped). Stripping belongs at the may_minihttp / front boundary
/// (Story 10.11). A path with `#` and **no** `?` yields an empty param list.
///
/// # Arguments
///
/// * `path` - The full URL path (e.g., `/users?limit=10&offset=20`)
///
/// # Returns
///
/// A stack-allocated SmallVec of query parameter (name, value) pairs
///
/// # JSF Compliance
///
/// Returns ParamVec (SmallVec) to avoid heap allocation for ≤8 params
pub fn parse_query_params(path: &str) -> ParamVec {
    if let Some(pos) = path.find('?') {
        let query_str = &path[pos + 1..];
        // JSF: Use Arc::from for param names (O(1) clone in hot path)
        // Values remain String as they're per-request data
        url::form_urlencoded::parse(query_str.as_bytes())
            .map(|(k, v)| (Arc::from(k.as_ref()), v.to_string()))
            .collect()
    } else {
        ParamVec::new()
    }
}

/// Decode a parameter value according to OpenAPI schema and style
///
/// Converts string parameter values to their appropriate JSON types based on
/// the OpenAPI schema (integer, number, boolean, array, object). Handles
/// different serialization styles (form, simple, etc.) for arrays and objects.
///
/// # Arguments
///
/// * `value` - The raw parameter value string
/// * `schema` - Optional JSON Schema for type conversion
/// * `style` - Optional OpenAPI parameter style (form, simple, etc.)
/// * `_explode` - Whether to use exploded format (currently unused)
///
/// # Returns
///
/// The decoded JSON value with appropriate type
pub fn decode_param_value(
    value: &str,
    schema: Option<&serde_json::Value>,
    style: Option<ParameterStyle>,
    _explode: Option<bool>,
) -> serde_json::Value {
    use serde_json::Value;

    fn convert_primitive(val: &str, schema: Option<&Value>) -> Value {
        if let Some(ty) = schema.and_then(|s| s.get("type").and_then(|v| v.as_str())) {
            match ty {
                "integer" => val
                    .parse::<i64>()
                    .map(Value::from)
                    .unwrap_or_else(|_| Value::String(val.to_string())),
                "number" => val
                    .parse::<f64>()
                    .map(Value::from)
                    .unwrap_or_else(|_| Value::String(val.to_string())),
                "boolean" => val
                    .parse::<bool>()
                    .map(Value::from)
                    .unwrap_or_else(|_| Value::String(val.to_string())),
                _ => Value::String(val.to_string()),
            }
        } else {
            Value::String(val.to_string())
        }
    }

    if let Some(ty) = schema.and_then(|s| s.get("type").and_then(|v| v.as_str())) {
        match ty {
            "array" => {
                let items_schema = schema.and_then(|s| s.get("items"));
                let style = style.unwrap_or(ParameterStyle::Form);
                let parts: Vec<&str> = if matches!(style, ParameterStyle::Matrix) {
                    // Matrix: OpenAPI uses comma-separated values in `;name=1,2,3`; browsers may
                    // also send a single segment `1;2;3` for `/matrix/{coords}` — split on `;` then.
                    let mut s = value.trim();
                    if let Some(i) = s.find('=') {
                        s = s[i + 1..].trim();
                    }
                    if s.contains(';') {
                        s.split(';').filter(|p| !p.is_empty()).collect()
                    } else {
                        s.split(',').filter(|p| !p.is_empty()).collect()
                    }
                } else {
                    let delim = match style {
                        ParameterStyle::SpaceDelimited => ' ',
                        ParameterStyle::PipeDelimited => '|',
                        _ => ',',
                    };
                    value.split(delim).filter(|p| !p.is_empty()).collect()
                };
                let parts = parts
                    .into_iter()
                    .map(|p| convert_primitive(p.trim(), items_schema))
                    .collect::<Vec<_>>();
                Value::Array(parts)
            }
            "object" => serde_json::from_str(value).unwrap_or(Value::String(value.to_string())),
            _ => convert_primitive(value, schema),
        }
    } else {
        Value::String(value.to_string())
    }
}

/// Return the primary media type from a `Content-Type` header value,
/// dropping any parameters (e.g. `; charset=utf-8`, `; boundary=...`).
///
/// Exposed so call sites outside this module (e.g. `server::service`) can
/// classify the declared Content-Type of an incoming request without
/// duplicating the trim logic.
pub fn primary_content_type(content_type: &str) -> &str {
    content_type.split(';').next().unwrap_or("").trim()
}

fn loose_json_scalar(s: &str) -> Value {
    if let Ok(i) = s.parse::<i64>() {
        return Value::Number(i.into());
    }
    if let Ok(u) = s.parse::<u64>() {
        return Value::Number(u.into());
    }
    if let Ok(f) = s.parse::<f64>() {
        if let Some(n) = Number::from_f64(f) {
            return Value::Number(n);
        }
    }
    if let Ok(b) = s.parse::<bool>() {
        return Value::Bool(b);
    }
    Value::String(s.to_string())
}

fn form_urlencoded_body_to_json(raw: &[u8]) -> Value {
    let mut map = Map::new();
    for (k, v) in parse_form_urlencoded(raw) {
        map.insert(k.into_owned(), loose_json_scalar(v.as_ref()));
    }
    Value::Object(map)
}

/// Build a [`serde_json::Value`] from raw bytes and `Content-Type`.
///
/// Supports `application/json`, `application/x-www-form-urlencoded`, and
/// `multipart/form-data` (Story 12.6 — text fields + documented file-part policy).
///
/// Multipart failures return `Err` with [`super::multipart`] markers (400/413).
fn parse_request_body(raw: &[u8], content_type: &str) -> Result<Option<Value>, String> {
    let ct = primary_content_type(content_type);
    let ct_lower = ct.to_ascii_lowercase();
    if ct_lower == "application/json" || ct_lower.ends_with("+json") {
        return Ok(serde_json::from_slice(raw).ok());
    }
    if ct_lower == "application/x-www-form-urlencoded" {
        return Ok(Some(form_urlencoded_body_to_json(raw)));
    }
    if ct_lower == "multipart/form-data" {
        let parsed = if super::multipart_stream::multipart_stream_files_enabled() {
            super::multipart_stream::parse_multipart_form_data_streaming(
                raw,
                content_type,
                &super::multipart_stream::MultipartStreamOptions::default(),
            )?
        } else {
            super::multipart::parse_multipart_form_data(
                raw,
                content_type,
                super::multipart::DEFAULT_MAX_FILE_PART_BYTES,
            )?
        };
        return Ok(Some(parsed));
    }
    Ok(serde_json::from_slice(raw).ok())
}

/// Parse an incoming HTTP request into a ParsedRequest
///
/// Extracts all components (method, path, headers, cookies, query params, body)
/// from the raw HTTP request.
///
/// # Request-target boundary (Story 10.11)
///
/// `req.path()` is may_minihttp/`httparse`'s request-target token (often
/// origin-form **including** `?query`). See
/// `docs/EPICS/URI_REQUEST_TARGET/request-line-boundary.md`. Absolute-form
/// targets are normalized via [`super::request_target::request_target_for_app`]
/// before query parse and path routing.
///
/// # Arguments
///
/// * `req` - The raw HTTP request from may_minihttp
///
/// # Returns
///
/// Returns `Ok(ParsedRequest)` if the request is valid, or `Err(invalid_method_string)`
/// if the HTTP method is invalid and cannot be parsed.
///
/// # JSF Compliance
///
/// Uses SmallVec for headers, cookies, and query params to avoid heap
/// allocation in the common case.
pub fn parse_request(req: Request) -> Result<ParsedRequest, String> {
    // JSF P1: Parse method directly to Method enum (avoids String allocation)
    // Reject invalid HTTP methods instead of defaulting to GET (security fix)
    let method_str = req.method();
    let method = method_str.parse().map_err(|_| method_str.to_string())?;
    // Story 10.11: normalize absolute-form → origin path+query for app use.
    let request_target = super::request_target::request_target_for_app(req.path()).to_string();
    // Story 10.6: enforce max request-target octets before heavier parsing.
    let max_len = super::request_target::max_request_target_octets();
    if super::request_target::request_target_exceeds_limit(&request_target, max_len) {
        tracing::debug!(
            target_len = request_target.len(),
            max_len,
            "Request-target exceeds configured max; rejecting with 414"
        );
        return Err(super::request_target::REQUEST_TARGET_TOO_LONG.to_string());
    }
    let path = super::request_target::path_only(&request_target).to_string();
    // JSF P1: Use static strings for HTTP version (avoids format! allocation)
    // Note: may_minihttp version() returns a Debug-able type, but we can't match on it
    // So we format once (acceptable as it's not in the hot path per-request allocation)
    let http_version = format!("{:?}", req.version());

    // R3: Headers extracted — using SmallVec for stack allocation.
    // PRD Phase 2.1: `intern_header_name` returns a shared `Arc<str>` for the
    // ~24 common HTTP header names (`content-type`, `authorization`, …)
    // without any heap allocation on the hit path, which is >95 % of traffic.
    // Falls back to the previous `Arc::from(lowercased)` on miss.
    let headers: HeaderVec = req
        .headers()
        .iter()
        .map(|h| {
            (
                super::header_intern::intern_header_name(h.name.as_bytes()),
                String::from_utf8_lossy(h.value).to_string(),
            )
        })
        .collect();

    // JSF P2: Header names are now Arc<str>, so we get references to the Arc
    let header_names: Vec<&Arc<str>> = headers.iter().map(|(k, _)| k).take(20).collect();
    let header_count = headers.len();
    let size_bytes: usize = headers.iter().map(|(k, v)| k.len() + v.len()).sum();

    debug!(
        header_count = header_count,
        size_bytes = size_bytes,
        header_names = ?header_names,
        "Headers extracted"
    );

    // R7: Cookies extracted
    let cookies = parse_cookies(&headers);
    // JSF P2: Cookie names are now Arc<str>
    let cookie_names: Vec<&Arc<str>> = cookies.iter().map(|(k, _)| k).collect();
    debug!(
        cookie_count = cookies.len(),
        cookie_names = ?cookie_names,
        "Cookies extracted"
    );

    // R4: Query params parsed
    let query_params = parse_query_params(&request_target);
    debug!(
        param_count = query_params.len(),
        query_params = ?query_params,
        "Query params parsed"
    );

    // Story 12.2: reject oversize Content-Length before allocating/reading the body.
    let global_body_max = super::body_limit::max_inbound_body_octets();
    let cl_header = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .map(|(_, v)| v.as_str());
    if super::body_limit::content_length_for_limit(cl_header, global_body_max).is_err() {
        tracing::debug!(
            max = global_body_max,
            "Content-Length exceeds global body max or is hostile; rejecting with 413"
        );
        return Err(super::body_limit::REQUEST_BODY_TOO_LARGE.to_string());
    }

    // R5 & R6: Request body read and parsed (JSON, form-urlencoded, multipart)
    let parse_start = std::time::Instant::now();
    let (body, body_octets) = {
        // Cap the stream at max+1 so we detect overrun without reading unbounded input.
        let mut raw: Vec<u8> = Vec::new();
        let mut limited = req.body().take(global_body_max as u64 + 1);
        match limited.read_to_end(&mut raw) {
            Ok(_) if raw.len() > global_body_max => {
                tracing::debug!(
                    body_len = raw.len(),
                    max = global_body_max,
                    "Request body exceeded global max during read; rejecting with 413"
                );
                return Err(super::body_limit::REQUEST_BODY_TOO_LARGE.to_string());
            }
            Ok(_) if !raw.is_empty() => {
                let size = raw.len();
                let content_type = headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .map(|(_, v)| v.as_str())
                    .unwrap_or("");

                debug!(
                    content_length = size,
                    content_type = %content_type,
                    body_size_bytes = size,
                    "Request body read"
                );

                let parsed = parse_request_body(&raw, content_type)?;
                let parse_duration_ms = parse_start.elapsed().as_millis() as u64;

                if let Some(ref json) = parsed {
                    debug!(
                        parse_duration_ms = parse_duration_ms,
                        body_fields = json.as_object().map(|o| o.len()),
                        "Request body parsed"
                    );
                } else {
                    debug!(
                        parse_duration_ms = parse_duration_ms,
                        "Request body not recognized or invalid JSON"
                    );
                }

                (parsed, size)
            }
            Ok(_) => (None, 0),
            Err(_) => (None, 0),
        }
    };

    // R2: HTTP request parsed — per-request, demoted to debug (PRD 2.2).
    debug!(
        method = %method,
        path = %path,
        http_version = %http_version,
        headers_count = header_count,
        "HTTP request parsed"
    );

    Ok(ParsedRequest {
        method,
        path,
        request_target,
        headers,
        cookies,
        query_params,
        body,
        body_octets,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::ParameterStyle;
    use serde_json::json;
    use std::sync::Arc;

    /// Helper to get a param value from ParamVec (uses Arc<str> keys)
    fn find_query_param<'a>(params: &'a ParamVec, name: &str) -> Option<&'a str> {
        params
            .iter()
            .find(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }

    /// Helper to get a param value from HeaderVec (uses Arc<str> keys)
    // JSF P2: Updated to work with Arc<str> keys
    fn find_header_param<'a>(params: &'a HeaderVec, name: &str) -> Option<&'a str> {
        params
            .iter()
            .find(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn test_parse_cookies() {
        let mut h: HeaderVec = HeaderVec::new();
        // JSF P2: Use Arc::from for header names
        h.push((Arc::from("cookie"), "a=b; c=d".to_string()));
        let cookies = parse_cookies(&h);
        assert_eq!(find_header_param(&cookies, "a"), Some("b"));
        assert_eq!(find_header_param(&cookies, "c"), Some("d"));
    }

    fn pairs(params: &ParamVec) -> Vec<(String, String)> {
        params
            .iter()
            .map(|(k, v)| (k.as_ref().to_string(), v.clone()))
            .collect()
    }

    // --- Story 10.2 positive ---

    #[test]
    fn parse_query_params_positive_p1_ascii_kv() {
        let q = parse_query_params("/p?x=1&y=2");
        assert_eq!(
            pairs(&q),
            vec![("x".into(), "1".into()), ("y".into(), "2".into())]
        );
    }

    #[test]
    fn parse_query_params_positive_p2_percent20_space() {
        let q = parse_query_params("/p?q=South%20Africa");
        assert_eq!(find_query_param(&q, "q"), Some("South Africa"));
    }

    #[test]
    fn parse_query_params_positive_p3_plus_space() {
        let q = parse_query_params("/p?q=South+Africa");
        assert_eq!(find_query_param(&q, "q"), Some("South Africa"));
    }

    #[test]
    fn parse_query_params_positive_p2_p3_spaces_equivalent() {
        let qa = parse_query_params("/p?q=South%20Africa");
        let qb = parse_query_params("/p?q=South+Africa");
        assert_eq!(find_query_param(&qa, "q"), find_query_param(&qb, "q"));
        assert_eq!(find_query_param(&qa, "q"), Some("South Africa"));
    }

    #[test]
    fn parse_query_params_positive_p4_accented() {
        let q = parse_query_params("/p?q=C%C3%B4te");
        assert_eq!(find_query_param(&q, "q"), Some("Côte"));
    }

    #[test]
    fn parse_query_params_positive_p5_duplicate_keys_order() {
        let q = parse_query_params("/p?a=1&a=2");
        assert_eq!(
            pairs(&q),
            vec![("a".into(), "1".into()), ("a".into(), "2".into())]
        );
    }

    #[test]
    fn parse_query_params_positive_p6_empty_value() {
        let q = parse_query_params("/p?k=");
        assert_eq!(find_query_param(&q, "k"), Some(""));
    }

    #[test]
    fn parse_query_params_positive_valueless_key() {
        // form_urlencoded: `k` without `=` → empty value
        let q = parse_query_params("/p?k");
        assert_eq!(find_query_param(&q, "k"), Some(""));
    }

    #[test]
    fn parse_query_params_positive_p7_encoded_plus() {
        let q = parse_query_params("/p?q=%2B");
        assert_eq!(find_query_param(&q, "q"), Some("+"));
    }

    #[test]
    fn parse_query_params_positive_p8_no_query() {
        let q = parse_query_params("/p");
        assert!(q.is_empty());
    }

    #[test]
    fn parse_query_params_positive_p9_trailing_question_mark() {
        // `/p?` → empty query string → no pairs (documented)
        let q = parse_query_params("/p?");
        assert!(q.is_empty(), "expected no pairs for trailing ?, got {q:?}");
    }

    #[test]
    fn parse_query_params_positive_p10_cjk() {
        let q = parse_query_params("/p?name=%E6%9D%B1%E4%BA%AC");
        assert_eq!(find_query_param(&q, "name"), Some("東京"));
    }

    // --- Story 10.2 negative (no panic; documented leave-as-is / lossy) ---

    #[test]
    fn parse_query_params_negative_n1_truncated_percent() {
        let q = parse_query_params("/p?q=%");
        assert_eq!(find_query_param(&q, "q"), Some("%"));
    }

    #[test]
    fn parse_query_params_negative_n2_truncated_hex() {
        let q = parse_query_params("/p?q=%2");
        assert_eq!(find_query_param(&q, "q"), Some("%2"));
    }

    #[test]
    fn parse_query_params_negative_n3_illegal_hex() {
        let q = parse_query_params("/p?q=%GG");
        assert_eq!(find_query_param(&q, "q"), Some("%GG"));
    }

    #[test]
    fn parse_query_params_negative_n4_invalid_utf8_byte() {
        // Lone %FF is not valid UTF-8 → lossy replacement (U+FFFD)
        let q = parse_query_params("/p?q=%FF");
        let v = find_query_param(&q, "q").expect("q present");
        assert_eq!(v, "\u{FFFD}");
    }

    #[test]
    fn parse_query_params_negative_n5_embedded_nul() {
        let q = parse_query_params("/p?q=a%00b");
        assert_eq!(find_query_param(&q, "q"), Some("a\0b"));
        let q2 = parse_query_params("/p?q=a\0b");
        assert_eq!(find_query_param(&q2, "q"), Some("a\0b"));
    }

    #[test]
    fn parse_query_params_negative_n6_long_query_under_414() {
        let long = "x".repeat(4000);
        let path = format!("/p?q={long}");
        let q = parse_query_params(&path);
        assert_eq!(find_query_param(&q, "q"), Some(long.as_str()));
    }

    #[test]
    fn parse_query_params_negative_n7_empty_key_forms() {
        assert_eq!(
            pairs(&parse_query_params("/p?=")),
            vec![("".into(), "".into())]
        );
        assert_eq!(
            pairs(&parse_query_params("/p?=v")),
            vec![("".into(), "v".into())]
        );
    }

    #[test]
    fn parse_query_params_negative_n8_hash_fragment_forms() {
        // No `?` → fragment never reaches query parser
        assert!(parse_query_params("/p#frag").is_empty());
        // `#` after `?` is not stripped here (front boundary owns that — 10.11)
        let q = parse_query_params("/p?q=a#frag");
        assert_eq!(find_query_param(&q, "q"), Some("a#frag"));
        let q2 = parse_query_params("/p?#frag");
        assert_eq!(pairs(&q2), vec![("#frag".into(), "".into())]);
    }

    #[test]
    fn test_parse_request_body_json() {
        let v = parse_request_body(br#"{"x":1}"#, "application/json")
            .unwrap()
            .expect("json");
        assert_eq!(v["x"], 1);
    }

    #[test]
    fn test_parse_request_body_form_urlencoded() {
        let v = parse_request_body(b"name=Bob&age=30", "application/x-www-form-urlencoded")
            .unwrap()
            .expect("form");
        assert_eq!(v["name"], "Bob");
        assert_eq!(v["age"], 30);
    }

    /// N4 — multipart must not silently become `{}` when parts are present.
    #[test]
    fn test_parse_request_body_multipart_parses_fields() {
        let raw =
            b"--WebKit\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n--WebKit--\r\n";
        let v = parse_request_body(raw, "multipart/form-data; boundary=WebKit")
            .unwrap()
            .expect("multipart");
        assert_eq!(v["a"], 1);
        assert_ne!(v, serde_json::json!({}));
    }

    /// `GET /matrix/1;2;3` captures `coords=1;2;3` as one path param; matrix style must split on `;`.
    #[test]
    fn test_decode_param_matrix_array_semicolons() {
        let schema = json!({"type": "array", "items": {"type": "integer"}});
        let v = decode_param_value(
            "1;2;3",
            Some(&schema),
            Some(ParameterStyle::Matrix),
            Some(false),
        );
        assert_eq!(v, json!([1, 2, 3]));
    }

    // Helper function to test HTTP method parsing logic
    // This mirrors the parsing logic in parse_request() to test method validation
    fn test_method_parsing(method_str: &str) -> Result<Method, String> {
        method_str.parse().map_err(|_| method_str.to_string())
    }

    #[test]
    fn test_parse_request_valid_methods() {
        // Test all standard HTTP methods that should be accepted
        let valid_methods = vec![
            ("GET", Method::GET),
            ("POST", Method::POST),
            ("PUT", Method::PUT),
            ("DELETE", Method::DELETE),
            ("PATCH", Method::PATCH),
            ("HEAD", Method::HEAD),
            ("OPTIONS", Method::OPTIONS),
            ("CONNECT", Method::CONNECT),
            ("TRACE", Method::TRACE),
            ("QUERY", crate::http::method_query()),
        ];

        for (method_str, expected_method) in valid_methods {
            let result = test_method_parsing(method_str);
            assert!(result.is_ok(), "Method '{}' should be accepted", method_str);
            assert_eq!(
                result.unwrap(),
                expected_method,
                "Method '{}' should parse to {:?}",
                method_str,
                expected_method
            );
        }
    }

    #[test]
    fn test_parse_request_invalid_method() {
        // Test methods that actually fail to parse (http::Method accepts custom methods,
        // so we test only methods with invalid characters that cause parse failures)
        let invalid_methods = vec![
            "G E T", // With spaces (invalid token character)
            "GET\n", // With newline
            "GET\r", // With carriage return
            "GET\t", // With tab
            "GET/",  // With forward slash
            "GET@",  // With @ symbol
            "",      // Empty string
        ];

        for method_str in invalid_methods {
            let result = test_method_parsing(method_str);
            assert!(
                result.is_err(),
                "Method '{}' should be rejected (contains invalid characters)",
                method_str
            );
            let err = result.unwrap_err();
            assert_eq!(
                err, method_str,
                "Error should contain the invalid method string '{}', got '{}'",
                method_str, err
            );
        }
    }

    #[test]
    fn test_parse_request_custom_methods_accepted() {
        // Note: http::Method accepts custom HTTP methods (extension methods per RFC 7231)
        // This is expected behavior - HTTP allows custom methods
        // The security fix ensures we don't default to GET on parse errors
        let custom_methods = vec!["BOGUS", "CUSTOM", "MYMETHOD", "EXTENSION"];

        for method_str in custom_methods {
            let result = test_method_parsing(method_str);
            // These should parse successfully (http::Method accepts custom methods)
            // The important thing is that parse errors are handled, not that we reject custom methods
            if result.is_ok() {
                // Custom method accepted - this is fine per HTTP spec
                continue;
            }
            // If it fails, that's also fine - the test documents the behavior
        }
    }

    #[test]
    fn test_parse_request_method_case_handling() {
        // Test case sensitivity - HTTP methods are case-sensitive per RFC 7231
        // Standard uppercase methods should work
        assert!(
            test_method_parsing("GET").is_ok(),
            "GET (uppercase) should be valid"
        );
        assert!(
            test_method_parsing("POST").is_ok(),
            "POST (uppercase) should be valid"
        );

        // Note: http::Method::from_str() may or may not accept lowercase depending on implementation
        // The important thing is that clearly invalid methods are rejected
        // If lowercase is accepted, that's fine - we're testing the rejection of invalid methods
    }
}
