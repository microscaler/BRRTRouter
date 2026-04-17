//! Centralized sensitive data sanitizer for log output.
//!
//! Provides field-name-based masking for credentials, PII, and other sensitive values
//! in structured log events. This module is the canonical place for redaction logic
//! used by request parsing, service logging, and dispatcher modules.
//!
//! # Architecture
//!
//! The `tracing` crate's `Layer::on_event` cannot mutate field values in-flight, so
//! the correct Rust/tracing idiom is **call-site sanitization**: sanitize data *before*
//! passing it to `tracing::debug!` / `tracing::info!` macros. This module provides
//! the utilities for that pattern.
//!
//! The [`Sanitizer`] struct holds a [`RedactionLevel`](crate::otel::RedactionLevel) and
//! exposes methods to check, mask, and deep-traverse JSON values.
//!
//! # Examples
//!
//! ```
//! use brrtrouter::sanitize::Sanitizer;
//! use brrtrouter::otel::RedactionLevel;
//! use serde_json::json;
//!
//! let s = Sanitizer::new(RedactionLevel::Credentials);
//!
//! // Field-name check
//! assert!(s.should_redact("password"));
//! assert!(!s.should_redact("username"));
//!
//! // Value masking
//! assert_eq!(s.redact_value("password", "hunter2"), "<REDACTED>");
//! assert_eq!(s.redact_value("api_key", "sk_live_abc123"), "sk_l***");
//!
//! // Deep JSON traversal
//! let input = json!({"user": "alice", "password": "secret"});
//! let safe = s.sanitize_json(&input);
//! assert_eq!(safe["user"], "alice");
//! assert_eq!(safe["password"], "<REDACTED>");
//! ```

use crate::dispatcher::HeaderVec;
use crate::otel::RedactionLevel;
use crate::router::ParamVec;
use serde_json::Value;
use std::sync::Arc;

/// Credential field-name patterns (case-insensitive substring match).
const CREDENTIAL_PATTERNS: &[&str] = &[
    "password",
    "passwd",
    "pwd",
    "secret",
    "api_key",
    "apikey",
    "token",
    "access_token",
    "refresh_token",
    "authorization",
    "credentials",
    "ssn",
    "social_security_number",
    "credit_card",
    "creditcard",
    "ccnumber",
];

/// PII field-name patterns (case-insensitive substring match).
/// Only applied at [`RedactionLevel::Full`].
const PII_PATTERNS: &[&str] = &["email", "ip", "ip_address", "user_id", "phone", "name"];

/// Centralized sensitive-data sanitizer.
///
/// Thread-safe and cheap to clone (contains only a `Copy` enum).
/// Construct once per service init and share via reference or `Arc`.
#[derive(Debug, Clone, Copy)]
pub struct Sanitizer {
    level: RedactionLevel,
}

impl Sanitizer {
    /// Create a new sanitizer with the given redaction level.
    pub fn new(level: RedactionLevel) -> Self {
        Self { level }
    }

    /// Create a sanitizer from the current environment configuration.
    ///
    /// Reads `BRRTR_LOG_REDACT_LEVEL` (default: `"credentials"`).
    pub fn from_env() -> Self {
        let level = RedactionLevel::parse(
            &std::env::var("BRRTR_LOG_REDACT_LEVEL").unwrap_or_else(|_| "credentials".to_string()),
        );
        Self { level }
    }

    /// Returns the redaction level this sanitizer is configured with.
    pub fn level(&self) -> RedactionLevel {
        self.level
    }

    /// Check whether a field name matches the sensitive-field policy.
    ///
    /// Matching is case-insensitive and uses substring containment (e.g.
    /// `"x_api_key"` matches the `"api_key"` pattern).
    pub fn should_redact(&self, field_name: &str) -> bool {
        if self.level == RedactionLevel::None {
            return false;
        }

        let lower = field_name.to_ascii_lowercase();

        for pattern in CREDENTIAL_PATTERNS {
            if lower.contains(pattern) {
                return true;
            }
        }

        if self.level == RedactionLevel::Full {
            for pattern in PII_PATTERNS {
                if lower.contains(pattern) {
                    return true;
                }
            }
        }

        false
    }

    /// Mask a sensitive value.
    ///
    /// For key/token fields whose value is longer than 4 characters, returns the
    /// first 4 characters followed by `***` (partial fuzzing for debugging). All
    /// other sensitive fields return `"<REDACTED>"`.
    ///
    /// When [`RedactionLevel::None`], returns the original value unchanged.
    pub fn redact_value(&self, field_name: &str, value: &str) -> String {
        if self.level == RedactionLevel::None {
            return value.to_string();
        }

        let lower = field_name.to_ascii_lowercase();
        if value.len() > 4 && (lower.contains("key") || lower.contains("token")) {
            // Use char_indices to find a UTF-8-safe prefix boundary (up to 4 chars).
            // Byte-slicing (`&value[..4]`) would panic on multi-byte characters.
            let prefix_end = value
                .char_indices()
                .nth(4)
                .map_or(value.len(), |(idx, _)| idx);
            format!("{}***", &value[..prefix_end])
        } else {
            "<REDACTED>".to_string()
        }
    }

