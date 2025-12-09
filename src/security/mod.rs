//! # Security Module
//!
//! The security module provides authentication and authorization providers for BRRTRouter,
//! implementing various security schemes defined in OpenAPI specifications.
//!
//! ## Overview
//!
//! This module implements the [`SecurityProvider`] trait for common authentication methods:
//! - **API Keys** - Header, query parameter, or cookie-based API keys
//! - **Bearer JWT** - JSON Web Token validation with signature verification
//! - **OAuth2** - OAuth2 token validation with scope checking
//!
//! Security providers are registered with the application and automatically enforced based on
//! the `security` requirements defined in your OpenAPI specification.
//!
//! ## Architecture
//!
//! Security validation follows this flow:
//!
//! 1. Request arrives with credentials (header, cookie, query param)
//! 2. Router determines which security scheme(s) are required for the route
//! 3. Appropriate [`SecurityProvider`] is invoked to validate credentials
//! 4. If validation succeeds, request proceeds to handler
//! 5. If validation fails, 401/403 response is returned
//!
//! ## Security Providers
//!
//! ### API Key Provider
//!
//! Validates simple API keys from headers, query parameters, or cookies:
//!
//! ```rust
//! use brrtrouter::security::{SecurityProvider, SecurityRequest};
//! use brrtrouter::spec::SecurityScheme;
//!
//! // Simple static API key validation
//! struct ApiKeyProvider { key: String }
//!
//! impl SecurityProvider for ApiKeyProvider {
//!     fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool {
//!         req.get_header("x-api-key")
//!             .map(|k| k == self.key)
//!             .unwrap_or(false)
//!     }
//! }
//! ```
//!
//! ### Bearer JWT Provider
//!
//! The [`BearerJwtProvider`] validates JWTs with:
//! - Signature verification
//! - Scope checking
//! - Cookie or header extraction
//!
//! ```rust
//! use brrtrouter::security::BearerJwtProvider;
//!
//! let provider = BearerJwtProvider::new("my-secret-signature")
//!     .cookie_name("auth_token");
//! ```
//!
//! ### OAuth2 Provider
//!
//! The [`OAuth2Provider`] validates OAuth2 tokens with scope checking:
//!
//! ```rust
//! use brrtrouter::security::OAuth2Provider;
//!
//! let provider = OAuth2Provider::new("oauth-signature");
//! ```
//!
//! ## Caching
//!
//! Security providers support optional caching to reduce validation overhead:
//! - Positive results can be cached to avoid repeated database/API lookups
//! - Negative results can be cached to prevent brute force attacks
//! - TTL-based expiration ensures credentials are re-validated periodically
//!
//! ## JWT Claims Extraction (BFF Pattern)
//!
//! For Backend-for-Frontend (BFF) architectures where a BFF service needs to forward
//! user context to downstream microservices, BRRTRouter provides JWT claims extraction.
//!
//! When a JWT token is successfully validated, the decoded claims are automatically
//! made available to handlers via `HandlerRequest::jwt_claims`. This enables:
//!
//! 1. **Accessing user information** in handlers (e.g., user ID, email, roles)
//! 2. **Forwarding claims to downstream services** as headers or in request bodies
//! 3. **Making authorization decisions** based on claims
//! 4. **Logging user context** for observability
//!
//! ### Example: Accessing Claims in Handlers
//!
//! ```rust,no_run
//! use brrtrouter::dispatcher::HandlerRequest;
//!
//! fn handler(req: HandlerRequest) {
//!     if let Some(claims) = &req.jwt_claims {
//!         let user_id = claims.get("sub").and_then(|v| v.as_str());
//!         let email = claims.get("email").and_then(|v| v.as_str());
//!         let org_id = claims.get("org_id").and_then(|v| v.as_str());
//!
//!         // Use claims for business logic
//!         println!("User {} ({}) from org {}",
//!                  user_id.unwrap_or("unknown"),
//!                  email.unwrap_or("unknown"),
//!                  org_id.unwrap_or("unknown"));
//!     }
//! }
//! ```
//!
//! ### Example: Forwarding Claims to Downstream Services
//!
//! ```rust,no_run
//! use brrtrouter::dispatcher::HandlerRequest;
//! use reqwest::blocking::Client;
//!
//! fn bff_handler(req: HandlerRequest) -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Client::new();
//!     let mut downstream_req = client.get("http://downstream-service/api/data");
//!
//!     // Forward JWT token (Option 1: Forward original token)
//!     if let Some(token) = req.get_header("authorization") {
//!         downstream_req = downstream_req.header("Authorization", token);
//!     }
//!
//!     // Forward claims as headers (Option 2: Extract and forward claims)
//!     if let Some(claims) = &req.jwt_claims {
//!         if let Some(user_id) = claims.get("sub").and_then(|v| v.as_str()) {
//!             downstream_req = downstream_req.header("X-User-ID", user_id);
//!         }
//!         if let Some(email) = claims.get("email").and_then(|v| v.as_str()) {
//!             downstream_req = downstream_req.header("X-User-Email", email);
//!         }
//!     }
//!
//!     let response = downstream_req.send()?;
//!     // ... handle response
//!     Ok(())
//! }
//! ```
//!
//! ### Claims Cache Performance
//!
//! JWT claims are cached after validation to avoid repeated decoding. The cache:
//! - Uses LRU eviction when capacity is reached
//! - Respects token expiration (with leeway)
//! - Invalidates on key rotation (via `kid` in cache key)
//! - Provides cache statistics via `JwksBearerProvider::cache_stats()`
//!
//! ## Example
//!
//! ```rust,ignore
//! // Example: Register a security provider (requires full server setup)
//! use brrtrouter::server::AppService;
//! use brrtrouter::security::BearerJwtProvider;
//! use std::sync::Arc;
//!
//! let jwt_provider = BearerJwtProvider::new("secret");
//! service.register_security_provider("bearerAuth", Arc::new(jwt_provider));
//! ```

