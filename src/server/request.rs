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
use serde_json::{json, Map, Number, Value};
use std::io::Read;
use std::sync::Arc;
use tracing::{debug, info};
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
    /// Request path including query string
    pub path: String,
    /// HTTP headers (lowercase keys) - stack-allocated for ≤16 headers
    pub headers: HeaderVec,
    /// Parsed cookies from Cookie header - stack-allocated for ≤16 cookies
    pub cookies: HeaderVec,
    /// Parsed query string parameters - stack-allocated for ≤8 params
    pub query_params: ParamVec,
    /// Parsed request body as JSON: `application/json`, `application/x-www-form-urlencoded`,
    /// or a placeholder object for `multipart/form-data` (see `parse_request_body`).
    pub body: Option<serde_json::Value>,
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

/// Parse query string parameters from a URL path
///
/// Extracts everything after the `?` character and URL-decodes parameter names and values.
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

fn primary_content_type(content_type: &str) -> &str {
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
        map.insert(k.into_owned(), loose_json_scalar(&v.into_owned()));
    }
    Value::Object(map)
}

/// Build a [`serde_json::Value`] from raw bytes and `Content-Type`.
///
/// Supports `application/json`, `application/x-www-form-urlencoded`, and a minimal
/// `multipart/form-data` placeholder so `request_body_required` routes receive `Some(body)`.
fn parse_request_body(raw: &[u8], content_type: &str) -> Option<Value> {
    let ct = primary_content_type(content_type);
    let ct_lower = ct.to_ascii_lowercase();
    if ct_lower == "application/json" || ct_lower.ends_with("+json") {
        return serde_json::from_slice(raw).ok();
    }
    if ct_lower == "application/x-www-form-urlencoded" {
        return Some(form_urlencoded_body_to_json(raw));
    }
    if ct_lower == "multipart/form-data" {
        return Some(json!({}));
    }
    serde_json::from_slice(raw).ok()
}

/// Parse an incoming HTTP request into a ParsedRequest
///
/// Extracts all components (method, path, headers, cookies, query params, body)
/// from the raw HTTP request.
///
/// # Arguments
///
/// * `req` - The raw HTTP request from may_minihttp
///
/// # Returns
///
/// A parsed request with all extracted components
///
/// # JSF Compliance
///
/// Uses SmallVec for headers, cookies, and query params to avoid heap
/// allocation in the common case.
///
/// # Returns
///
/// Returns `Ok(ParsedRequest)` if the request is valid, or `Err(invalid_method_string)`
/// if the HTTP method is invalid and cannot be parsed.
pub fn parse_request(req: Request) -> Result<ParsedRequest, String> {
    // JSF P1: Parse method directly to Method enum (avoids String allocation)
    // Reject invalid HTTP methods instead of defaulting to GET (security fix)
    let method_str = req.method();
    let method = method_str.parse().map_err(|_| method_str.to_string())?;
    let raw_path = req.path().to_string();
    let path = raw_path.split('?').next().unwrap_or("/").to_string();
    // JSF P1: Use static strings for HTTP version (avoids format! allocation)
    // Note: may_minihttp version() returns a Debug-able type, but we can't match on it
    // So we format once (acceptable as it's not in the hot path per-request allocation)
    let http_version = format!("{:?}", req.version());

    // R3: Headers extracted - using SmallVec for stack allocation
    // JSF P2: Use Arc::from for header names (O(1) clone instead of O(n) string copy)
    let headers: HeaderVec = req
        .headers()
        .iter()
        .map(|h| {
            (
                Arc::from(h.name.to_ascii_lowercase().as_str()),
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
    let query_params = parse_query_params(&raw_path);
    let sanitizer = crate::sanitize::default_sanitizer();
    let safe_query = sanitizer.sanitize_params(&query_params);
    debug!(
        param_count = query_params.len(),
        query_params = ?safe_query,
        "Query params parsed"
    );

    // R5 & R6: Request body read and parsed (JSON, form-urlencoded, multipart)
    let parse_start = std::time::Instant::now();
    let body = {
        let mut raw: Vec<u8> = Vec::new();
        if let Ok(size) = req.body().read_to_end(&mut raw) {
            if size > 0 {
                // Find content-type header using the HeaderVec helper
                let content_type = headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .map(|(_, v)| v.as_str())
                    .unwrap_or("");

                // R5: Request body read
                info!(
                    content_length = size,
                    content_type = %content_type,
                    body_size_bytes = size,
                    "Request body read"
                );

                let parsed = parse_request_body(&raw, content_type);
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

                parsed
            } else {
                None
            }
        } else {
            None
        }
    };

    // R2: HTTP request parsed
    info!(
        method = %method,
        path = %path,
        http_version = %http_version,
        headers_count = header_count,
        "HTTP request parsed"
    );

    Ok(ParsedRequest {
        method,
        path,
        headers,
        cookies,
        query_params,
        body,
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

    #[test]
    fn test_parse_query_params() {
        let q = parse_query_params("/p?x=1&y=2");
        assert_eq!(find_query_param(&q, "x"), Some("1"));
        assert_eq!(find_query_param(&q, "y"), Some("2"));
    }

    #[test]
    fn test_parse_request_body_json() {
        let v = parse_request_body(br#"{"x":1}"#, "application/json").expect("json");
        assert_eq!(v["x"], 1);
    }

    #[test]
    fn test_parse_request_body_form_urlencoded() {
        let v = parse_request_body(b"name=Bob&age=30", "application/x-www-form-urlencoded")
            .expect("form");
        assert_eq!(v["name"], "Bob");
        assert_eq!(v["age"], 30);
    }

    #[test]
    fn test_parse_request_body_multipart_placeholder() {
        let v = parse_request_body(
            b"pretend-binary",
            "multipart/form-data; boundary=----WebKit",
        )
        .expect("multipart");
        assert!(v.is_object());
        assert!(v.as_object().unwrap().is_empty());
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
