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
//! use std::collections::HashMap;
//!
//! // Simple static API key validation
//! struct ApiKeyProvider { key: String }
//!
//! impl SecurityProvider for ApiKeyProvider {
//!     fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool {
//!         req.headers.get("X-API-Key")
//!             .map(|k| k == &self.key)
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
//! ## Example
//!
//! ```rust
//! use brrtrouter::server::AppService;
//! use brrtrouter::security::BearerJwtProvider;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let spec = brrtrouter::spec::load_spec("examples/openapi.yaml")?;
//! # let router = brrtrouter::router::Router::from_spec(&spec);
//! let mut service = AppService::new(router, spec);
//!
//! // Register security provider
//! let jwt_provider = BearerJwtProvider::new("secret");
//! service.register_security_provider("bearerAuth", Box::new(jwt_provider));
//! # Ok(())
//! # }
//! ```

use crate::spec::SecurityScheme;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Request context for security validation.
///
/// Contains extracted credentials from various sources (headers, query, cookies)
/// that security providers can use to validate the request.
pub struct SecurityRequest<'a> {
    /// HTTP headers from the request
    pub headers: &'a HashMap<String, String>,
    /// Query parameters from the request URL
    pub query: &'a HashMap<String, String>,
    /// Cookies from the request
    pub cookies: &'a HashMap<String, String>,
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
}

use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;

/// Simple Bearer/JWT provider that validates tokens embedded in the
/// `Authorization` header or a cookie.
///
/// Tokens are expected to have the form `header.payload.signature` where the
/// signature part must match the configured `signature` string. Only the
/// payload section is inspected for a whitespace separated `scope` field.
pub struct BearerJwtProvider {
    signature: String,
    cookie_name: Option<String>,
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
            if let Some(t) = req.cookies.get(name) {
                return Some(t);
            }
        }
        req.headers
            .get("authorization")
            .and_then(|h| h.strip_prefix("Bearer "))
    }

    fn validate_token(&self, token: &str, scopes: &[String]) -> bool {
        let mut parts = token.split('.');
        let header = parts.next();
        let payload = parts.next();
        let sig = parts.next();
        if header.is_none() || payload.is_none() || sig != Some(self.signature.as_str()) {
            return false;
        }
        let payload_bytes = match general_purpose::STANDARD.decode(payload.unwrap()) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let json: Value = match serde_json::from_slice(&payload_bytes) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let token_scopes = json.get("scope").and_then(|v| v.as_str()).unwrap_or("");
        scopes
            .iter()
            .all(|s| token_scopes.split_whitespace().any(|ts| ts == s))
    }
}

impl SecurityProvider for BearerJwtProvider {
    fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::Http { scheme, .. } if scheme.eq_ignore_ascii_case("bearer") => {}
            _ => return false,
        }
        let token = match self.extract_token(req) {
            Some(t) => t,
            None => return false,
        };
        self.validate_token(token, scopes)
    }
}

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
            if let Some(t) = req.cookies.get(name) {
                return Some(t);
            }
        }
        req.headers
            .get("authorization")
            .and_then(|h| h.strip_prefix("Bearer "))
    }
}

impl SecurityProvider for OAuth2Provider {
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

/// JWKS-based Bearer provider for production integrations.
/// Fetches keys from a JWKS URL and validates JWTs (signature and claims).
pub struct JwksBearerProvider {
    jwks_url: String,
    iss: Option<String>,
    aud: Option<String>,
    leeway_secs: u64,
    cache_ttl: Duration,
    // kid -> DecodingKey
    cache: std::sync::Mutex<(Instant, HashMap<String, jsonwebtoken::DecodingKey>)>,
}

impl JwksBearerProvider {
    /// Create a new JWKS-based Bearer provider
    ///
    /// Fetches JSON Web Key Sets from the provided URL and uses them to validate
    /// JWT signatures. This is the production-ready JWT validation provider.
    ///
    /// # Arguments
    ///
    /// * `jwks_url` - URL to fetch JWKS from (e.g., `https://example.auth0.com/.well-known/jwks.json`)
    pub fn new(jwks_url: impl Into<String>) -> Self {
        Self {
            jwks_url: jwks_url.into(),
            iss: None,
            aud: None,
            leeway_secs: 30,
            cache_ttl: Duration::from_secs(300),
            cache: std::sync::Mutex::new((
                Instant::now() - Duration::from_secs(1000),
                HashMap::new(),
            )),
        }
    }

    /// Configure the expected JWT issuer claim
    ///
    /// Validation will fail if the JWT `iss` claim doesn't match this value.
    pub fn issuer(mut self, iss: impl Into<String>) -> Self {
        self.iss = Some(iss.into());
        self
    }
    
