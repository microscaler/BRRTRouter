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
mod revocation;

use crate::security::{SecurityProvider, SecurityRequest};
use crate::spec::SecurityScheme;
use serde_json::Value;
use std::collections::HashSet;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{RwLock, Mutex, Condvar};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use url::Url;
use tracing::debug;

pub use revocation::{RevocationChecker, InMemoryRevocationChecker, NoOpRevocationChecker};

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
/// - **JWKS URL**: Optional URL for signature verification (if not provided, signature verification is skipped)
///
/// # Security
///
/// - ✅ SPIFFE ID format validation
/// - ✅ Trust domain whitelist enforcement
/// - ✅ Audience validation
/// - ✅ JWT signature verification (via JWKS if configured)
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
///     .jwks_url("https://spiffe.example.com/.well-known/jwks.json")
///     .leeway(60);
/// ```
pub struct SpiffeProvider {
    /// Allowed trust domains (whitelist)
    trust_domains: Arc<HashSet<String>>,
    /// Required audiences (SVID must contain at least one)
    audiences: Arc<HashSet<String>>,
    /// Clock skew tolerance in seconds
    leeway_secs: u64,
    /// Optional JWKS URL for signature verification
    jwks_url: Option<String>,
    /// Optional cookie name for token extraction
    cookie_name: Option<String>,
    /// JWKS cache: (timestamp, kid -> DecodingKey)
    /// Only used if jwks_url is Some
    jwks_cache: Option<Arc<RwLock<(Instant, HashMap<String, jsonwebtoken::DecodingKey>)>>>,
    /// JWKS cache TTL
    jwks_cache_ttl: Duration,
    /// Debounce flag for JWKS refresh
    jwks_refresh_in_progress: Option<Arc<AtomicBool>>,
    /// Condition variable for JWKS refresh completion
    jwks_refresh_complete: Option<Arc<(Mutex<()>, Condvar)>>,
    /// Optional revocation checker for token revocation
    revocation_checker: Option<Arc<dyn RevocationChecker>>,
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
            jwks_cache: None,
            jwks_cache_ttl: Duration::from_secs(3600),
            jwks_refresh_in_progress: None,
            jwks_refresh_complete: None,
            revocation_checker: None,
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
    /// If not provided, signature verification is skipped (claims-only validation).
    ///
    /// # Arguments
    ///
    /// * `url` - JWKS URL (e.g., `"https://spiffe.example.com/.well-known/jwks.json"`)
    ///
    /// # Security
    ///
    /// JWKS URL must use HTTPS (validated in this method). HTTP URLs are rejected for security,
    /// except for localhost/127.0.0.1 for testing.
    ///
    /// JSF Compliance: Panics only during initialization, never on hot path
    /// This method is only called during provider construction (startup)
    #[allow(clippy::panic)]
    pub fn jwks_url(mut self, url: impl Into<String>) -> Self {
        let url_str = url.into();
        
        // Validate JWKS URL (same validation as JwksBearerProvider)
        // This panic is intentional: invalid configuration should fail fast at startup
        let parsed_url = match Url::parse(&url_str) {
            Ok(u) => u,
            Err(e) => {
                panic!("JWKS URL is invalid: {}. Error: {}", url_str, e);
            }
        };
        
        // Allow HTTPS for all hosts
        if parsed_url.scheme() == "https" {
            // HTTPS is always allowed
        } else if parsed_url.scheme() == "http" {
            // HTTP only allowed for exact localhost or 127.0.0.1
            // This panic is intentional: invalid configuration should fail fast at startup
            let host = match parsed_url.host_str() {
                Some(h) => h,
                None => {
                    panic!("JWKS URL must have a valid hostname. Got: {}", url_str);
                }
            };
            
            // This panic is intentional: invalid configuration should fail fast at startup
            if host != "localhost" && host != "127.0.0.1" {
                panic!("JWKS URL must use HTTPS for security (HTTP only allowed for localhost/127.0.0.1). Got: {}", url_str);
            }
        } else {
            // This panic is intentional: invalid configuration should fail fast at startup
            panic!(
                "JWKS URL must use HTTPS or HTTP (for localhost only). Got: {}",
                url_str
            );
        }
        
        // Initialize JWKS infrastructure
        let cache = Arc::new(RwLock::new((
            Instant::now() - Duration::from_secs(1000), // Start expired to trigger initial fetch
            HashMap::new(),
        )));
        let refresh_in_progress = Arc::new(AtomicBool::new(false));
        let refresh_complete = Arc::new((Mutex::new(()), Condvar::new()));
        
        self.jwks_url = Some(url_str);
        self.jwks_cache = Some(cache);
        self.jwks_refresh_in_progress = Some(refresh_in_progress);
        self.jwks_refresh_complete = Some(refresh_complete);
        
        self
    }
    
