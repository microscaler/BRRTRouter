use crate::security::{BearerJwtProvider, SecurityProvider, SecurityRequest};
use crate::spec::SecurityScheme;

/// OAuth2 provider using the same simple JWT validation as `BearerJwtProvider`.
pub struct OAuth2Provider {
    signature: String,
    cookie_name: Option<String>,
}

impl OAuth2Provider {
    /// Create a new OAuth2 provider with the given signature
    ///
    /// Uses JWT validation similar to `BearerJwtProvider`. This is a simplified
    /// implementation for testing - production should use proper OAuth2 libraries.
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

    /// Configure the cookie name used to read the OAuth2 token
    ///
    /// # Arguments
    ///
    /// * `name` - Cookie name (e.g., "oauth_token")
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
}

/// OAuth2 provider implementation using JWT validation
///
/// Simplified OAuth2 provider that reuses `BearerJwtProvider` logic for token validation.
/// Supports both Authorization header and cookie-based tokens.
///
/// # Validation Flow
///
/// 1. Verify security scheme is OAuth2
/// 2. Extract token from cookie (if configured) or Authorization header
/// 3. Delegate validation to `BearerJwtProvider` logic
///
/// # Token Sources (Priority Order)
///
/// 1. **Cookie**: If `cookie_name()` is configured, read token from cookie
/// 2. **Authorization Header**: Falls back to `Authorization: Bearer {token}`
///
/// # Usage
///
/// ```rust
/// use brrtrouter::security::OAuth2Provider;
///
/// // Authorization header only
/// let provider = OAuth2Provider::new("secret_signature");
///
/// // Cookie-based (e.g., for browser SPAs)
/// let provider = OAuth2Provider::new("secret_signature")
///     .cookie_name("oauth_token");
/// ```
///
/// # Security
///
/// - ✅ Testing and development
/// - ✅ Internal APIs with controlled clients
/// - ❌ NOT for production OAuth2 flows (use proper OAuth2 library)
///
/// For production: Use `JwksBearerProvider` with proper JWKS validation.
impl SecurityProvider for OAuth2Provider {
    /// Validate an OAuth2 token (uses JWT validation internally)
    ///
    /// # Arguments
    ///
    /// * `scheme` - Security scheme from OpenAPI spec (must be OAuth2)
    /// * `scopes` - Required OAuth2 scopes from operation
    /// * `req` - The security request containing headers/cookies
    ///
    /// # Returns
    ///
    /// - `true` - Token is valid and contains required scopes
    /// - `false` - Token missing, invalid, or missing scopes
    fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::OAuth2 { .. } => {}
            _ => return false,
        }
        let token = match self.extract_token(req) {
            Some(t) => t,
            None => return false,
        };
        // Reuse BearerJwtProvider logic
        let helper = BearerJwtProvider {
            signature: self.signature.clone(),
            cookie_name: None,
        };
        helper.validate_token(token, scopes)
    }
}

