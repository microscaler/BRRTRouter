//! # SPIFFE Security Provider
//!
//! This module provides SPIFFE (Secure Production Identity Framework for Everyone) support
//! for BRRTRouter, enabling workload identity validation and enterprise Windows single sign-on.
//!
//! ## Overview
//!
//! SPIFFE provides a framework for securely identifying and authenticating services in dynamic
//! environments. This provider validates SPIFFE JWT SVIDs (SPIFFE Verifiable Identity Documents)
//! and extracts SPIFFE IDs for authorization decisions.
//!
//! ## SPIFFE ID Format
//!
//! SPIFFE IDs follow the format: `spiffe://trust-domain/path`
//!
//! Examples:
//! - `spiffe://example.com/api/users`
//! - `spiffe://enterprise.local/windows/service/api`
//! - `spiffe://prod.example.com/frontend/web`
//!
//! ## JWT SVID Claims
//!
//! SPIFFE JWT SVIDs contain standard JWT claims plus SPIFFE-specific requirements:
//!
//! - `sub` (subject): **Required** - Must be a valid SPIFFE ID
//! - `aud` (audience): **Required** - Must match configured audiences
//! - `exp` (expiration): **Required** - Standard JWT expiration
//! - `iat` (issued at): **Required** - Standard JWT issued time
//! - `iss` (issuer): **Optional** - Trust domain (extracted from `sub` if not present)
//!
//! ## Usage
//!
//! ```rust
//! use brrtrouter::security::SpiffeProvider;
//!
//! let provider = SpiffeProvider::new()
//!     .trust_domains(&["example.com", "enterprise.local"])
//!     .audiences(&["api.example.com", "brrtrouter"])
//!     .leeway(60); // 60 seconds clock skew tolerance
//! ```
//!
//! ## Windows Enterprise SSO
//!
//! For Windows enterprise environments, SPIFFE IDs can be mapped to Windows user accounts
//! and integrated with Active Directory for seamless single sign-on.

mod validation;

use crate::security::{SecurityProvider, SecurityRequest};
use crate::spec::SecurityScheme;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;

/// SPIFFE security provider for JWT SVID validation.
///
/// Validates SPIFFE JWT SVIDs (SPIFFE Verifiable Identity Documents) and extracts
/// SPIFFE IDs for authorization decisions. Supports trust domain validation and
/// audience checking for enterprise security.
///
/// # Configuration
///
/// - **Trust Domains**: Whitelist of allowed trust domains (e.g., `["example.com"]`)
/// - **Audiences**: Required audiences that must be present in SVID (e.g., `["api.example.com"]`)
/// - **Leeway**: Clock skew tolerance in seconds (default: 60)
///
/// # Security
///
/// - ✅ SPIFFE ID format validation
/// - ✅ Trust domain whitelist enforcement
/// - ✅ Audience validation
/// - ✅ JWT signature verification (via JWKS or configured keys)
/// - ✅ Expiration checking with leeway
///
/// # Example
///
/// ```rust
/// use brrtrouter::security::SpiffeProvider;
///
/// let provider = SpiffeProvider::new()
///     .trust_domains(&["example.com", "enterprise.local"])
///     .audiences(&["api.example.com"])
///     .leeway(60);
/// ```
pub struct SpiffeProvider {
    /// Allowed trust domains (whitelist)
    trust_domains: Arc<HashSet<String>>,
    /// Required audiences (SVID must contain at least one)
    audiences: Arc<HashSet<String>>,
    /// Clock skew tolerance in seconds
    leeway_secs: u64,
    /// Optional JWKS URL for signature verification (if not provided, uses configured public key)
    jwks_url: Option<String>,
    /// Optional cookie name for token extraction
    cookie_name: Option<String>,
}

impl SpiffeProvider {
    /// Create a new SPIFFE provider with default configuration.
    ///
    /// Default configuration:
    /// - Empty trust domains (must be configured)
    /// - Empty audiences (must be configured)
    /// - 60 seconds leeway
    ///
    /// # Panics
    ///
    /// This will not panic, but validation will fail if trust domains or audiences
    /// are not configured. Use `trust_domains()` and `audiences()` to configure.
    pub fn new() -> Self {
        Self {
            trust_domains: Arc::new(HashSet::new()),
            audiences: Arc::new(HashSet::new()),
            leeway_secs: 60,
            jwks_url: None,
            cookie_name: None,
        }
    }

    /// Configure allowed trust domains.
    ///
    /// Trust domains are extracted from SPIFFE IDs (format: `spiffe://trust-domain/path`).
    /// Only SVIDs with trust domains in this whitelist will be accepted.
    ///
    /// # Arguments
    ///
    /// * `domains` - Slice of trust domain strings (e.g., `["example.com", "enterprise.local"]`)
    ///
    /// # Example
    ///
    /// ```rust
    /// use brrtrouter::security::SpiffeProvider;
    ///
    /// let provider = SpiffeProvider::new()
    ///     .trust_domains(&["example.com", "enterprise.local"]);
    /// ```
    pub fn trust_domains(mut self, domains: &[&str]) -> Self {
        self.trust_domains = Arc::new(domains.iter().map(|s| s.to_string()).collect());
        self
    }

