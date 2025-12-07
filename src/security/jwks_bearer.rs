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
    // P2: Debounce JWKS refresh to prevent concurrent HTTP requests
    refresh_in_progress: AtomicBool,
    // JSF P2: Cache decoded JWT claims to avoid repeated decode operations
    // Uses LRU cache with Arc<str> keys to prevent memory leaks and avoid allocations
    // P1: RwLock for explicit read/write separation (LruCache::get() requires &mut for LRU updates)
    // SECURITY: Cache key includes kid (key ID) so cache invalidates on key rotation
    // Format: "token|kid" -> (exp_timestamp_with_leeway, decoded_claims, kid)
    claims_cache: std::sync::RwLock<LruCache<Arc<str>, (i64, serde_json::Value, String)>>,
    claims_cache_size: usize,
    cookie_name: Option<String>,
    // P2: Cache metrics for observability and tuning
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    cache_evictions: AtomicU64,
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
            panic!("JWKS URL must use HTTPS or HTTP (for localhost only). Got: {}", url_str);
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
                NonZeroUsize::new(1000).expect("claims_cache_size must be > 0")
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
            let mut guard = self.claims_cache.write()
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

    fn extract_token<'a>(&self, req: &'a SecurityRequest) -> Option<&'a str> {
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
        if self.refresh_in_progress.compare_exchange(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_err() {
            // Another thread is refreshing, wait for it to complete
            // HTTP requests can take up to 1.5s (500ms timeout × 3 retries),
            // so we poll with exponential backoff until refresh completes
            let start = Instant::now();
            let timeout = Duration::from_secs(2); // Allow 2s for refresh (1.5s max + buffer)
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
        // Fetch outside lock with brief retries to reduce flakiness in tests
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()
        {
            Ok(c) => c,
            Err(_) => {
                // P2: Clear refresh flag on client build failure
                self.refresh_in_progress.store(false, Ordering::Release);
                return;
            },
        };
        let mut body_opt: Option<String> = None;
        for _ in 0..3 {
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
        let mut guard = self
            .cache
            .lock()
            .expect("JWKS cache Mutex poisoned - critical error");
        *guard = (Instant::now(), new_map);
        
        // P2: Clear refresh flag on success
        self.refresh_in_progress.store(false, Ordering::Release);
    }

    fn get_key_for(&self, kid: &str) -> Option<jsonwebtoken::DecodingKey> {
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
        match scheme {
            SecurityScheme::Http { scheme, .. } if scheme.eq_ignore_ascii_case("bearer") => {}
            _ => return false,
        }
        let token = match self.extract_token(req) {
            Some(t) => t,
            None => {
                debug!("JWT validation failed: missing token (no Authorization header or cookie)");
                return false;
            }
        };
        
        // SECURITY: Parse header FIRST to get kid before cache lookup
        // This ensures cache key includes kid, so cache invalidates on key rotation
        let header = match jsonwebtoken::decode_header(token) {
            Ok(h) => h,
            Err(e) => {
                warn!("JWT validation failed: invalid token header - {:?}", e);
                return false;
            }
        };
        
        let kid = match header.kid {
            Some(k) => k,
            None => {
                warn!("JWT validation failed: missing 'kid' (key ID) in token header");
                return false;
            }
        };
        
        // SECURITY: Include kid in cache key so cache invalidates on key rotation
        // Format: "token|kid" ensures different cache entries for same token with different keys
        let token_key: Arc<str> = Arc::from(format!("{}|{}", token, kid));
        
        // Check claims cache AFTER parsing header (we need kid for cache key)
        // SECURITY: On cache hit, verify key still exists in JWKS before using cached claims
        // This ensures tokens are invalidated when keys are rotated/revoked
        {
            // CRITICAL: Clone all needed values and release lock before calling get_key_for
            // get_key_for() can trigger HTTP requests (up to 1.5+ seconds) via refresh_jwks_if_needed(),
            // which would block all other threads from accessing the claims cache
            let cached_data = {
                if let Ok(mut cache_guard) = self.claims_cache.write() {
                    if let Some((exp_timestamp_with_leeway, cached_claims, cached_kid)) = cache_guard.get(&token_key) {
                        // Clone all values while holding the lock
                        Some((
                            *exp_timestamp_with_leeway,
                            cached_claims.clone(),
                            cached_kid.clone(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            
            if let Some((exp_timestamp_clone, cached_claims_clone, cached_kid_clone)) = cached_data {
                // Lock is now released - safe to call get_key_for which may trigger HTTP requests
                // SECURITY: Verify the key still exists in JWKS (key rotation check)
                // If key was rotated, this will return None and we'll re-validate
                if self.get_key_for(&cached_kid_clone).is_none() {
                    // Key no longer exists (rotated/revoked), remove from cache
                    debug!("JWT cache: key '{}' no longer in JWKS, invalidating cache entry", cached_kid_clone);
                    // Re-acquire lock to remove cache entry
                    if let Ok(mut cache_guard) = self.claims_cache.write() {
                        cache_guard.pop(&token_key);
                    }
                    // Fall through to full validation below
                } else {
                    // Key still exists, check expiration
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    
                    if now < exp_timestamp_clone {
                        // P2: Track cache hit
                        self.cache_hits.fetch_add(1, Ordering::Relaxed);
                        
                        // SECURITY: Key verified, expiration checked, use cached claims
                        // Note: We skip signature/issuer/audience re-validation here for performance,
                        // but the key existence check ensures rotation is detected
                        let token_scopes = cached_claims_clone.get("scope").and_then(|v| v.as_str()).unwrap_or("");
                        let has_all_scopes = scopes
                            .iter()
                            .all(|s| token_scopes.split_whitespace().any(|ts| ts == s));
                        
                        if has_all_scopes {
                            debug!("JWT validation succeeded: cache hit, key verified, scopes valid");
                        } else {
                            warn!(
                                "JWT validation failed: missing required scopes (token: {:?}, required: {:?})",
                                token_scopes,
                                scopes
                            );
                        }
                        return has_all_scopes;
                    } else {
                        // Token expired, remove from cache
                        debug!("JWT cache: token expired, removing from cache");
                        // Re-acquire lock to remove expired entry
                        if let Ok(mut cache_guard) = self.claims_cache.write() {
                            cache_guard.pop(&token_key);
                        }
                    }
                }
            }
        }
        
        // Cache miss or key rotation detected - need to decode token
        // P2: Track cache miss
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
        
        // Calculate SystemTime only when needed (cache miss)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        
        // Get key for validation (will trigger JWKS refresh if needed)
        let key = match self.get_key_for(&kid) {
            Some(k) => k,
            None => {
                warn!("JWT validation failed: key not found for kid '{}' in JWKS", kid);
                return false;
            }
        };
        
        // P4 Security: Only allow supported algorithms (implicitly rejects unsupported ones)
        // Note: jsonwebtoken crate doesn't have Algorithm::None variant, but we explicitly
        // match only supported algorithms for defense in depth
        let selected_alg = match header.alg {
            jsonwebtoken::Algorithm::HS256 => jsonwebtoken::Algorithm::HS256,
            jsonwebtoken::Algorithm::HS384 => jsonwebtoken::Algorithm::HS384,
            jsonwebtoken::Algorithm::HS512 => jsonwebtoken::Algorithm::HS512,
            jsonwebtoken::Algorithm::RS256 => jsonwebtoken::Algorithm::RS256,
            jsonwebtoken::Algorithm::RS384 => jsonwebtoken::Algorithm::RS384,
            jsonwebtoken::Algorithm::RS512 => jsonwebtoken::Algorithm::RS512,
            // P4: Explicitly reject all other algorithms (security hardening)
            unsupported => {
                warn!("JWT validation failed: unsupported algorithm '{:?}'", unsupported);
                return false;
            }
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
        let data: Result<jsonwebtoken::TokenData<serde_json::Value>, jsonwebtoken::errors::Error> =
            jsonwebtoken::decode(token, &key, &validation);
        let claims = match data {
            Ok(d) => d.claims,
            Err(e) => {
                // Log specific error types for better debugging
                let error_msg = match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => "token expired",
                    jsonwebtoken::errors::ErrorKind::InvalidSignature => "invalid signature",
                    jsonwebtoken::errors::ErrorKind::InvalidIssuer => "invalid issuer",
                    jsonwebtoken::errors::ErrorKind::InvalidAudience => "invalid audience",
                    jsonwebtoken::errors::ErrorKind::InvalidSubject => "invalid subject",
                    jsonwebtoken::errors::ErrorKind::MissingRequiredClaim(claim) => {
                        return {
                            warn!("JWT validation failed: missing required claim '{}'", claim);
                            false
                        };
                    }
                    _ => "decode error",
                };
                warn!("JWT validation failed: {} - {:?}", error_msg, e);
                return false;
            }
        };
        
        // P0: Store decoded claims in cache with leeway applied to expiration
        // Extract exp claim to determine cache validity
        if let Some(exp_value) = claims.get("exp") {
            if let Some(exp_timestamp) = exp_value.as_i64() {
                // P0: Store expiration WITH leeway to match validation logic
                let exp_timestamp_with_leeway = exp_timestamp + self.leeway_secs as i64;
                
                // Only cache if token hasn't expired (with leeway)
                // SECURITY: Store kid with cached claims so we can verify key existence on cache hits
                if now < exp_timestamp_with_leeway {
                    if let Ok(mut cache_guard) = self.claims_cache.write() {
                        // P0: Use Arc<str> key (already created above with kid included)
                        // P1: Write lock for cache insert (eviction may occur)
                        // P2: Track evictions correctly - LruCache::put() returns Some(old_value) when
                        //     updating an existing key, NOT when evicting. To detect evictions, we must
                        //     check if the key doesn't exist AND the cache is at capacity before inserting.
                        let key_exists = cache_guard.peek(&token_key).is_some();
                        let cache_at_capacity = cache_guard.len() >= cache_guard.cap().get();
                        let will_evict = !key_exists && cache_at_capacity;
                        
                        // Insert/update the cache entry
                        cache_guard.put(token_key.clone(), (exp_timestamp_with_leeway, claims.clone(), kid.clone()));
                        
                        // Track eviction only if we inserted a new key when cache was at capacity
                        if will_evict {
                            self.cache_evictions.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        }
        
        // scope check
        let token_scopes = claims.get("scope").and_then(|v| v.as_str()).unwrap_or("");
        let has_all_scopes = scopes
            .iter()
            .all(|s| token_scopes.split_whitespace().any(|ts| ts == s));
        
        if has_all_scopes {
            debug!("JWT validation succeeded: token valid, scopes present");
        } else {
            warn!(
                "JWT validation failed: missing required scopes (token: {:?}, required: {:?})",
                token_scopes,
                scopes
            );
        }
        
        has_all_scopes
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
    fn extract_claims(&self, scheme: &SecurityScheme, req: &SecurityRequest) -> Option<serde_json::Value> {
        match scheme {
            SecurityScheme::Http { scheme, .. } if scheme.eq_ignore_ascii_case("bearer") => {}
            _ => return None,
        }
        
        let token = match self.extract_token(req) {
            Some(t) => t,
            None => return None,
        };
        
        // Parse header to get kid for cache key
        let header = match jsonwebtoken::decode_header(token) {
            Ok(h) => h,
            Err(_) => return None,
        };
        
        let kid = match header.kid {
            Some(k) => k,
            None => return None,
        };
        
        // Check cache first
        // SECURITY: On cache hit, verify key still exists in JWKS before using cached claims
        // This ensures tokens are invalidated when keys are rotated/revoked
        let token_key: Arc<str> = Arc::from(format!("{}|{}", token, kid));
        {
            // CRITICAL: Clone all needed values and release lock before calling get_key_for
            // get_key_for() can trigger HTTP requests (up to 1.5+ seconds) via refresh_jwks_if_needed(),
            // which would block all other threads from accessing the claims cache
            let cached_data = {
                if let Ok(mut cache_guard) = self.claims_cache.write() {
                    if let Some((exp_timestamp_with_leeway, cached_claims, cached_kid)) = cache_guard.get(&token_key) {
                        // Clone all values while holding the lock
                        Some((
                            *exp_timestamp_with_leeway,
                            cached_claims.clone(),
                            cached_kid.clone(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            
            if let Some((exp_timestamp_clone, cached_claims_clone, cached_kid_clone)) = cached_data {
                // Lock is now released - safe to call get_key_for which may trigger HTTP requests
                // SECURITY: Verify the key still exists in JWKS (key rotation check)
                // If key was rotated, this will return None and we'll re-validate
                if self.get_key_for(&cached_kid_clone).is_none() {
                    // Key no longer exists (rotated/revoked), remove from cache
                    debug!("JWT cache: key '{}' no longer in JWKS, invalidating cache entry", cached_kid_clone);
                    // Re-acquire lock to remove cache entry
                    if let Ok(mut cache_guard) = self.claims_cache.write() {
                        cache_guard.pop(&token_key);
                    }
                    // Fall through to full decode below
                } else {
                    // Key still exists, check expiration
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    
                    if now < exp_timestamp_clone {
                        // SECURITY: Key verified, expiration checked, use cached claims
                        // Note: We skip signature/issuer/audience re-validation here for performance,
                        // but the key existence check ensures rotation is detected
                        return Some(cached_claims_clone);
                    } else {
                        // Token expired, remove from cache
                        debug!("JWT cache: token expired, removing from cache");
                        // Re-acquire lock to remove expired entry
                        if let Ok(mut cache_guard) = self.claims_cache.write() {
                            cache_guard.pop(&token_key);
                        }
                    }
                }
            }
        }
        
        // Cache miss - decode token (same logic as validate, but we return claims)
        let key = match self.get_key_for(&kid) {
            Some(k) => k,
            None => return None,
        };
        
        let selected_alg = match header.alg {
            jsonwebtoken::Algorithm::HS256 => jsonwebtoken::Algorithm::HS256,
            jsonwebtoken::Algorithm::HS384 => jsonwebtoken::Algorithm::HS384,
            jsonwebtoken::Algorithm::HS512 => jsonwebtoken::Algorithm::HS512,
            jsonwebtoken::Algorithm::RS256 => jsonwebtoken::Algorithm::RS256,
            jsonwebtoken::Algorithm::RS384 => jsonwebtoken::Algorithm::RS384,
            jsonwebtoken::Algorithm::RS512 => jsonwebtoken::Algorithm::RS512,
            _ => return None,
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
        
        match jsonwebtoken::decode(token, &key, &validation) {
            Ok(data) => Some(data.claims),
            Err(_) => None,
        }
    }
}
