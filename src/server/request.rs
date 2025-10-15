use crate::spec::ParameterStyle;
use may_minihttp::Request;
use std::collections::HashMap;
use std::io::Read;
use tracing::{debug, info};

/// Parsed HTTP request data used by `AppService`.
///
/// Contains all extracted information from the raw HTTP request including
/// headers, cookies, query parameters, and JSON body.
#[derive(Debug, PartialEq)]
pub struct ParsedRequest {
    /// HTTP method (GET, POST, etc.)
    pub method: String,
    /// Request path including query string
    pub path: String,
    /// HTTP headers (lowercase keys)
    pub headers: HashMap<String, String>,
    /// Parsed cookies from Cookie header
    pub cookies: HashMap<String, String>,
    /// Parsed query string parameters
    pub query_params: HashMap<String, String>,
    /// Parsed JSON body (if content-type is application/json)
    pub body: Option<serde_json::Value>,
}

/// Extract useful information from a `may_minihttp::Request`.
pub fn parse_cookies(headers: &HashMap<String, String>) -> HashMap<String, String> {
    headers
        .get("cookie")
        .map(|c| {
            c.split(";")
                .filter_map(|pair| {
                    let mut parts = pair.trim().splitn(2, "=");
                    let name = parts.next()?.trim().to_string();
                    let value = parts.next().unwrap_or("").trim().to_string();
                    Some((name, value))
                })
                .collect()
        })
        .unwrap_or_default()
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
/// A map of query parameter names to values
pub fn parse_query_params(path: &str) -> HashMap<String, String> {
    if let Some(pos) = path.find("?") {
        let query_str = &path[pos + 1..];
        url::form_urlencoded::parse(query_str.as_bytes())
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    } else {
        HashMap::new()
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
                let delim = match style.unwrap_or(ParameterStyle::Form) {
                    ParameterStyle::SpaceDelimited => ' ',
                    ParameterStyle::PipeDelimited => '|',
                    _ => ',',
                };
                let parts = value
                    .split(delim)
                    .filter(|s| !s.is_empty())
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
pub fn parse_request(req: Request) -> ParsedRequest {
    let method = req.method().to_string();
    let raw_path = req.path().to_string();
    let path = raw_path.split('?').next().unwrap_or("/").to_string();
    let http_version = format!("{:?}", req.version());

    // R3: Headers extracted
    let headers: HashMap<String, String> = req
        .headers()
        .iter()
        .map(|h| {
            (
                h.name.to_ascii_lowercase(),
                String::from_utf8_lossy(h.value).to_string(),
            )
        })
        .collect();

    let header_names: Vec<&String> = headers.keys().take(20).collect(); // Limit for log size
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
    debug!(
        cookie_count = cookies.len(),
        cookie_names = ?cookies.keys().collect::<Vec<_>>(),
        "Cookies extracted"
    );

    // R4: Query params parsed
    let query_params = parse_query_params(&raw_path);
    debug!(
        param_count = query_params.len(),
        query_params = ?query_params,
        "Query params parsed"
    );

    // R5 & R6: Request body read and JSON body parsed
    let parse_start = std::time::Instant::now();
    let body = {
        let mut body_str = String::new();
        if let Ok(size) = req.body().read_to_string(&mut body_str) {
            if size > 0 {
                let content_type = headers
                    .get("content-type")
                    .map(|s| s.as_str())
                    .unwrap_or("");

                // R5: Request body read
                info!(
                    content_length = size,
                    content_type = %content_type,
                    body_size_bytes = size,
                    "Request body read"
                );

                // R6: JSON body parsed
                let body_result: Result<serde_json::Value, _> = serde_json::from_str(&body_str);
                let parse_duration_ms = parse_start.elapsed().as_millis() as u64;

                if let Ok(ref json) = body_result {
                    debug!(
                        parse_duration_ms = parse_duration_ms,
                        body_fields = json.as_object().map(|o| o.len()),
                        "JSON body parsed"
                    );
                } else if body_result.is_err() {
                    debug!(
                        parse_duration_ms = parse_duration_ms,
                        error = "JSON parse failed",
                        "JSON body parse attempted"
                    );
                }

                body_result.ok()
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

    ParsedRequest {
        method,
        path,
        headers,
        cookies,
        query_params,
        body,
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cookies() {
        let mut h = std::collections::HashMap::new();
        h.insert("cookie".to_string(), "a=b; c=d".to_string());
        let cookies = parse_cookies(&h);
        assert_eq!(cookies.get("a"), Some(&"b".to_string()));
        assert_eq!(cookies.get("c"), Some(&"d".to_string()));
    }

    #[test]
    fn test_parse_query_params() {
        let q = parse_query_params("/p?x=1&y=2");
        assert_eq!(q.get("x"), Some(&"1".to_string()));
        assert_eq!(q.get("y"), Some(&"2".to_string()));
    }
}