    /// Configure JWKS cache TTL.
    ///
    /// Default is 3600 seconds (1 hour). Keys are automatically refreshed when cache expires.
    ///
    /// # Arguments
    ///
    /// * `ttl` - Cache TTL in seconds
    pub fn jwks_cache_ttl(mut self, ttl_secs: u64) -> Self {
        self.jwks_cache_ttl = Duration::from_secs(ttl_secs);
        self
    }
    
    /// Get decoding key for a given key ID (kid).
    ///
    /// This is used internally for signature verification.
    /// Returns None if key not found or JWKS not configured.
    ///
    /// If cache is empty, triggers blocking refresh and waits for completion.
    pub(super) fn get_key_for(&self, kid: &str) -> Option<jsonwebtoken::DecodingKey> {
        let (cache, refresh_complete) = match (
            &self.jwks_cache,
            &self.jwks_refresh_complete,
        ) {
            (Some(c), Some(rc)) => (c, rc),
            _ => return None, // JWKS not configured
        };
        
        // Check if cache is empty - if so, we need to wait for refresh
        let is_empty = {
            if let Ok(guard) = cache.read() {
                guard.1.is_empty()
            } else {
                return None;
            }
        };
        
        // Trigger refresh if needed
        self.refresh_jwks_if_needed();
        
        // If cache was empty, wait for refresh to complete
        if is_empty {
            let (lock, cvar) = &**refresh_complete;
            let guard = lock.lock().unwrap();
            let _ = cvar.wait_timeout(guard, Duration::from_secs(5));
        }
        
        // Read from cache
        if let Ok(guard) = cache.read() {
            guard.1.get(kid).cloned()
        } else {
            None
        }
    }
    
    /// Refresh JWKS if cache is expired or empty.
    ///
    /// If cache is empty, does a blocking initial refresh to ensure first validation succeeds.
    /// Otherwise, triggers refresh in background thread.
    fn refresh_jwks_if_needed(&self) {
        let (cache, refresh_in_progress, refresh_complete, jwks_url) = match (
            &self.jwks_cache,
            &self.jwks_refresh_in_progress,
            &self.jwks_refresh_complete,
            &self.jwks_url,
        ) {
            (Some(c), Some(r), Some(rc), Some(url)) => (c, r, rc, url),
            _ => return, // JWKS not configured
        };
        
        // Check if refresh is needed
        let (needs_refresh, is_empty) = {
            if let Ok(guard) = cache.read() {
                (guard.0.elapsed() >= self.jwks_cache_ttl || guard.1.is_empty(), guard.1.is_empty())
            } else {
                return;
            }
        };
        
        if !needs_refresh {
            return;
        }
        
        // If cache is empty, do blocking refresh to ensure first validation succeeds
        if is_empty {
            // Try to claim refresh
            if refresh_in_progress
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                // We won the race - do blocking initial refresh
                validation::refresh_jwks_internal(
                    cache,
                    jwks_url,
                    refresh_in_progress,
                    refresh_complete,
                    true, // Already claimed
                );
                return;
            } else {
                // Another thread is refreshing - wait for it
                let (lock, cvar) = &**refresh_complete;
                let guard = lock.lock().unwrap();
                let _ = cvar.wait_timeout(guard, Duration::from_secs(5));
                return;
            }
        }
        
