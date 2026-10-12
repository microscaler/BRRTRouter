//! Why a request was refused or failed, recorded on the request span where the decision is made.
//!
//! The access log says *that* a request answered 401 or 500; these attributes say *why*, so a single trace explains a
//! failure without correlating logs:
//!
//! | Attribute | Set when | Example |
//! | --- | --- | --- |
//! | `brrtrouter.error.reason` | any 4xx/5xx with a JSON error body | `invalid_request: Authorization session is invalid` |
//! | `brrtrouter.auth.result` | the route has `security` | `allowed`, `denied`, `insufficient_scope` |
//! | `brrtrouter.auth.schemes` | the route has `security` | `BearerAuth` |
//! | `brrtrouter.auth.reason` | a credential was refused | `token_expired`, `invalid_audience`, `scheme_undefined: BearerAuth` |
//! | `brrtrouter.validation.error` | the request body failed its schema | `"qty" is not of type "integer"` |
//! | `brrtrouter.hook` | an application request hook ran | `continued`, `authorization set`, `answered 302` |
//!
//! 5xx responses also mark the span as an error (`otel.status_description`) and set `error.type` to the status code,
//! following the OpenTelemetry HTTP server conventions. Each distinct (method, route, status, reason) is logged at WARN
//! at most once a minute, so a refused route is visible in logs without flooding them.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

/// Longest reason recorded on a span or log line, in bytes.
pub const MAX_REASON_BYTES: usize = 256;
const LOG_WINDOW: Duration = Duration::from_secs(60);
const LOG_KEYS_MAX: usize = 1024;

/// The reason a JSON error body gives: `error: error_description` (OAuth 2.0), `title: detail` (RFC 9457 problem
/// details) or `code: message`, falling back to the first of those fields present. `None` for a body that names no
/// reason.
#[must_use]
pub fn error_reason(body: &Value) -> Option<String> {
    let obj = body.as_object()?;
    let field = |key: &str| -> Option<String> {
        match obj.get(key)? {
            Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    };
    for (head, tail) in [
        ("error", "error_description"),
        ("title", "detail"),
        ("code", "message"),
    ] {
        if let (Some(head), Some(tail)) = (field(head), field(tail)) {
            return Some(truncate(format!("{head}: {tail}")));
        }
    }
    [
        "error_description",
        "detail",
        "message",
        "error",
        "title",
        "code",
    ]
    .into_iter()
    .find_map(field)
    .map(truncate)
}

/// `s` cut to [`MAX_REASON_BYTES`] on a character boundary, marked with an ellipsis when cut.
#[must_use]
pub fn truncate(mut s: String) -> String {
    if s.len() > MAX_REASON_BYTES {
        let mut end = MAX_REASON_BYTES;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push('…');
    }
    s
}

/// True the first time `key` is seen in a minute: gates failure logs that would otherwise repeat for every request.
#[must_use]
pub fn first_in_window(key: &str) -> bool {
    first_in_window_at(key, Instant::now())
}

fn first_in_window_at(key: &str, now: Instant) -> bool {
    static SEEN: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
    let mut guard = match SEEN.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let seen = guard.get_or_insert_with(HashMap::new);
    if seen.len() >= LOG_KEYS_MAX {
        seen.retain(|_, at| now.saturating_duration_since(*at) < LOG_WINDOW);
        if seen.len() >= LOG_KEYS_MAX {
            seen.clear();
        }
    }
    match seen.get(key) {
        Some(at) if now.saturating_duration_since(*at) < LOG_WINDOW => false,
        _ => {
            seen.insert(key.to_string(), now);
            true
        }
    }
}

/// Security scheme names that routes require but the spec does not define, each with one route that uses it. A route
/// requiring an undefined scheme can never be authorized: every request to it answers 401, whatever it carries.
#[must_use]
pub fn undefined_security_schemes<S>(
    routes: &[crate::spec::RouteMeta],
    schemes: &HashMap<String, S>,
) -> Vec<(String, String)> {
    let mut undefined: Vec<(String, String)> = Vec::new();
    for route in routes {
        for requirement in &route.security {
            for name in requirement.0.keys() {
                if !schemes.contains_key(name.as_str()) && !undefined.iter().any(|(n, _)| n == name)
                {
                    undefined.push((
                        name.clone(),
                        format!("{} {}", route.method, route.path_pattern),
                    ));
                }
            }
        }
    }
    undefined.sort();
    undefined
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn oauth_errors_read_as_error_and_description() {
        let body = json!({"error": "invalid_request", "error_description": "Authorization session is invalid"});
        assert_eq!(
            error_reason(&body).as_deref(),
            Some("invalid_request: Authorization session is invalid")
        );
    }

    #[test]
    fn problem_details_read_as_title_and_detail() {
        let body = json!({"type": "about:blank", "title": "Unauthorized", "status": 401, "detail": "Missing or invalid credentials"});
        assert_eq!(
            error_reason(&body).as_deref(),
            Some("Unauthorized: Missing or invalid credentials")
        );
    }

    #[test]
    fn a_single_field_is_enough() {
        assert_eq!(
            error_reason(&json!({"error": "Spec not found"})).as_deref(),
            Some("Spec not found")
        );
        assert_eq!(
            error_reason(&json!({"message": "upstream timed out"})).as_deref(),
            Some("upstream timed out")
        );
        assert_eq!(error_reason(&json!({"code": 42})).as_deref(), Some("42"));
    }

    #[test]
    fn bodies_without_a_reason_give_none() {
        assert_eq!(error_reason(&json!({"items": []})), None);
        assert_eq!(error_reason(&json!("plain")), None);
        assert_eq!(error_reason(&json!({"error": "  "})), None);
    }

    #[test]
    fn long_reasons_are_cut_on_a_character_boundary() {
        let cut = truncate("é".repeat(400));
        assert!(cut.len() <= MAX_REASON_BYTES + '…'.len_utf8());
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn a_key_logs_once_a_minute() {
        let start = Instant::now();
        let key = "test GET /x 401 token_expired";
        assert!(first_in_window_at(key, start));
        assert!(!first_in_window_at(key, start + Duration::from_secs(30)));
        assert!(first_in_window_at(key, start + Duration::from_secs(61)));
        assert!(first_in_window_at("test another key", start));
    }
}