    /// Configure the expected JWT audience claim
    ///
    /// Validation will fail if the JWT `aud` claim doesn't match this value.
    pub fn audience(mut self, aud: impl Into<String>) -> Self {
        self.aud = Some(aud.into());
        self
    }
    
    /// Configure leeway for time-based claims validation
    ///
    /// Allows some clock skew between client and server when validating exp, nbf, and iat claims.
    pub fn leeway(mut self, secs: u64) -> Self {
        self.leeway_secs = secs;
        self
    }
    
    /// Configure the TTL for cached JWKS keys
    ///
    /// Keys are cached to avoid repeated HTTP requests to the JWKS URL.
    pub fn cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    fn extract_token<'a>(&self, req: &'a SecurityRequest) -> Option<&'a str> {
        req.headers
            .get("authorization")
            .and_then(|h| h.strip_prefix("Bearer "))
    }

    fn refresh_jwks_if_needed(&self) {
        let mut guard = self.cache.lock().unwrap();
        let (last, map) = &mut *guard;
        if last.elapsed() < self.cache_ttl && !map.is_empty() {
            return;
        }
        drop(guard);
        // Fetch outside lock with brief retries to reduce flakiness in tests
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()
        {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut body_opt: Option<String> = None;
        for _ in 0..3 {
            match client.get(&self.jwks_url).send() {
                Ok(r) => match r.text() {
                    Ok(t) => {
                        body_opt = Some(t);
                        break;
                    }
                    Err(_) => {}
                },
                Err(_) => {}
            }
        }
        let body = match body_opt {
            Some(b) => b,
            None => return,
        };
        let parsed: serde_json::Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) => return,
        };
        let mut new_map: HashMap<String, jsonwebtoken::DecodingKey> = HashMap::new();
        if let Some(keys) = parsed.get("keys").and_then(|v| v.as_array()) {
            for k in keys {
                let kid = k.get("kid").and_then(|v| v.as_str()).unwrap_or("");
                let kty = k.get("kty").and_then(|v| v.as_str()).unwrap_or("");
                let alg = k.get("alg").and_then(|v| v.as_str()).unwrap_or("");
                // HMAC (oct) keys for HS* algorithms
                if kty.eq_ignore_ascii_case("oct")
                    && (alg.eq_ignore_ascii_case("HS256")
                        || alg.eq_ignore_ascii_case("HS384")
                        || alg.eq_ignore_ascii_case("HS512"))
                {
                    if let Some(kval) = k.get("k").and_then(|v| v.as_str()) {
                        // base64url decode secret
                        if let Ok(secret) =
                            base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(kval)
                        {
                            let dk = jsonwebtoken::DecodingKey::from_secret(&secret);
                            new_map.insert(kid.to_string(), dk);
                        }
                    }
                    continue;
                }
                // RSA public keys for RS* algorithms
                if kty.eq_ignore_ascii_case("RSA")
                    && (alg.eq_ignore_ascii_case("RS256")
                        || alg.eq_ignore_ascii_case("RS384")
                        || alg.eq_ignore_ascii_case("RS512"))
                {
                    let n = match k.get("n").and_then(|v| v.as_str()) {
                        Some(v) => v,
                        None => continue,
                    };
                    let e = match k.get("e").and_then(|v| v.as_str()) {
                        Some(v) => v,
                        None => continue,
                    };
                    // jsonwebtoken expects base64url-encoded components for RSA
                    if let Ok(dk) = jsonwebtoken::DecodingKey::from_rsa_components(n, e) {
                        new_map.insert(kid.to_string(), dk);
                    }
                    continue;
                }
                // Unsupported kty/alg combinations are skipped
            }
        }
        let mut guard = self.cache.lock().unwrap();
        *guard = (Instant::now(), new_map);
    }

    fn get_key_for(&self, kid: &str) -> Option<jsonwebtoken::DecodingKey> {
        self.refresh_jwks_if_needed();
        let guard = self.cache.lock().unwrap();
        guard.1.get(kid).cloned()
    }
}