        // Cache not empty but expired - trigger background refresh
        if refresh_in_progress
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Another thread is refreshing
            return;
        }
        
        // Spawn thread to refresh JWKS
        let cache_clone = Arc::clone(cache);
        let refresh_in_progress_clone = Arc::clone(refresh_in_progress);
        let refresh_complete_clone = Arc::clone(refresh_complete);
        let jwks_url_clone = jwks_url.clone();
        
        // Clone for error handler
        let refresh_in_progress_err = Arc::clone(refresh_in_progress);
        let refresh_complete_err = Arc::clone(refresh_complete);
        
        // Use thread::Builder to handle spawn failures
        let _ = std::thread::Builder::new()
            .name("spiffe-jwks-refresh".to_string())
            .spawn(move || {
                validation::refresh_jwks_internal(
                    &cache_clone,
                    &jwks_url_clone,
                    &refresh_in_progress_clone,
                    &refresh_complete_clone,
                    true, // Already claimed
                );
            })
            .map_err(move |e| {
                // Clear flag on spawn failure
                refresh_in_progress_err.store(false, Ordering::Release);
                let (lock, cvar) = &*refresh_complete_err;
                let _guard = lock.lock().unwrap();
                cvar.notify_all();
                debug!("Failed to spawn SPIFFE JWKS refresh thread: {}", e);
            });
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

    /// Configure revocation checker for token revocation.
    ///
    /// When provided, the provider will check if a token's JWT ID (jti) has been
    /// revoked before accepting it. This is critical for enterprise security,
    /// allowing immediate revocation of compromised tokens.
    ///
    /// # Arguments
    ///
    /// * `checker` - Implementation of `RevocationChecker` trait
    ///
    /// # Example
    ///
    /// ```rust
    /// use brrtrouter::security::spiffe::{SpiffeProvider, InMemoryRevocationChecker};
    ///
    /// let checker = InMemoryRevocationChecker::new();
    /// checker.revoke("compromised-token-id");
    ///
    /// let provider = SpiffeProvider::new()
    ///     .trust_domains(&["example.com"])
    ///     .revocation_checker(checker);
    /// ```
    pub fn revocation_checker(mut self, checker: impl RevocationChecker + 'static) -> Self {
        self.revocation_checker = Some(Arc::new(checker));
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

    /// Extract JWT ID (jti) from a validated request.
    ///
    /// The `jti` claim provides a unique identifier for the token, which is essential for:
    /// - **Token revocation**: Blacklist specific tokens without waiting for expiration
    /// - **Audit logging**: Track which tokens were used for which operations
    /// - **Replay prevention**: Detect and reject reused tokens
    /// - **Security incident response**: Quickly revoke compromised tokens
    ///
    /// This is particularly valuable in microservice-to-microservice identity scenarios
    /// where fine-grained revocation and audit trails are required (e.g., Pricewhisperer).
    ///
    /// # Arguments
    ///
    /// * `req` - The security request context
    ///
    /// # Returns
    ///
    /// * `Some(jti)` - The JWT ID claim value (as string)
    /// * `None` - Token missing, invalid, or `jti` claim not present
    ///
    /// # Example
    ///
    /// ```rust
    /// use brrtrouter::security::SpiffeProvider;
    ///
    /// let provider = SpiffeProvider::new()
    ///     .trust_domains(&["example.com"])
    ///     .audiences(&["api.example.com"]);
    ///
    /// // After validation, extract jti for revocation checking
    /// if let Some(jti) = provider.extract_jti(&req) {
    ///     // Check revocation list
    ///     if is_revoked(jti) {
    ///         return Err("Token revoked");
    ///     }
    ///     // Log for audit trail
    ///     audit_log("token_used", jti);
    /// }
    /// ```
    pub fn extract_jti(&self, req: &SecurityRequest) -> Option<String> {
        let token = self.extract_token(req)?;
        validation::extract_jti_from_token(token)
    }

    fn extract_token<'a>(&self, req: &'a SecurityRequest) -> Option<&'a str> {
        if let Some(name) = &self.cookie_name {
            if let Some(t) = req.get_cookie(name) {
                return Some(t);
            }
        }
        // RFC 6750: Bearer token must be exactly "Bearer " (single space) followed by token
        // Reject double spaces, tabs, newlines, trailing whitespace, etc.
        // Note: "Bearer" prefix is case-sensitive per RFC 6750 Section 2.1
        req.get_header("authorization")
            .and_then(|h| {
                // Check for exact "Bearer " prefix (case-sensitive)
                if h.len() < 7 {
                    return None; // Too short to be "Bearer "
                }
                if !h.starts_with("Bearer ") {
                    return None;
                }
                // Ensure it's exactly "Bearer " (single space), not "Bearer  " (double space)
                // Check that character at index 6 is a space
                if h.as_bytes().get(6) != Some(&b' ') {
                    return None;
                }
                // If there's a character at index 7, it must not be whitespace
                if let Some(&b' ') = h.as_bytes().get(7) {
                    return None; // Double space - reject
                }
                let token = &h[7..];
                // Reject if token has leading or trailing whitespace
                if token.starts_with(char::is_whitespace) || token.ends_with(char::is_whitespace) {
                    return None;
                }
                Some(token)
            })
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
    /// - `jti` - JWT ID (if present) - useful for revocation and audit logging
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

