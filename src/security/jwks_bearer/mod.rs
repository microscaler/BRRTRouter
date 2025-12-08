mod validation;

use crate::security::{CacheStats, SecurityProvider, SecurityRequest};
use crate::spec::SecurityScheme;
use base64::Engine as _;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use url::Url;

// P3: Supported JWT algorithms - whitelist for security and code simplification
pub(super) const SUPPORTED_ALGORITHMS: &[jsonwebtoken::Algorithm] = &[
    jsonwebtoken::Algorithm::HS256,
    jsonwebtoken::Algorithm::HS384,
    jsonwebtoken::Algorithm::HS512,
    jsonwebtoken::Algorithm::RS256,
    jsonwebtoken::Algorithm::RS384,
    jsonwebtoken::Algorithm::RS512,
];

/// JWKS-based Bearer provider for production integrations.
/// Fetches keys from a JWKS URL and validates JWTs (signature and claims).
pub struct JwksBearerProvider {
    jwks_url: String,
    pub(super) iss: Option<String>,
    pub(super) aud: Option<String>,
    pub(super) leeway_secs: u64,
    cache_ttl: Duration,
    // kid -> DecodingKey
    cache: std::sync::Mutex<(Instant, HashMap<String, jsonwebtoken::DecodingKey>)>,
    // P2: Debounce JWKS refresh to prevent concurrent HTTP requests
    refresh_in_progress: AtomicBool,
    // JSF P2: Cache decoded JWT claims to avoid repeated decode operations
    // Uses LRU cache with Arc<str> keys to prevent memory leaks and avoid allocations
    // P1: RwLock for explicit read/write separation (LruCache::get() requires &mut for LRU updates)
    // SECURITY: Cache key includes kid (key ID) so cache invalidates on key rotation
    // Format: "token|kid" -> (exp_timestamp_with_leeway, decoded_claims, kid)
    pub(super) claims_cache:
        std::sync::RwLock<LruCache<Arc<str>, (i64, serde_json::Value, String)>>,
    claims_cache_size: usize,
    cookie_name: Option<String>,
    // P2: Cache metrics for observability and tuning
    pub(super) cache_hits: AtomicU64,
    pub(super) cache_misses: AtomicU64,
    pub(super) cache_evictions: AtomicU64,
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
    ///
    /// # Security
    ///
    /// JWKS URL must use HTTPS (validated in `new()`). HTTP URLs are rejected for security.
    pub fn new(jwks_url: impl Into<String>) -> Self {
        let url_str = jwks_url.into();

        // P4 Security: Validate JWKS URL requires HTTPS (except localhost for testing)
        // SECURITY FIX: Parse URL properly to prevent hostname prefix attacks (e.g., localhost.attacker.com)
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
            // HTTP only allowed for exact localhost or 127.0.0.1 (not subdomains)
            let host = match parsed_url.host_str() {
                Some(h) => h,
                None => {
                    panic!("JWKS URL must have a valid hostname. Got: {}", url_str);
                }
            };

            // Only allow exact "localhost" or "127.0.0.1" - reject subdomains like "localhost.attacker.com"
            if host != "localhost" && host != "127.0.0.1" {
                panic!("JWKS URL must use HTTPS for security (HTTP only allowed for localhost/127.0.0.1). Got: {}", url_str);
            }
        } else {
            panic!(
                "JWKS URL must use HTTPS or HTTP (for localhost only). Got: {}",
                url_str
            );
        }

