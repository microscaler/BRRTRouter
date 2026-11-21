//! A static API key: one secret, compared in constant time.
//!
//! The provider the generated `main.rs`, [`crate::server::security_setup`] and
//! [`crate::server::AppService::register_default_security_providers_from_env`] register for an
//! OpenAPI `apiKey` scheme with a configured key. There is no default key: a scheme nobody
//! configured a key for is not registered, so requests that need it are refused.

use super::{SecurityProvider, SecurityRequest};
use crate::spec::SecurityScheme;

/// Compare two byte strings in time that depends only on their lengths, not on where they differ,
/// so a caller timing the answers learns nothing about how much of a guess was right. A length
/// difference is folded into the result rather than returned early.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = (a.len() ^ b.len()) as u64;
    let n = a.len().max(b.len());
    for i in 0..n {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= u64::from(x ^ y);
    }
    std::hint::black_box(diff) == 0
}

/// A static API key for header, query or cookie `apiKey` schemes.
pub struct StaticApiKeyProvider {
    key: String,
    header_override: Option<String>,
    accept_bearer: bool,
}

impl StaticApiKeyProvider {
    /// A provider that accepts exactly `key` where the scheme says (header, query or cookie).
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            header_override: None,
            accept_bearer: false,
        }
    }

    /// Read the key from this header instead of the scheme's `name` (header schemes only).
    #[must_use]
    pub fn header_override(mut self, header: Option<String>) -> Self {
        self.header_override = header;
        self
    }

    /// Also accept `Authorization: Bearer <key>` for a header scheme (a migration convenience).
    #[must_use]
    pub fn accept_bearer(mut self, accept: bool) -> Self {
        self.accept_bearer = accept;
        self
    }

    fn matches(&self, presented: Option<&str>) -> bool {
        presented.is_some_and(|v| constant_time_eq(v.as_bytes(), self.key.as_bytes()))
    }
}

impl SecurityProvider for StaticApiKeyProvider {
    fn validate(&self, scheme: &SecurityScheme, _scopes: &[String], req: &SecurityRequest) -> bool {
        // An empty key never authorises anything (an unset variable must not open the door).
        if self.key.is_empty() {
            return false;
        }
        match scheme {
            SecurityScheme::ApiKey { name, location, .. } => match location.as_str() {
                "header" => {
                    let target = self.header_override.as_deref().unwrap_or(name);
                    let header_ok = self.matches(req.get_header(&target.to_ascii_lowercase()));
                    let bearer_ok = self.accept_bearer
                        && self.matches(
                            req.get_header("authorization")
                                .and_then(|h| h.strip_prefix("Bearer ")),
                        );
                    header_ok | bearer_ok
                }
                "query" => self.matches(req.get_query(name)),
                "cookie" => self.matches(req.get_cookie(name)),
                _ => false,
            },
            _ => false,
        }
    }
}

/// The key a scheme falls back to when `config.yaml` names none: `BRRTR_API_KEY__<SCHEME>` (non-empty), then
/// `BRRTR_API_KEY` (non-empty), then the `--test-api-key` flag. `None` when none is set: the scheme stays
/// unregistered and its requests are refused. There is no built-in default.
#[must_use]
pub fn api_key_from_env(scheme_name: &str, test_api_key: Option<&str>) -> Option<String> {
    let per_scheme = format!(
        "BRRTR_API_KEY__{}",
        scheme_name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            })
            .collect::<String>()
    );
    std::env::var(&per_scheme)
        .ok()
        .filter(|k| !k.is_empty())
        .or_else(|| {
            std::env::var("BRRTR_API_KEY")
                .ok()
                .filter(|k| !k.is_empty())
        })
        .or_else(|| test_api_key.filter(|k| !k.is_empty()).map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_only_when_every_byte_and_the_length_match() {
        assert!(constant_time_eq(b"k-123", b"k-123"));
        assert!(!constant_time_eq(b"k-123", b"k-124"));
        assert!(!constant_time_eq(b"k-123", b"k-1234"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn no_key_anywhere_is_none_never_a_default() {
        if std::env::var("BRRTR_API_KEY").is_ok() {
            return; // the environment chose a key; nothing to show here
        }
        assert_eq!(api_key_from_env("NoSuchScheme_q9", None), None);
        assert_eq!(api_key_from_env("NoSuchScheme_q9", Some("")), None);
        assert_eq!(
            api_key_from_env("NoSuchScheme_q9", Some("t")),
            Some("t".to_string())
        );
    }
}