impl SecurityProvider for JwksBearerProvider {
    fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool {
        match scheme {
            SecurityScheme::Http { scheme, .. } if scheme.eq_ignore_ascii_case("bearer") => {}
            _ => return false,
        }
        let token = match self.extract_token(req) {
            Some(t) => t,
            None => return false,
        };
        // Parse header to locate kid/alg
        let header = match jsonwebtoken::decode_header(token) {
            Ok(h) => h,
            Err(_) => return false,
        };
        let kid = match header.kid {
            Some(k) => k,
            None => return false,
        };
        let key = match self.get_key_for(&kid) {
            Some(k) => k,
            None => return false,
        };
        let selected_alg = match header.alg {
            jsonwebtoken::Algorithm::HS256 => jsonwebtoken::Algorithm::HS256,
            jsonwebtoken::Algorithm::HS384 => jsonwebtoken::Algorithm::HS384,
            jsonwebtoken::Algorithm::HS512 => jsonwebtoken::Algorithm::HS512,
            jsonwebtoken::Algorithm::RS256 => jsonwebtoken::Algorithm::RS256,
            jsonwebtoken::Algorithm::RS384 => jsonwebtoken::Algorithm::RS384,
            jsonwebtoken::Algorithm::RS512 => jsonwebtoken::Algorithm::RS512,
            _ => return false,
        };
        let mut validation = jsonwebtoken::Validation::new(selected_alg);
        validation.validate_exp = true;
        validation.set_required_spec_claims(&["exp"]);
        validation.leeway = self.leeway_secs;
        if let Some(ref iss) = self.iss {
            validation.set_issuer(&[iss]);
        }
        if let Some(ref aud) = self.aud {
            validation.set_audience(&[aud]);
        }
        let data: Result<jsonwebtoken::TokenData<serde_json::Value>, _> =
            jsonwebtoken::decode(token, &key, &validation);
        let claims = match data {
            Ok(d) => d.claims,
            Err(_) => return false,
        };
        // scope check
        let token_scopes = claims.get("scope").and_then(|v| v.as_str()).unwrap_or("");
        scopes
            .iter()
            .all(|s| token_scopes.split_whitespace().any(|ts| ts == s))
    }
}

/// Remote API key verification provider with simple caching.
pub struct RemoteApiKeyProvider {
    verify_url: String,
    timeout_ms: u64,
    cache_ttl: Duration,
    cache: std::sync::Mutex<HashMap<String, (Instant, bool)>>,
    header_name: String,
}

impl RemoteApiKeyProvider {
    /// Create a new remote API key provider
    ///
    /// Validates API keys by making HTTP requests to an external verification service.
    /// Results are cached to reduce latency and load on the verification service.
    ///
    /// # Arguments
    ///
    /// * `verify_url` - URL of the verification service (e.g., `https://api.example.com/verify`)
    pub fn new(verify_url: impl Into<String>) -> Self {
        Self {
            verify_url: verify_url.into(),
            timeout_ms: 500,
            cache_ttl: Duration::from_secs(60),
            cache: std::sync::Mutex::new(HashMap::new()),
            header_name: "x-api-key".to_string(),
        }
    }
    
    /// Configure the HTTP request timeout in milliseconds
    ///
    /// Default: 500ms
    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }
    
    /// Configure the TTL for cached validation results
    ///
    /// Default: 60 seconds
    pub fn cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }
    
    /// Configure the header name to look for the API key
    ///
    /// Default: `x-api-key`
    pub fn header_name(mut self, name: impl Into<String>) -> Self {
        self.header_name = name.into().to_ascii_lowercase();
        self
    }

    fn extract_key<'a>(&self, req: &'a SecurityRequest, header_name: &str) -> Option<&'a str> {
        // Prefer named header, also accept Authorization: Bearer <key>
        req.headers
            .get(header_name)
            .map(|s| s.as_str())
            .or_else(|| {
                req.headers
                    .get("authorization")
                    .and_then(|h| h.strip_prefix("Bearer "))
            })
    }
}

impl SecurityProvider for RemoteApiKeyProvider {
    fn validate(&self, scheme: &SecurityScheme, _scopes: &[String], req: &SecurityRequest) -> bool {
        let name = match scheme {
            SecurityScheme::ApiKey { name, location, .. } if location == "header" => {
                name.to_ascii_lowercase()
            }
            _ => return false,
        };
        let key = match self
            .extract_key(req, &self.header_name)
            .or_else(|| self.extract_key(req, &name))
        {
            Some(k) => k,
            None => return false,
        };
        // Cache lookup
        if let Some((ts, ok)) = self.cache.lock().unwrap().get(key).cloned() {
            if ts.elapsed() < self.cache_ttl {
                return ok;
            }
        }
        // Remote verify
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(self.timeout_ms))
            .build();
        let ok = match client {
            Ok(c) => match c.get(&self.verify_url).header("X-API-Key", key).send() {
                Ok(r) => r.status().is_success(),
                Err(_) => false,
            },
            Err(_) => false,
        };
        self.cache
            .lock()
            .unwrap()
            .insert(key.to_string(), (Instant::now(), ok));
        ok
    }
}