    /// Configure required audiences.
    ///
    /// The SVID's `aud` claim must contain at least one of these audiences.
    /// If empty, audience validation is skipped.
    ///
    /// # Arguments
    ///
    /// * `auds` - Slice of audience strings (e.g., `["api.example.com", "brrtrouter"]`)
    ///
    /// # Example
    ///
    /// ```rust
    /// use brrtrouter::security::SpiffeProvider;
    ///
    /// let provider = SpiffeProvider::new()
    ///     .audiences(&["api.example.com"]);
    /// ```
    pub fn audiences(mut self, auds: &[&str]) -> Self {
        self.audiences = Arc::new(auds.iter().map(|s| s.to_string()).collect());
        self
    }

    /// Configure clock skew tolerance (leeway).
    ///
    /// This is the maximum time difference (in seconds) allowed between the server's
    /// clock and the token's `exp` claim. Default is 60 seconds.
    ///
    /// # Arguments
    ///
    /// * `secs` - Leeway in seconds
    pub fn leeway(mut self, secs: u64) -> Self {
        self.leeway_secs = secs;
        self
    }

    /// Configure JWKS URL for signature verification.
    ///
    /// If provided, JWT signatures will be verified using keys from this JWKS endpoint.
    /// If not provided, signature verification must be handled externally (e.g., via
    /// a separate JWT provider middleware).
    ///
    /// # Arguments
    ///
    /// * `url` - JWKS URL (e.g., `"https://spiffe.example.com/.well-known/jwks.json"`)
    pub fn jwks_url(mut self, url: impl Into<String>) -> Self {
        self.jwks_url = Some(url.into());
        self
    }

    /// Configure cookie name for token extraction.
    ///
    /// If provided, tokens will be read from this cookie in addition to the
    /// `Authorization: Bearer` header.
    ///
    /// # Arguments
    ///
    /// * `name` - Cookie name (e.g., `"spiffe_token"`)
    pub fn cookie_name(mut self, name: impl Into<String>) -> Self {
        self.cookie_name = Some(name.into());
        self
    }

    /// Extract SPIFFE ID from a validated request.
    ///
    /// This method extracts the SPIFFE ID from the `sub` claim of a validated SVID.
    /// Returns `None` if the token is invalid or missing.
    ///
    /// # Arguments
    ///
    /// * `req` - The security request context
    ///
    /// # Returns
    ///
    /// * `Some(spiffe_id)` - The SPIFFE ID (e.g., `"spiffe://example.com/api/users"`)
    /// * `None` - Token missing or invalid
    pub fn extract_spiffe_id(&self, req: &SecurityRequest) -> Option<String> {
        let token = self.extract_token(req)?;
        validation::extract_spiffe_id_from_token(token, self)
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

impl Default for SpiffeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SecurityProvider for SpiffeProvider {
    /// Validate a SPIFFE JWT SVID.
    ///
    /// Performs full validation including:
    /// 1. Token extraction from header or cookie
    /// 2. JWT signature verification (if JWKS URL configured)
    /// 3. SPIFFE ID format validation (`sub` claim)
    /// 4. Trust domain whitelist check
    /// 5. Audience validation
    /// 6. Expiration checking with leeway
    ///
    /// # Arguments
    ///
    /// * `scheme` - Security scheme from OpenAPI spec (must be HTTP Bearer)
    /// * `scopes` - Required OAuth2 scopes (not used for SPIFFE, but kept for trait compatibility)
    /// * `req` - The security request containing headers/cookies
    ///
    /// # Returns
    ///
    /// - `true` - SVID is valid and passes all checks
    /// - `false` - SVID missing, invalid format, wrong trust domain, or expired
    fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool {
        validation::validate_svid_impl(self, scheme, scopes, req)
    }

    /// Extract SPIFFE claims from a validated request.
    ///
    /// Returns the decoded JWT claims from a validated SPIFFE SVID, including:
    /// - `sub` - SPIFFE ID
    /// - `aud` - Audiences
    /// - `exp` - Expiration timestamp
    /// - `iat` - Issued at timestamp
    /// - `iss` - Issuer (if present)
    ///
    /// # Arguments
    ///
    /// * `scheme` - Security scheme from OpenAPI spec
    /// * `req` - The security request context
    ///
    /// # Returns
    ///
    /// * `Some(Value)` - Decoded JWT claims as JSON
    /// * `None` - Token missing, invalid, or not yet validated
    fn extract_claims(&self, _scheme: &SecurityScheme, req: &SecurityRequest) -> Option<Value> {
        let token = self.extract_token(req)?;
        validation::extract_claims_from_token(token, self)
    }
}

// Re-export validation module for testing
#[cfg(test)]
pub use validation::*;

