//! RFC 7807 / RFC 9457 Problem Details for framework-generated errors (Epic 13.3).
//!
//! Default `Content-Type` is [`PROBLEM_CONTENT_TYPE`]. Set
//! `BRRTR_LEGACY_ERROR_JSON=1` to emit the pre-13.3 `{error,reason,message}` shape
//! with `application/json` (escape hatch for one migration cycle).
//!
//! Type URI catalog: [`docs/PROBLEM_DETAILS.md`](../../docs/PROBLEM_DETAILS.md).

use std::sync::Arc;

use may_minihttp::Response;
use serde_json::{json, Map, Value};

use crate::dispatcher::{HandlerResponse, HeaderVec};

/// `Content-Type` for Problem Details responses.
pub const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";

/// Env escape hatch: emit legacy JSON error bodies (`application/json`).
pub const LEGACY_ERROR_JSON_ENV: &str = "BRRTR_LEGACY_ERROR_JSON";

/// Stable type URI prefix (catalog authority).
pub const TYPE_BASE: &str = "https://microscaler.dev/problems/";

pub const TYPE_PARAMETER_VALIDATION_FAILED: &str =
    "https://microscaler.dev/problems/parameter-validation-failed";
pub const TYPE_REQUEST_BODY_TOO_LARGE: &str =
    "https://microscaler.dev/problems/request-body-too-large";
pub const TYPE_MULTIPART_MISSING_BOUNDARY: &str =
    "https://microscaler.dev/problems/multipart-missing-boundary";
pub const TYPE_MULTIPART_MALFORMED: &str = "https://microscaler.dev/problems/multipart-malformed";
pub const TYPE_MULTIPART_FILE_TOO_LARGE: &str =
    "https://microscaler.dev/problems/multipart-file-too-large";
pub const TYPE_UNAUTHORIZED: &str = "https://microscaler.dev/problems/unauthorized";
pub const TYPE_FORBIDDEN: &str = "https://microscaler.dev/problems/forbidden";
pub const TYPE_NOT_FOUND: &str = "https://microscaler.dev/problems/not-found";
pub const TYPE_URI_TOO_LONG: &str = "https://microscaler.dev/problems/uri-too-long";
pub const TYPE_BAD_REQUEST: &str = "https://microscaler.dev/problems/bad-request";
pub const TYPE_RATE_LIMIT_EXCEEDED: &str = "https://microscaler.dev/problems/rate-limit-exceeded";
pub const TYPE_GATEWAY_TIMEOUT: &str = "https://microscaler.dev/problems/gateway-timeout";
pub const TYPE_INTERNAL: &str = "https://microscaler.dev/problems/internal-error";

/// RFC 7807 Problem Details builder.
#[derive(Debug, Clone)]
pub struct Problem {
    pub type_uri: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    pub reason: Option<String>,
    pub fields: Option<Value>,
    pub instance: Option<String>,
}