use crate::dispatcher::HeaderVec;
use crate::router::ParamVec;
use crate::spec::SecurityScheme;
use serde_json::Value;

/// Cache statistics for JWT claims cache
#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    /// Number of cache hits (successful lookups)
    pub hits: u64,
    /// Number of cache misses (lookups that required decode)
    pub misses: u64,
    /// Number of entries evicted due to LRU capacity
    pub evictions: u64,
    /// Current number of entries in cache
    pub size: usize,
    /// Maximum capacity of cache
    pub capacity: usize,
}

impl CacheStats {
    /// Calculate cache hit rate as a percentage
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }
}

/// Request context for security validation.
///
/// Contains extracted credentials from various sources (headers, query, cookies)
/// that security providers can use to validate the request.
///
/// # JSF Compliance
///
/// Uses SmallVec (HeaderVec/ParamVec) references to avoid copying request data.
pub struct SecurityRequest<'a> {
    /// HTTP headers from the request (SmallVec for stack allocation)
    pub headers: &'a HeaderVec,
    /// Query parameters from the request URL (SmallVec for stack allocation)
    pub query: &'a ParamVec,
    /// Cookies from the request (SmallVec for stack allocation)
    pub cookies: &'a HeaderVec,
}

impl<'a> SecurityRequest<'a> {
    /// Get a header by name (case-insensitive)
    #[inline]
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Get a query parameter by name
    #[inline]
    pub fn get_query(&self, name: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }

    /// Get a cookie by name
    #[inline]
    pub fn get_cookie(&self, name: &str) -> Option<&str> {
        self.cookies
            .iter()
            .find(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Trait for implementing security validation providers.
///
/// Implement this trait to create custom authentication/authorization logic
/// for your OpenAPI security schemes.
pub trait SecurityProvider: Send + Sync {
    /// Validate a request against a security scheme.
    ///
    /// # Arguments
    ///
    /// * `scheme` - The OpenAPI security scheme definition
    /// * `scopes` - Required scopes for this operation (for OAuth2/OpenID)
    /// * `req` - The security request context with credentials
    ///
    /// # Returns
    ///
    /// `true` if the request is authenticated and authorized, `false` otherwise
    fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool;

    /// Extract claims from a validated request (optional).
    ///
    /// This method is called after `validate()` returns `true` to extract any
    /// claims or user information from the validated credentials. For JWT-based
    /// providers, this returns the decoded JWT claims. For other providers, this
    /// may return `None` or provider-specific information.
    ///
    /// # Arguments
    ///
    /// * `scheme` - The OpenAPI security scheme definition
    /// * `req` - The security request context with credentials
    ///
    /// # Returns
    ///
    /// * `Some(Value)` - The extracted claims/information as JSON
    /// * `None` - No claims available or provider doesn't support claims extraction
    ///
    /// # Default Implementation
    ///
    /// Returns `None` by default. Providers that support claims extraction should
    /// override this method.
    fn extract_claims(&self, scheme: &SecurityScheme, req: &SecurityRequest) -> Option<Value> {
        let _ = (scheme, req);
        None
    }
}

// Re-export all providers
pub use bearer_jwt::BearerJwtProvider;
pub use jwks_bearer::JwksBearerProvider;
pub use oauth2::OAuth2Provider;
pub use remote_api_key::RemoteApiKeyProvider;
pub use spiffe::{SpiffeProvider, InMemoryRevocationChecker, NoOpRevocationChecker, RevocationChecker};

// Provider modules
mod bearer_jwt;
mod jwks_bearer;
mod oauth2;
mod remote_api_key;
mod spiffe;