    /// Deep-traverse a JSON value, masking values whose keys match the
    /// sensitive-field policy.
    ///
    /// - Objects: each key is checked; matching values are replaced with
    ///   `"<REDACTED>"` (or partial fuzz for key/token fields). Non-matching
    ///   values are recursively sanitized.
    /// - Arrays: each element is recursively sanitized.
    /// - Scalars: returned unchanged.
    ///
    /// When [`RedactionLevel::None`], returns a clone of the input.
    pub fn sanitize_json(&self, value: &Value) -> Value {
        if self.level == RedactionLevel::None {
            return value.clone();
        }

        match value {
            Value::Object(map) => {
                let mut out = serde_json::Map::with_capacity(map.len());
                for (key, val) in map {
                    if self.should_redact(key) {
                        let masked = match val {
                            Value::String(s) => Value::String(self.redact_value(key, s)),
                            _ => Value::String("<REDACTED>".to_string()),
                        };
                        out.insert(key.clone(), masked);
                    } else {
                        out.insert(key.clone(), self.sanitize_json(val));
                    }
                }
                Value::Object(out)
            }
            Value::Array(arr) => Value::Array(arr.iter().map(|v| self.sanitize_json(v)).collect()),
            other => other.clone(),
        }
    }

    /// Return a copy of the header vec with sensitive header values masked.
    ///
    /// Uses the same field-name policy as [`should_redact`](Self::should_redact).
    pub fn sanitize_headers(&self, headers: &HeaderVec) -> HeaderVec {
        if self.level == RedactionLevel::None {
            return headers.clone();
        }

        headers
            .iter()
            .map(|(name, value)| {
                if self.should_redact(name) {
                    (Arc::clone(name), self.redact_value(name, value))
                } else {
                    (Arc::clone(name), value.clone())
                }
            })
            .collect()
    }

    /// Return a copy of the param vec with sensitive parameter values masked.
    ///
    /// Uses the same field-name policy as [`should_redact`](Self::should_redact).
    pub fn sanitize_params(&self, params: &ParamVec) -> ParamVec {
        if self.level == RedactionLevel::None {
            return params.clone();
        }

        params
            .iter()
            .map(|(name, value)| {
                if self.should_redact(name) {
                    (Arc::clone(name), self.redact_value(name, value))
                } else {
                    (Arc::clone(name), value.clone())
                }
            })
            .collect()
    }
}

/// Global default sanitizer, initialised lazily from environment.
///
/// Used by logging call-sites that don't have access to the `LogConfig`.
static DEFAULT_SANITIZER: std::sync::OnceLock<Sanitizer> = std::sync::OnceLock::new();