        Self {
            jwks_url: url_str,
            iss: None,
            aud: None,
            leeway_secs: 30,
            cache_ttl: Duration::from_secs(300),
            cache: std::sync::Mutex::new((
                Instant::now() - Duration::from_secs(1000),
                HashMap::new(),
            )),
            refresh_in_progress: AtomicBool::new(false),
            claims_cache: std::sync::RwLock::new(LruCache::new(
                NonZeroUsize::new(1000).expect("claims_cache_size must be > 0"),
            )),
            claims_cache_size: 1000,
            cookie_name: None,
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            cache_evictions: AtomicU64::new(0),
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

    /// Configure the cookie name used to read the token.
    ///
    /// If set, tokens will be extracted from cookies in addition to the Authorization header.
    /// Cookie extraction takes precedence over header extraction.
    ///
    /// # Arguments
    ///
    /// * `name` - Cookie name to look for (e.g., "auth_token")
    pub fn cookie_name(mut self, name: impl Into<String>) -> Self {
        self.cookie_name = Some(name.into());
        self
    }

    /// Configure the maximum size of the claims cache.
    ///
    /// When the cache reaches this size, least-recently-used entries are evicted.
    /// Default: 1000 entries.
    ///
    /// # Arguments
    ///
    /// * `size` - Maximum number of cached token claims
    pub fn claims_cache_size(mut self, size: usize) -> Self {
        if size == 0 {
            panic!("claims_cache_size must be > 0");
        }
        self.claims_cache_size = size;
        {
            let mut guard = self
                .claims_cache
                .write()
                .expect("Claims cache RwLock poisoned - critical error");
            *guard = LruCache::new(NonZeroUsize::new(size).unwrap());
        }
        self
    }

    /// Clear all cached JWT claims.
    ///
    /// Useful for testing, key rotation, or security incidents where tokens need to be invalidated.
    pub fn clear_claims_cache(&self) {
        if let Ok(mut guard) = self.claims_cache.write() {
            guard.clear();
        }
    }

    /// Invalidate a specific token from the claims cache.
    ///
    /// Useful when a token is revoked or needs to be re-validated immediately.
    /// This method extracts the key ID (kid) from the token header and invalidates
    /// only that specific token entry, avoiding the thundering herd problem of
    /// clearing the entire cache.
    ///
    /// # Arguments
    ///
    /// * `token` - The JWT token string to invalidate
    ///
    /// # Note
    ///
    /// If the token cannot be parsed (missing or invalid header), this method
    /// will log a warning and return without invalidating. Tokens without valid
    /// headers are not cached, so this is safe. For manual invalidation with a
    /// known key ID, use `invalidate_token_with_kid()`.
    pub fn invalidate_token(&self, token: &str) {
        // SECURITY: Cache key format is "token|kid", so we need to extract kid from token
        // Parse the token header to get the kid
        let header = match jsonwebtoken::decode_header(token) {
            Ok(h) => h,
            Err(e) => {
                warn!(
                    "JWT invalidation failed: cannot parse token header - {:?}. \
                     Token may not be cached, skipping invalidation.",
                    e
                );
                return;
            }
        };

        let kid = match header.kid {
            Some(k) => k,
            None => {
                warn!(
                    "JWT invalidation failed: missing 'kid' (key ID) in token header. \
                     Tokens without kids are not cached, skipping invalidation."
                );
                return;
            }
        };

        // Use the more precise invalidation method with the extracted kid
        self.invalidate_token_with_kid(token, &kid);
    }

    /// Invalidate a specific token with a specific key ID from the claims cache.
    ///
    /// More precise than `invalidate_token()` - only invalidates the token for the given key ID.
    ///
    /// # Arguments
    ///
    /// * `token` - The JWT token string to invalidate
    /// * `kid` - The key ID to invalidate
    pub fn invalidate_token_with_kid(&self, token: &str, kid: &str) {
        // SECURITY: Cache key format is "token|kid"
        let token_key: Arc<str> = Arc::from(format!("{}|{}", token, kid));
        if let Ok(mut guard) = self.claims_cache.write() {
            guard.pop(&token_key);
        }
    }

    /// Get cache statistics for observability and tuning.
    ///
    /// Returns hit/miss counts, evictions, and current cache size.
    ///
    /// # Returns
    ///
    /// A struct containing cache metrics:
    /// - `hits`: Number of cache hits (successful lookups)
    /// - `misses`: Number of cache misses (lookups that required decode)
    /// - `evictions`: Number of entries evicted due to LRU capacity
    /// - `size`: Current number of entries in cache
    /// - `capacity`: Maximum cache capacity
    pub fn cache_stats(&self) -> CacheStats {
        let cache_size = self
            .claims_cache
            .read()
            .map(|guard| guard.len())
            .unwrap_or(0);
        CacheStats {
            hits: self.cache_hits.load(Ordering::Relaxed),
            misses: self.cache_misses.load(Ordering::Relaxed),
            evictions: self.cache_evictions.load(Ordering::Relaxed),
            size: cache_size,
            capacity: self.claims_cache_size,
        }
    }

    pub(super) fn extract_token<'a>(&self, req: &'a SecurityRequest) -> Option<&'a str> {
        // P2: Cookie support - check cookie first if configured
        if let Some(name) = &self.cookie_name {
            if let Some(t) = req.get_cookie(name) {
                return Some(t);
            }
        }
        // Fall back to Authorization header
        req.get_header("authorization")
            .and_then(|h| h.strip_prefix("Bearer "))
    }

    fn refresh_jwks_if_needed(&self) {
        let mut guard = self
            .cache
            .lock()
            .expect("JWKS cache Mutex poisoned - critical error");
        let (last, map) = &mut *guard;
        if last.elapsed() < self.cache_ttl && !map.is_empty() {
            return;
        }
        drop(guard);

        // P2: Debounce - check if another thread is already refreshing
        // Use compare_and_swap to atomically set the flag
        if self
            .refresh_in_progress
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Another thread is refreshing, wait for it to complete
            // HTTP requests can take up to 400ms (200ms timeout × 2 retries),
            // so we poll with exponential backoff until refresh completes
            let start = Instant::now();
            let timeout = Duration::from_secs(1); // Allow 1s for refresh (400ms max + buffer)
            let mut wait_ms = 10; // Start with 10ms

            while self.refresh_in_progress.load(Ordering::Acquire) {
                if start.elapsed() >= timeout {
                    // Timeout - refresh may have failed, proceed to read cache anyway
                    // (will use stale data or fail validation, which is acceptable)
                    warn!("JWKS refresh timeout after 2s, proceeding with stale cache");
                    return;
                }
                std::thread::sleep(Duration::from_millis(wait_ms));
                // Exponential backoff: 10ms, 20ms, 40ms, 80ms, 100ms (capped)
                wait_ms = (wait_ms * 2).min(100);
            }
            // Refresh completed, cache is now fresh - return to allow caller to read it
            return;
        }

        // We're the thread doing the refresh - proceed
        // The flag will be cleared when we're done (success or failure)
        // P1: Optimized timeout and retries for faster failure (200ms timeout, 2 retries)
        // This reduces maximum blocking time from 1.5s (500ms × 3) to 400ms (200ms × 2)
        let refresh_start = Instant::now();
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(200))
            .build()
        {
            Ok(c) => c,
            Err(_) => {
                // P2: Clear refresh flag on client build failure
                self.refresh_in_progress.store(false, Ordering::Release);
                return;
            }
        };
        let mut body_opt: Option<String> = None;
        for _ in 0..2 {
            if let Ok(r) = client.get(&self.jwks_url).send() {
                if let Ok(t) = r.text() {
                    body_opt = Some(t);
                    break;
                }
            }
        }
        let body = match body_opt {
            Some(b) => b,
            None => {
                // P2: Clear refresh flag on failure
                self.refresh_in_progress.store(false, Ordering::Release);
                return;
            }
        };
        let parsed: serde_json::Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) => {
                // P2: Clear refresh flag on parse failure
                self.refresh_in_progress.store(false, Ordering::Release);
                return;
            }
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
        // P1: Log refresh latency for observability (get key count before moving)
        let key_count = new_map.len();
        let refresh_duration = refresh_start.elapsed();

        let mut guard = self
            .cache
            .lock()
            .expect("JWKS cache Mutex poisoned - critical error");
        *guard = (Instant::now(), new_map);

        // P2: Clear refresh flag on success
        self.refresh_in_progress.store(false, Ordering::Release);

        debug!(
            "JWKS refresh completed in {:?} (keys: {})",
            refresh_duration, key_count
        );
    }

    pub(super) fn get_key_for(&self, kid: &str) -> Option<jsonwebtoken::DecodingKey> {
        self.refresh_jwks_if_needed();
        let guard = self
            .cache
            .lock()
            .expect("JWKS cache Mutex poisoned - critical error");
        guard.1.get(kid).cloned()
    }
}