impl Problem {
    /// Build a problem; empty detail/reason never panics (NFR-1).
    #[must_use]
    pub fn new(type_uri: impl Into<String>, title: impl Into<String>, status: u16) -> Self {
        Self {
            type_uri: type_uri.into(),
            title: title.into(),
            status,
            detail: String::new(),
            reason: None,
            fields: None,
            instance: None,
        }
    }

    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    #[must_use]
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        let r = reason.into();
        self.reason = if r.is_empty() { None } else { Some(r) };
        self
    }

    #[must_use]
    pub fn fields(mut self, fields: Value) -> Self {
        self.fields = Some(fields);
        self
    }

    /// Map HTTP status + human detail to a catalog type (auth / generic).
    #[must_use]
    pub fn from_status_detail(status: u16, detail: &str) -> Self {
        let (type_uri, title) = match status {
            401 => (TYPE_UNAUTHORIZED, "Unauthorized"),
            403 => (TYPE_FORBIDDEN, "Forbidden"),
            404 => (TYPE_NOT_FOUND, "Not Found"),
            413 => (TYPE_REQUEST_BODY_TOO_LARGE, "Payload Too Large"),
            414 => (TYPE_URI_TOO_LONG, "URI Too Long"),
            429 => (TYPE_RATE_LIMIT_EXCEEDED, "Too Many Requests"),
            400 => (TYPE_BAD_REQUEST, "Bad Request"),
            504 => (TYPE_GATEWAY_TIMEOUT, "Gateway Timeout"),
            500..=599 => (TYPE_INTERNAL, "Internal Server Error"),
            _ => (TYPE_BAD_REQUEST, "Error"),
        };
        let detail = if detail.is_empty() {
            title.to_string()
        } else {
            detail.to_string()
        };
        Self::new(type_uri, title, status).detail(detail)
    }

    /// Serialize to JSON. Includes RFC members plus legacy `error` / `message`
    /// (and `reason` / `fields` extensions) for one migration cycle (NFR-4).
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("type".into(), json!(self.type_uri));
        map.insert("title".into(), json!(self.title));
        map.insert("status".into(), json!(self.status));
        let detail = if self.detail.is_empty() {
            self.title.as_str()
        } else {
            self.detail.as_str()
        };
        map.insert("detail".into(), json!(detail));
        // Legacy aliases (same major cycle).
        map.insert("error".into(), json!(detail));
        map.insert("message".into(), json!(detail));
        if let Some(r) = &self.reason {
            map.insert("reason".into(), json!(r));
        }
        if let Some(f) = &self.fields {
            map.insert("fields".into(), f.clone());
        }
        if let Some(i) = &self.instance {
            map.insert("instance".into(), json!(i));
        }
        Value::Object(map)
    }

    /// Legacy-only body (escape hatch).
    #[must_use]
    pub fn to_legacy_value(&self) -> Value {
        let detail = if self.detail.is_empty() {
            self.title.as_str()
        } else {
            self.detail.as_str()
        };
        let mut map = Map::new();
        map.insert("error".into(), json!(self.title));
        map.insert("message".into(), json!(detail));
        if let Some(r) = &self.reason {
            map.insert("reason".into(), json!(r));
            // Historical body_too_large used title as error string "Payload Too Large"
            map.insert("error".into(), json!(self.title));
        } else {
            // HandlerResponse::error historically put the message in `error`.
            map.insert("error".into(), json!(detail));
        }
        if let Some(f) = &self.fields {
            map.insert("fields".into(), f.clone());
        }
        Value::Object(map)
    }

    /// Build a [`HandlerResponse`] with the correct Content-Type.
    #[must_use]
    pub fn into_handler_response(self) -> HandlerResponse {
        let (ct, body) = if legacy_error_json_enabled() {
            ("application/json", self.to_legacy_value())
        } else {
            (PROBLEM_CONTENT_TYPE, self.to_value())
        };
        let mut headers = HeaderVec::new();
        headers.push((Arc::from("content-type"), ct.to_string()));
        HandlerResponse::new(self.status, headers, body)
    }
}

/// `true` when `BRRTR_LEGACY_ERROR_JSON` is `1` / `true` / `yes`.
#[must_use]
pub fn legacy_error_json_enabled() -> bool {
    match std::env::var(LEGACY_ERROR_JSON_ENV) {
        Ok(v) => {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        }
        Err(_) => false,
    }
}

/// Write a Problem Details (or legacy) error to the wire.
pub fn write_problem(res: &mut Response, problem: &Problem) {
    let reason_phrase = problem_status_reason(problem.status);
    res.status_code(problem.status as usize, reason_phrase);
    let (ct, body) = if legacy_error_json_enabled() {
        ("application/json", problem.to_legacy_value())
    } else {
        (PROBLEM_CONTENT_TYPE, problem.to_value())
    };
    res.header(format!("Content-Type: {ct}"));
    match serde_json::to_vec(&body) {
        Ok(bytes) => res.body_vec(bytes),
        Err(_) => res.body_vec(br#"{"type":"about:blank","title":"Error","status":500,"detail":"serialization failure"}"#.to_vec()),
    }
}

/// Wrap a pre-built JSON error (with optional `reason` / `fields` / `message`) as a Problem.
#[must_use]
pub fn problem_from_legacy_body(status: u16, body: &Value) -> Problem {
    let reason = body.get("reason").and_then(|v| v.as_str()).unwrap_or("");
    let message = body
        .get("message")
        .and_then(|v| v.as_str())
        .or_else(|| body.get("error").and_then(|v| v.as_str()))
        .unwrap_or("");
    let title = body
        .get("error")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default_title(status));

    let mut p = if !reason.is_empty() {
        problem_for_reason(reason, status, title, message)
    } else {
        Problem::from_status_detail(status, message).detail(if message.is_empty() {
            title.to_string()
        } else {
            message.to_string()
        })
    };

    // Prefer explicit title from legacy `error` when it looks like a title.
    if body.get("error").and_then(|v| v.as_str()).is_some() && !title.is_empty() {
        p.title = title.to_string();
    }
    if let Some(fields) = body.get("fields") {
        p.fields = Some(fields.clone());
    }
    p
}

