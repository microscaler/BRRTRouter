use crate::security::{SecurityProvider, SecurityRequest};
use crate::spec::SecurityScheme;
use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use tracing::{debug, warn};

/// Simple Bearer/JWT provider that validates tokens embedded in the
/// `Authorization` header or a cookie.
///
/// Tokens are expected to have the form `header.payload.signature` where the
/// signature part must match the configured `signature` string. Only the
/// payload section is inspected for a whitespace separated `scope` field.
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
        let payload_bytes = match general_purpose::STANDARD
            .decode(payload.expect("payload already validated as Some"))
        {
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
