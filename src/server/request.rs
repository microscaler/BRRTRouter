use crate::spec::ParameterStyle;
use may_minihttp::Request;
use std::collections::HashMap;
use std::io::Read;

/// Parsed HTTP request data used by `AppService`.
#[derive(Debug, PartialEq)]
pub struct ParsedRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub cookies: HashMap<String, String>,
    pub query_params: HashMap<String, String>,
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

pub fn decode_param_value(
    value: &str,
    schema: Option<&serde_json::Value>,
    style: Option<ParameterStyle>,
    explode: Option<bool>,
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

pub fn parse_request(req: Request) -> ParsedRequest {
    let method = req.method().to_string();
    let raw_path = req.path().to_string();
    let path = raw_path.split('?').next().unwrap_or("/").to_string();

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

    let cookies = parse_cookies(&headers);
    let query_params = parse_query_params(&raw_path);

    let body = {
        let mut body_str = String::new();
        if let Ok(size) = req.body().read_to_string(&mut body_str) {
            if size > 0 {
                serde_json::from_str(&body_str).ok()
            } else {
                None
            }
        } else {
            None
        }
    };

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
