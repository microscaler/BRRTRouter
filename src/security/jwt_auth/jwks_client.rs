//! JWKS client interface for JWT validation.
//!
//! This module provides a trait-based interface for JWT validation using JWKS
//! (JSON Web Key Set) endpoints. The trait abstraction allows for:
//! - Production implementations using real JWKS endpoints
//! - Test implementations using pre-configured keys
//! - Mock implementations for unit testing
//!
//! **Story 4.2:** The JWT common-path middleware uses a `JwksClient` to:
//! 1. Fetch and cache public keys from a JWKS endpoint
//! 2. Validate JWT signatures against cached keys
//! 3. Extract and validate claims (typ, iss, aud, exp, nbf)
//! 4. Return `AccessClaims` for policy evaluation
//!
//! **Security Note (HACK-402, HACK-404):**
//! - JWT signature validation is the ONLY defense
//! - JWKS cache poisoning must be prevented
//! - Token validation MUST NEVER skip signature verification

use crate::security::jwt_auth::types::{AccessClaims, AuthError};

/// Client for JWT validation using JWKS (JSON Web Key Set) endpoints.
///
/// In production, implement this trait to fetch keys from a JWKS endpoint
/// (e.g., `https://auth.example.com/.well-known/jwks.json`).
///
/// # Test Use
///
/// For tests, implement this trait with pre-configured keys to avoid
/// network dependencies.
///
/// # Example
///
/// ```rust
/// use brrtrouter::security::jwt_auth::jwks_client::JwksClient;
/// use brrtrouter::security::jwt_auth::types::{AccessClaims, AuthError};
///
/// struct TestJwksClient;
///
/// impl JwksClient for TestJwksClient {
///     fn validate_and_extract_claims(
///         &self,
///         token: &str,
///     ) -> Result<AccessClaims, AuthError> {
///         // For testing, return pre-configured claims
///         Ok(AccessClaims {
///             sub: "user-123".to_string(),
///             tenant_id: "tenant-abc".to_string(),
///             user_type: "customer".to_string(),
///             sx: Default::default(),
///         })
///     }
/// }
/// ```
pub trait JwksClient: Send + Sync {
    /// Validate a JWT token and extract claims.
    ///
    /// This is the primary method used by the JWT common-path middleware.
    /// It performs signature validation (using cached JWKS keys), validates
    /// standard claims (typ, iss, aud, exp, nbf), and returns the decoded
    /// `AccessClaims`.
    ///
    /// # Arguments
    ///
    /// * `token` - The JWT token string to validate
    ///
    /// # Returns
    ///
    /// - `Ok(AccessClaims)` - Token is valid, claims extracted
    /// - `Err(AuthError::TokenExpired)` - Token has expired
    /// - `Err(AuthError::TokenInvalid)` - Signature validation failed or token is malformed
    ///
    /// # Security
    ///
    /// This method MUST always validate the JWT signature. It MUST NOT
    /// skip signature validation for any reason (HACK-408).
    fn validate_and_extract_claims(&self, token: &str) -> Result<AccessClaims, AuthError>;

    /// Get the JWKS client's trust domain or issuer.
    ///
    /// This is used for issuer validation during JWT verification.
    #[must_use]
    fn issuer(&self) -> Option<&str>;

    /// Get the JWKS client's audience claim requirement.
    ///
    /// This is used for audience validation during JWT verification.
    #[must_use]
    fn audience(&self) -> Option<&str>;
}