/// Get or initialize the global default [`Sanitizer`].
///
/// The first call reads `BRRTR_LOG_REDACT_LEVEL` from the environment;
/// subsequent calls return the cached instance.
pub fn default_sanitizer() -> &'static Sanitizer {
    DEFAULT_SANITIZER.get_or_init(Sanitizer::from_env)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ========================================================================
    // should_redact tests
    // ========================================================================

    #[test]
    fn test_credentials_level_redacts_credential_fields() {
        let s = Sanitizer::new(RedactionLevel::Credentials);
        assert!(s.should_redact("password"));
        assert!(s.should_redact("PASSWORD"));
        assert!(s.should_redact("Password"));
        assert!(s.should_redact("user_password"));
        assert!(s.should_redact("passwd"));
        assert!(s.should_redact("pwd"));
        assert!(s.should_redact("secret"));
        assert!(s.should_redact("api_key"));
        assert!(s.should_redact("apikey"));
        assert!(s.should_redact("ApiKey"));
        assert!(s.should_redact("token"));
        assert!(s.should_redact("accessToken"));
        assert!(s.should_redact("access_token"));
        assert!(s.should_redact("refresh_token"));
        assert!(s.should_redact("authorization"));
        assert!(s.should_redact("Authorization"));
        assert!(s.should_redact("credentials"));
        assert!(s.should_redact("ssn"));
        assert!(s.should_redact("credit_card"));
        assert!(s.should_redact("creditCard"));
        assert!(s.should_redact("ccNumber"));
    }

    #[test]
    fn test_credentials_level_does_not_redact_pii() {
        let s = Sanitizer::new(RedactionLevel::Credentials);
        assert!(!s.should_redact("email"));
        assert!(!s.should_redact("ip_address"));
        assert!(!s.should_redact("user_id"));
        assert!(!s.should_redact("phone"));
        assert!(!s.should_redact("name"));
    }

    #[test]
    fn test_full_level_redacts_credentials_and_pii() {
        let s = Sanitizer::new(RedactionLevel::Full);
        // Credentials
        assert!(s.should_redact("password"));
        assert!(s.should_redact("api_key"));
        assert!(s.should_redact("token"));
        // PII
        assert!(s.should_redact("email"));
        assert!(s.should_redact("ip_address"));
        assert!(s.should_redact("user_id"));
        assert!(s.should_redact("phone"));
        assert!(s.should_redact("name"));
    }

    #[test]
    fn test_none_level_redacts_nothing() {
        let s = Sanitizer::new(RedactionLevel::None);
        assert!(!s.should_redact("password"));
        assert!(!s.should_redact("api_key"));
        assert!(!s.should_redact("email"));
        assert!(!s.should_redact("user_id"));
    }

    #[test]
    fn test_non_sensitive_fields_pass_through() {
        let s = Sanitizer::new(RedactionLevel::Full);
        // "username" contains "name" which is a PII pattern at Full level
        assert!(s.should_redact("username"));
        // These should not be redacted at any level
        assert!(!s.should_redact("path"));
        assert!(!s.should_redact("method"));
        assert!(!s.should_redact("status"));
        assert!(!s.should_redact("content_type"));
        assert!(!s.should_redact("host"));

        // At Credentials level, "username" is safe
        let s2 = Sanitizer::new(RedactionLevel::Credentials);
        assert!(!s2.should_redact("username"));
        assert!(!s2.should_redact("path"));
    }

    // ========================================================================
    // redact_value tests
    // ========================================================================

    #[test]
    fn test_redact_value_key_token_partial_fuzz() {
        let s = Sanitizer::new(RedactionLevel::Credentials);
        assert_eq!(s.redact_value("api_key", "sk_live_abc123"), "sk_l***");
        assert_eq!(s.redact_value("token", "abcdefghij"), "abcd***");
        assert_eq!(s.redact_value("access_token", "12345678"), "1234***");
    }

    #[test]
    fn test_redact_value_multibyte_utf8_no_panic() {
        let s = Sanitizer::new(RedactionLevel::Credentials);
        // 3-byte chars: each '€' is 3 bytes, so 5 chars = 15 bytes (> 4).
        // The old byte-slice approach would panic; char-boundary extraction must work.
        let euro_token = "€€€€€rest";
        let result = s.redact_value("token", euro_token);
        assert_eq!(result, "€€€€***");

        // Mixed ASCII + multi-byte
        let mixed = "ab🔑🔑rest";
        let result2 = s.redact_value("api_key", mixed);
        assert_eq!(result2, "ab🔑🔑***");
    }

    #[test]
    fn test_redact_value_short_values_fully_redacted() {
        let s = Sanitizer::new(RedactionLevel::Credentials);
        assert_eq!(s.redact_value("api_key", "abc"), "<REDACTED>");
        assert_eq!(s.redact_value("token", "ab"), "<REDACTED>");
        assert_eq!(s.redact_value("api_key", "abcd"), "<REDACTED>");
    }

    #[test]
    fn test_redact_value_non_key_fields_fully_redacted() {
        let s = Sanitizer::new(RedactionLevel::Credentials);
        assert_eq!(s.redact_value("password", "super_secret_123"), "<REDACTED>");
        assert_eq!(s.redact_value("secret", "my_secret_value"), "<REDACTED>");
        assert_eq!(s.redact_value("authorization", "Bearer xyz"), "<REDACTED>");
    }

    #[test]
    fn test_redact_value_none_level_passthrough() {
        let s = Sanitizer::new(RedactionLevel::None);
        assert_eq!(s.redact_value("password", "hunter2"), "hunter2");
        assert_eq!(s.redact_value("api_key", "sk_live_abc"), "sk_live_abc");
    }

    // ========================================================================
    // sanitize_json tests
    // ========================================================================

    #[test]
    fn test_sanitize_json_flat_object() {
        let s = Sanitizer::new(RedactionLevel::Credentials);
        let input = json!({"user": "alice", "password": "secret", "age": 30});
        let safe = s.sanitize_json(&input);
        assert_eq!(safe["user"], "alice");
        assert_eq!(safe["password"], "<REDACTED>");
        assert_eq!(safe["age"], 30);
    }

    #[test]
    fn test_sanitize_json_nested_object() {
        let s = Sanitizer::new(RedactionLevel::Credentials);
        let input = json!({
            "user": {
                "display_name": "alice",
                "auth": {
                    "password": "secret",
                    "api_key": "sk_live_abcdef"
                }
            }
        });
        let safe = s.sanitize_json(&input);
        assert_eq!(safe["user"]["display_name"], "alice");
        assert_eq!(safe["user"]["auth"]["password"], "<REDACTED>");
        assert_eq!(safe["user"]["auth"]["api_key"], "sk_l***");
    }

    #[test]
    fn test_sanitize_json_credentials_key_redacts_entire_value() {
        let s = Sanitizer::new(RedactionLevel::Credentials);
        // "credentials" is a sensitive key, so the entire nested object is redacted
        let input = json!({
            "credentials": {"password": "secret", "api_key": "sk_live_abcdef"}
        });
        let safe = s.sanitize_json(&input);
        assert_eq!(safe["credentials"], "<REDACTED>");
    }

    #[test]
    fn test_sanitize_json_array_with_objects() {
        let s = Sanitizer::new(RedactionLevel::Credentials);
        let input = json!([
            {"user": "a", "token": "xxxxx"},
            {"user": "b", "token": "yyyyy"}
        ]);
        let safe = s.sanitize_json(&input);
        assert_eq!(safe[0]["user"], "a");
        assert_eq!(safe[0]["token"], "xxxx***");
        assert_eq!(safe[1]["user"], "b");
        assert_eq!(safe[1]["token"], "yyyy***");
    }

    #[test]
    fn test_sanitize_json_none_level_returns_clone() {
        let s = Sanitizer::new(RedactionLevel::None);
        let input = json!({"password": "secret", "token": "abcde"});
        let safe = s.sanitize_json(&input);
        assert_eq!(safe, input);
    }

    #[test]
    fn test_sanitize_json_non_string_sensitive_value() {
        let s = Sanitizer::new(RedactionLevel::Credentials);
        let input = json!({"password": 12345, "token": true});
        let safe = s.sanitize_json(&input);
        assert_eq!(safe["password"], "<REDACTED>");
        assert_eq!(safe["token"], "<REDACTED>");
    }

    #[test]
    fn test_sanitize_json_scalars_unchanged() {
        let s = Sanitizer::new(RedactionLevel::Full);
        assert_eq!(s.sanitize_json(&json!(42)), json!(42));
        assert_eq!(s.sanitize_json(&json!("hello")), json!("hello"));
        assert_eq!(s.sanitize_json(&json!(null)), json!(null));
        assert_eq!(s.sanitize_json(&json!(true)), json!(true));
    }

    // ========================================================================
    // sanitize_headers tests
    // ========================================================================

    #[test]
    fn test_sanitize_headers_masks_authorization() {
        let s = Sanitizer::new(RedactionLevel::Credentials);
        let headers: HeaderVec = smallvec::smallvec![
            (Arc::from("content-type"), "application/json".to_string()),
            (
                Arc::from("authorization"),
                "Bearer sk_live_secret123".to_string()
            ),
            (Arc::from("host"), "example.com".to_string()),
        ];
        let safe = s.sanitize_headers(&headers);
        assert_eq!(safe[0].1, "application/json");
        assert_eq!(safe[1].1, "<REDACTED>");
        assert_eq!(safe[2].1, "example.com");
    }

    #[test]
    fn test_sanitize_headers_none_level_passthrough() {
        let s = Sanitizer::new(RedactionLevel::None);
        let headers: HeaderVec =
            smallvec::smallvec![(Arc::from("authorization"), "Bearer secret".to_string()),];
        let safe = s.sanitize_headers(&headers);
        assert_eq!(safe[0].1, "Bearer secret");
    }

    // ========================================================================
    // sanitize_params tests
    // ========================================================================

    #[test]
    fn test_sanitize_params_masks_sensitive() {
        let s = Sanitizer::new(RedactionLevel::Credentials);
        let params: ParamVec = smallvec::smallvec![
            (Arc::from("page"), "1".to_string()),
            (Arc::from("api_key"), "sk_live_xyz123".to_string()),
        ];
        let safe = s.sanitize_params(&params);
        assert_eq!(safe[0].1, "1");
        assert_eq!(safe[1].1, "sk_l***");
    }

    #[test]
    fn test_sanitize_params_none_level_passthrough() {
        let s = Sanitizer::new(RedactionLevel::None);
        let params: ParamVec = smallvec::smallvec![(Arc::from("token"), "abc12345".to_string()),];
        let safe = s.sanitize_params(&params);
        assert_eq!(safe[0].1, "abc12345");
    }

    // ========================================================================
    // default_sanitizer tests
    // ========================================================================

    #[test]
    fn test_default_sanitizer_returns_consistent_instance() {
        let a = default_sanitizer();
        let b = default_sanitizer();
        // Same pointer — OnceLock guarantees single init
        assert!(std::ptr::eq(a, b));
    }
}