fn default_title(status: u16) -> &'static str {
    problem_status_reason(status)
}

fn problem_for_reason(reason: &str, status: u16, title: &str, detail: &str) -> Problem {
    let (type_uri, catalog_title) = match reason {
        "parameter_validation_failed" => (TYPE_PARAMETER_VALIDATION_FAILED, "Bad Request"),
        "request_body_too_large" => (TYPE_REQUEST_BODY_TOO_LARGE, "Payload Too Large"),
        "multipart_missing_boundary" => (
            TYPE_MULTIPART_MISSING_BOUNDARY,
            "Missing multipart boundary",
        ),
        "multipart_malformed" => (TYPE_MULTIPART_MALFORMED, "Malformed multipart body"),
        "multipart_file_too_large" => (
            TYPE_MULTIPART_FILE_TOO_LARGE,
            "Multipart file part exceeds size limit",
        ),
        "rate_limit_exceeded" | "rate limit exceeded" => {
            (TYPE_RATE_LIMIT_EXCEEDED, "Too Many Requests")
        }
        "handler_deadline_exceeded" => (TYPE_GATEWAY_TIMEOUT, "Gateway Timeout"),
        _ => (TYPE_BAD_REQUEST, default_title(status)),
    };
    let title = if title.is_empty() {
        catalog_title
    } else {
        title
    };
    let detail = if detail.is_empty() { title } else { detail };
    Problem::new(type_uri, title, status)
        .detail(detail)
        .reason(reason)
}

fn problem_status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        414 => "URI Too Long",
        415 => "Unsupported Media Type",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Error",
    }
}

/// Parameter validation → problem JSON value.
#[must_use]
pub fn parameter_validation_problem(fields: Value, detail: &str) -> Problem {
    Problem::new(TYPE_PARAMETER_VALIDATION_FAILED, "Bad Request", 400)
        .detail(detail)
        .reason("parameter_validation_failed")
        .fields(fields)
}

/// Body too large → problem.
#[must_use]
pub fn body_too_large_problem(detail: &str) -> Problem {
    Problem::new(TYPE_REQUEST_BODY_TOO_LARGE, "Payload Too Large", 413)
        .detail(detail)
        .reason("request_body_too_large")
}

/// Multipart error marker → problem.
#[must_use]
pub fn multipart_problem(err: &str) -> Problem {
    let (type_uri, title, status, reason, detail) = match err {
        "multipart_missing_boundary" => (
            TYPE_MULTIPART_MISSING_BOUNDARY,
            "Missing multipart boundary",
            400,
            "multipart_missing_boundary",
            "Content-Type is multipart but boundary is missing",
        ),
        "multipart_file_too_large" => (
            TYPE_MULTIPART_FILE_TOO_LARGE,
            "Multipart file part exceeds size limit",
            413,
            "multipart_file_too_large",
            "A multipart file part exceeds the configured size limit",
        ),
        _ => (
            TYPE_MULTIPART_MALFORMED,
            "Malformed multipart body",
            400,
            "multipart_malformed",
            "The multipart body could not be parsed",
        ),
    };
    Problem::new(type_uri, title, status)
        .detail(detail)
        .reason(reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn n2_status_member_always_present() {
        let p = Problem::from_status_detail(401, "nope");
        let v = p.to_value();
        assert_eq!(v["status"], 401);
        assert!(v.get("type").is_some());
        assert!(v.get("title").is_some());
        assert!(v.get("detail").is_some());
    }

    #[test]
    fn n4_empty_detail_no_panic() {
        let p = Problem::new(TYPE_BAD_REQUEST, "Bad Request", 400).detail("");
        let v = p.to_value();
        assert_eq!(v["detail"], "Bad Request");
    }

    #[test]
    fn n5_detail_has_no_bearer_token() {
        let p = Problem::from_status_detail(401, "Unauthorized");
        let s = p.to_value().to_string();
        assert!(!s.to_lowercase().contains("bearer ey"));
        assert!(!s.contains("secret="));
    }
}