/// JWKS-based Bearer JWT provider implementation
///
/// Production-grade JWT validation using JSON Web Key Sets (JWKS).
/// Fetches public keys from a JWKS endpoint and validates JWTs using proper cryptography.
///
/// # Validation Flow
///
/// 1. Verify security scheme is HTTP Bearer
/// 2. Extract token from Authorization header or cookie
/// 3. Parse JWT header to get `kid` (key ID) and `alg` (algorithm)
/// 4. Fetch decoding key from JWKS cache (refreshes if expired)
/// 5. Validate token signature using `jsonwebtoken` crate
/// 6. Verify issuer (`iss`), audience (`aud`), expiration (`exp`)
/// 7. Check required scopes in `scope` claim
///
/// # Supported Algorithms
///
/// - **HMAC**: HS256, HS384, HS512 (symmetric keys)
/// - **RSA**: RS256, RS384, RS512 (asymmetric keys)
///
/// # JWKS Caching
///
/// - Keys are cached in-memory with configurable TTL (default: 3600s)
/// - Automatic refresh when cache expires
/// - Retry logic (3 attempts) for JWKS fetch
/// - Thread-safe using `Mutex`
///
/// # Claims Validation
///
/// - **`exp`** (expiration): Always validated with configurable leeway
/// - **`iss`** (issuer): Optional, validated if configured via `issuer()`
/// - **`aud`** (audience): Optional, validated if configured via `audience()`
/// - **`scope`**: Required for scope-protected operations
///
/// # Usage
///
/// ```rust
/// use brrtrouter::security::JwksBearerProvider;
///
/// let provider = JwksBearerProvider::new("https://auth.example.com/.well-known/jwks.json")
///     .issuer("https://auth.example.com")
///     .audience("my-api")
///     .leeway(60); // 60 seconds clock skew tolerance
/// ```
///
/// # Security
///
/// - ✅ Production-ready
/// - ✅ Supports key rotation (JWKS updates automatically)
/// - ✅ Proper cryptographic validation
/// - ✅ Issuer and audience validation
/// - ✅ Expiration checking with leeway
impl SecurityProvider for JwksBearerProvider {
    /// Validate a JWT token using JWKS
    ///
    /// Performs full cryptographic validation including signature, issuer, audience,
    /// expiration, and scopes.
    ///
    /// # Arguments
    ///
    /// * `scheme` - Security scheme from OpenAPI spec (must be HTTP Bearer)
    /// * `scopes` - Required OAuth2 scopes from operation
    /// * `req` - The security request containing headers/cookies
    ///
    /// # Returns
    ///
    /// - `true` - Token is valid and contains required scopes
    /// - `false` - Token missing, invalid signature, expired, or missing scopes
    ///
    /// # Validation Steps
    ///
    /// 1. Extract token
    /// 2. Parse header for `kid` and `alg`
    /// 3. Fetch decoding key from JWKS (cached)
    /// 4. Validate signature with `jsonwebtoken`
    /// 5. Check `iss`, `aud`, `exp` claims
    /// 6. Verify scopes
    fn validate(&self, scheme: &SecurityScheme, scopes: &[String], req: &SecurityRequest) -> bool {
        validation::validate_token_impl(self, scheme, scopes, req)
    }

    /// Extract JWT claims from a validated request.
    ///
    /// This method retrieves the decoded JWT claims from the cache if available,
    /// or decodes the token if not cached. The claims are returned as a JSON Value
    /// containing all claims from the JWT payload (e.g., `sub`, `email`, `scope`, etc.).
    ///
    /// # Arguments
    ///
    /// * `scheme` - The OpenAPI security scheme definition
    /// * `req` - The security request context with credentials
    ///
    /// # Returns
    ///
    /// * `Some(Value)` - The decoded JWT claims if token is valid and present
    /// * `None` - Token is missing, invalid, or cannot be decoded
    fn extract_claims(
        &self,
        scheme: &SecurityScheme,
        req: &SecurityRequest,
    ) -> Option<serde_json::Value> {
        validation::extract_claims_impl(self, scheme, req)
    }
}
