use crate::security::{SecurityProvider, SecurityRequest};
use crate::spec::SecurityScheme;
use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use tracing::{debug, warn};

/// Simple Bearer/JWT provider that validates tokens embedded in the
/// `Authorization` header or a cookie.
///
/// Tokens are expected to have the form `header.payload.signature` where the
/// signature part must match the configured `signature` string. The payload is
/// decoded as JWT base64url (RFC 7515), with a fallback for padded standard base64.
/// Only the payload JSON is inspected for a whitespace separated `scope` field.
pub struct BearerJwtProvider {
    pub(crate) signature: String,
    pub(crate) cookie_name: Option<String>,
}

impl BearerJwtProvider {
    /// Create a new Bearer JWT provider with the given signature
    ///
    /// The signature is used to validate JWT tokens (checked against the 3rd part of the JWT).
    /// This is a simplified implementation for testing - production should use proper JWT libraries.
    ///
    /// # Arguments
    ///
    /// * `signature` - Expected JWT signature value
    pub fn new(signature: impl Into<String>) -> Self {
        Self {
            signature: signature.into(),
            cookie_name: None,
        }
    }

    /// Configure the cookie name used to read the token.
    pub fn cookie_name(mut self, name: impl Into<String>) -> Self {
        self.cookie_name = Some(name.into());
        self
    }

    fn extract_token<'a>(&self, req: &'a SecurityRequest) -> Option<&'a str> {
        if let Some(name) = &self.cookie_name {
            if let Some(t) = req.get_cookie(name) {
                return Some(t);
            }
        }
        req.get_header("authorization")
            .and_then(|h| h.strip_prefix("Bearer "))
    }

    pub(crate) fn validate_token(&self, token: &str, scopes: &[String]) -> bool {
        let mut parts = token.split('.');
        let header = parts.next();
        let payload = parts.next();
        let sig = parts.next();
        if header.is_none() || payload.is_none() || sig != Some(self.signature.as_str()) {
            debug!("BearerJWT token validation failed: malformed token or invalid signature");
            return false;
        }
        // Safe to unwrap here because we checked is_none() above
        let payload_str = payload.expect("payload already validated as Some");
        let payload_bytes = match decode_jwt_segment(payload_str) {
            Ok(b) => b,
            Err(e) => {
                debug!(
                    "BearerJWT token validation failed: invalid base64 payload - {:?}",
                    e
                );
                return false;
            }
        };
        let json: Value = match serde_json::from_slice(&payload_bytes) {
            Ok(v) => v,
            Err(e) => {
                debug!(
                    "BearerJWT token validation failed: invalid JSON payload - {:?}",
                    e
                );
                return false;
            }
        };
        let token_scopes = json.get("scope").and_then(|v| v.as_str()).unwrap_or("");
        let has_all_scopes = scopes
            .iter()
            .all(|s| token_scopes.split_whitespace().any(|ts| ts == s));

        if !has_all_scopes {
            warn!(
                "BearerJWT validation failed: missing required scopes (token: {:?}, required: {:?})",
                token_scopes,
                scopes
            );
        }

        has_all_scopes
    }
}

/// Decode a JWT segment: real JWTs use base64url without padding (RFC 7515); tests may use
/// standard base64 with padding.
fn decode_jwt_segment(segment: &str) -> Result<Vec<u8>, base64::DecodeError> {
    general_purpose::URL_SAFE_NO_PAD
        .decode(segment)
        .or_else(|_| {
            let mut s = segment.to_string();
            let rem = s.len() % 4;
            if rem != 0 {
                s.push_str(&"=".repeat(4 - rem));
            }
            general_purpose::STANDARD.decode(s)
        })
}

/// Bearer JWT authentication provider implementation
///
/// Validates JWT tokens passed via the `Authorization: Bearer {token}` header.
/// Checks token signature and required scopes.
///
/// # Validation Flow
///
/// 1. Verify security scheme is HTTP Bearer
/// 2. Extract token from Authorization header (or cookie if configured)
/// 3. Parse JWT and validate signature
/// 4. Check if token contains all required scopes
///
/// # JWT Format
///
/// Expects JWT format: `header.payload.signature`
/// - Signature (3rd part) must match configured signature value
/// - Payload must be valid JSON with optional `scope` field
/// - Scopes can be space-separated string: `"read:users write:users"`
///
/// # Security
///
/// This is a simplified implementation suitable for:
/// - ✅ Testing and development
/// - ✅ Internal microservices with pre-shared secrets
/// - ❌ NOT for production with external clients (use `JwksBearerProvider`)
///
/// For production JWT validation with proper key rotation,
/// use `JwksBearerProvider` with JWKS endpoint.
impl SecurityProvider for BearerJwtProvider {
    /// Validate a Bearer JWT token against the security scheme
    ///
    /// # Arguments
    ///
    /// * `scheme` - Security scheme from OpenAPI spec
    /// * `scopes` - Required OAuth2 scopes from operation
    /// * `req` - The security request containing headers/cookies
    ///
    /// # Returns
    ///
    /// - `true` - Token is valid and contains required scopes
    /// - `false` - Token missing, invalid signature, or missing scopes
    fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::Http { scheme, .. } if scheme.eq_ignore_ascii_case("bearer") => {}
            _ => {
                debug!("BearerJWT validation failed: unsupported security scheme");
                return false;
            }
        }
        let token = match self.extract_token(req) {
            Some(t) => t,
            None => {
                debug!("BearerJWT validation failed: missing token (no Authorization header or cookie)");
                return false;
            }
        };
        let result = self.validate_token(token, scopes);
        if result {
            debug!("BearerJWT validation succeeded: token valid");
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_jwt_segment, BearerJwtProvider};

    #[test]
    fn jwt_io_example_payload_decodes() {
        let payload = "eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ";
        let _ = decode_jwt_segment(payload).expect("jwt.io-style base64url payload should decode");
    }

    #[test]
    fn pet_store_e2e_style_token_validates_with_default_sig() {
        let p = BearerJwtProvider::new("sig");
        let tok = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.sig";
        assert!(p.validate_token(tok, &[]));
    }
}
