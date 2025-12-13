mod validation;

use crate::security::{CacheStats, SecurityProvider, SecurityRequest};
use crate::spec::SecurityScheme;
use base64::Engine as _;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};
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
    // P1: Shared cache_ttl for background thread to read current value
    // Stored as milliseconds (u64) in AtomicU64 for lock-free reads
    // Using milliseconds preserves sub-second precision (e.g., 100ms TTL)
    cache_ttl_millis: Arc<std::sync::atomic::AtomicU64>,
    // P1: Background refresh - use Arc<RwLock> for lock-free reads
    // kid -> DecodingKey
    cache: Arc<RwLock<(Instant, HashMap<String, jsonwebtoken::DecodingKey>)>>,
    // P2: Debounce JWKS refresh to prevent concurrent HTTP requests
    refresh_in_progress: Arc<AtomicBool>,
    // P1: Condition variable to notify waiting threads when refresh completes
    // Waiting threads use wait_timeout to be woken immediately when refresh finishes
    refresh_complete: Arc<(Mutex<()>, Condvar)>,
    // P1: Background refresh task handle for lifecycle management
    background_handle: Option<Arc<RwLock<Option<JoinHandle<()>>>>>,
    // P1: Shutdown flag for graceful background thread termination
    shutdown: Arc<AtomicBool>,
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
    ///
    /// JSF Compliance: Panics only during initialization, never on hot path
    /// This method is only called during provider construction (startup)
    #[allow(clippy::panic)]
    pub fn new(jwks_url: impl Into<String>) -> Self {
        let url_str = jwks_url.into();

        // P4 Security: Validate JWKS URL requires HTTPS (except localhost for testing)
        // SECURITY FIX: Parse URL properly to prevent hostname prefix attacks (e.g., localhost.attacker.com)
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
            // HTTP only allowed for exact localhost or 127.0.0.1 (not subdomains)
            // This panic is intentional: invalid configuration should fail fast at startup
            let host = match parsed_url.host_str() {
                Some(h) => h,
                None => {
                    panic!("JWKS URL must have a valid hostname. Got: {}", url_str);
                }
            };

            // Only allow exact "localhost" or "127.0.0.1" - reject subdomains like "localhost.attacker.com"
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

        let cache = Arc::new(RwLock::new((
            Instant::now() - Duration::from_secs(1000),
            HashMap::new(),
        )));
        let background_handle = Arc::new(RwLock::new(None::<JoinHandle<()>>));
        let refresh_in_progress = Arc::new(AtomicBool::new(false));
        let refresh_complete = Arc::new((Mutex::new(()), Condvar::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let cache_ttl_millis = Arc::new(std::sync::atomic::AtomicU64::new(300_000));
        
        let provider = Self {
            jwks_url: url_str,
            iss: None,
            aud: None,
            leeway_secs: 30,
            cache_ttl: Duration::from_secs(300),
            cache_ttl_millis: cache_ttl_millis.clone(),
            cache: cache.clone(),
            refresh_in_progress: refresh_in_progress.clone(),
            refresh_complete: refresh_complete.clone(),
            claims_cache: std::sync::RwLock::new(LruCache::new(
                NonZeroUsize::new(1000).expect("claims_cache_size must be > 0"),
            )),
            claims_cache_size: 1000,
            cookie_name: None,
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            cache_evictions: AtomicU64::new(0),
            background_handle: Some(background_handle.clone()),
            shutdown: shutdown.clone(),
        };
        
        // Start background refresh task
        provider.start_background_refresh_internal(
            cache,
            refresh_in_progress,
            refresh_complete,
            shutdown,
            background_handle,
            cache_ttl_millis,
        );
        
        provider
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
    /// This updates both the field and the background refresh thread's interval.
    /// 
    /// # Precision
    /// 
    /// Sub-second precision is preserved (e.g., `Duration::from_millis(100)` works correctly).
    pub fn cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        // Update atomic value so background thread picks up the new TTL
        // Store as milliseconds to preserve sub-second precision
        self.cache_ttl_millis.store(ttl.as_millis() as u64, Ordering::Release);
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
    ///
    /// JSF Compliance: Panics only during initialization, never on hot path
    /// This method is only called during provider construction (startup)
    #[allow(clippy::panic)]
    pub fn claims_cache_size(mut self, size: usize) -> Self {
        // This panic is intentional: invalid configuration should fail fast at startup
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

    /// Start background refresh task that proactively refreshes JWKS
    ///
    /// The background task refreshes JWKS every (cache_ttl - 10s) to stay ahead of expiration.
    /// This ensures validation threads never block on HTTP requests.
    fn start_background_refresh_internal(
        &self,
        cache: Arc<RwLock<(Instant, HashMap<String, jsonwebtoken::DecodingKey>)>>,
        refresh_in_progress: Arc<AtomicBool>,
        refresh_complete: Arc<(Mutex<()>, Condvar)>,
        shutdown: Arc<AtomicBool>,
        handle_lock: Arc<RwLock<Option<JoinHandle<()>>>>,
        cache_ttl_millis: Arc<std::sync::atomic::AtomicU64>,
    ) {
        let jwks_url = self.jwks_url.clone();
        let cache_ttl_millis = cache_ttl_millis;
        
        let handle = thread::spawn(move || {
            // Do immediate refresh on startup to populate cache
            // This ensures the cache is ready before the first validation request
            // Only do this if shutdown hasn't been signaled (allows tests to stop refresh before it starts)
            if !shutdown.load(Ordering::Acquire) {
                Self::refresh_jwks_internal(
                    &cache,
                    &jwks_url,
                    &refresh_in_progress,
                    &refresh_complete,
                    false, // Background thread claims the refresh itself
                );
            }
            
            loop {
                // Check shutdown flag
                if shutdown.load(Ordering::Acquire) {
                    debug!("JWKS background refresh thread shutting down");
                    break;
                }
                
                // Read current cache_ttl from atomic ONCE to ensure consistency
                // This value is used for both refresh_interval calculation and change detection
                // Reading twice could cause inconsistency if TTL changes between reads
                let initial_cache_ttl_millis = cache_ttl_millis.load(Ordering::Acquire);
                // Convert from milliseconds to Duration, preserving sub-second precision
                let cache_ttl = Duration::from_millis(initial_cache_ttl_millis);
                // Refresh interval: cache_ttl - 10s to stay ahead of expiration
                // For cache_ttl <= 10s, use cache_ttl / 2 to avoid zero interval and CPU spinning
                let refresh_interval = if cache_ttl <= Duration::from_secs(10) {
                    // For very short TTLs, refresh at half the TTL interval
                    cache_ttl / 2
                } else {
                    cache_ttl.saturating_sub(Duration::from_secs(10))
                };
                // Ensure minimum refresh interval of 1 second to prevent CPU spinning
                let refresh_interval = refresh_interval.max(Duration::from_secs(1));
                
                // Sleep until next refresh time (with periodic shutdown and cache_ttl change checks)
                // If cache_ttl is updated via cache_ttl() builder, we need to wake up early
                // and recalculate refresh_interval with the new value
                let sleep_duration = Duration::from_secs(1).min(refresh_interval);
                let mut slept = Duration::ZERO;
                let mut ttl_changed = false;
                while slept < refresh_interval {
                    if shutdown.load(Ordering::Acquire) {
                        debug!("JWKS background refresh thread shutting down");
                        return;
                    }
                    // Check if cache_ttl has changed (cache_ttl() builder was called)
                    // If it has, break out of sleep loop early to recalculate refresh_interval
                    let current_cache_ttl_millis = cache_ttl_millis.load(Ordering::Acquire);
                    if current_cache_ttl_millis != initial_cache_ttl_millis {
                        debug!(
                            "JWKS cache_ttl changed from {}ms to {}ms, recalculating refresh interval",
                            initial_cache_ttl_millis,
                            current_cache_ttl_millis
                        );
                        ttl_changed = true;
                        break; // Break out of sleep loop to recalculate refresh_interval
                    }
                    thread::sleep(sleep_duration);
                    slept += sleep_duration;
                }
                
                // Refresh if we completed the full sleep interval OR if TTL changed
                // When TTL changes, refresh immediately to pick up the new refresh schedule
                if ttl_changed {
                    // TTL changed - refresh immediately and recalculate interval on next iteration
                    Self::refresh_jwks_internal(
                        &cache,
                        &jwks_url,
                        &refresh_in_progress,
                        &refresh_complete,
                        false, // Background thread claims the refresh itself
                    );
                } else {
                    // After sleeping for refresh_interval, always refresh proactively
                    // The refresh_interval is calculated to wake up before expiration,
                    // so we should refresh now to keep the cache fresh.
                    // The debounce mechanism in refresh_jwks_internal prevents concurrent refreshes.
                    Self::refresh_jwks_internal(
                        &cache,
                        &jwks_url,
                        &refresh_in_progress,
                        &refresh_complete,
                        false, // Background thread claims the refresh itself
                    );
                }
                // Continue to next loop iteration to recalculate refresh_interval
            }
        });
        
        if let Ok(mut guard) = handle_lock.write() {
            *guard = Some(handle);
        }
    }
    
    /// Stop the background refresh task
    ///
    /// This should be called during cleanup/shutdown to gracefully stop the background thread.
    pub fn stop_background_refresh(&self) {
        // Signal shutdown
        self.shutdown.store(true, Ordering::Release);
        
        // Wait for thread to finish
        if let Some(handle_lock) = &self.background_handle {
            if let Ok(mut guard) = handle_lock.write() {
                if let Some(handle) = guard.take() {
                    // Wait for thread to finish (will exit when shutdown flag is set)
                    let _ = handle.join();
                }
            }
        }
    }
    
    /// Internal method to refresh JWKS (can be called from background thread or on-demand)
    /// 
    /// # Arguments
    /// * `already_claimed` - If true, the caller has already atomically claimed the refresh
    ///   (set refresh_in_progress to true). If false, this method will atomically claim it.
    fn refresh_jwks_internal(
        cache: &Arc<RwLock<(Instant, HashMap<String, jsonwebtoken::DecodingKey>)>>,
        jwks_url: &str,
        refresh_in_progress: &Arc<AtomicBool>,
        refresh_complete: &Arc<(Mutex<()>, Condvar)>,
        already_claimed: bool,
    ) {
        // P2: Debounce - check if another thread is already refreshing
        // If already_claimed is true, we've already set the flag, so skip the check
        if !already_claimed
            && refresh_in_progress
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                // Another thread is refreshing, skip this cycle
                return;
            }
        
        let refresh_start = Instant::now();
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(200))
            .build()
        {
            Ok(c) => c,
            Err(_) => {
                refresh_in_progress.store(false, Ordering::Release);
                // Notify waiting threads even on failure so they don't wait forever
                let (lock, cvar) = &**refresh_complete;
                let _guard = lock.lock().unwrap();
                cvar.notify_all();
                return;
            }
        };
        
        let mut body_opt: Option<String> = None;
        for _ in 0..2 {
            if let Ok(r) = client.get(jwks_url).send() {
                if let Ok(t) = r.text() {
                    body_opt = Some(t);
                    break;
                }
            }
        }
        
        let body = match body_opt {
            Some(b) => b,
            None => {
                refresh_in_progress.store(false, Ordering::Release);
                // Notify waiting threads even on failure so they don't wait forever
                let (lock, cvar) = &**refresh_complete;
                let _guard = lock.lock().unwrap();
                cvar.notify_all();
                return;
            }
        };
        
        let parsed: serde_json::Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) => {
                refresh_in_progress.store(false, Ordering::Release);
                // Notify waiting threads even on failure so they don't wait forever
                let (lock, cvar) = &**refresh_complete;
                let _guard = lock.lock().unwrap();
                cvar.notify_all();
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
                    if let Ok(dk) = jsonwebtoken::DecodingKey::from_rsa_components(n, e) {
                        new_map.insert(kid.to_string(), dk);
                    }
                    continue;
                }
            }
        }
        
        let key_count = new_map.len();
        let refresh_duration = refresh_start.elapsed();
        
        if let Ok(mut guard) = cache.write() {
            *guard = (Instant::now(), new_map);
        }
        
        refresh_in_progress.store(false, Ordering::Release);
        
        // Notify all waiting threads that refresh has completed
        // This wakes them immediately instead of waiting for their next poll
        let (lock, cvar) = &**refresh_complete;
        let _guard = lock.lock().unwrap();
        cvar.notify_all();
        
        debug!(
            "JWKS refresh completed in {:?} (keys: {})",
            refresh_duration, key_count
        );
    }
    
    /// P1: Non-blocking refresh check - triggers refresh if needed but doesn't wait
    /// Uses stale cache if refresh fails (graceful degradation)
    /// 
    /// If cache is empty, does a blocking initial refresh to ensure first validation succeeds.
    /// When cache is empty and another thread is refreshing, waits with exponential backoff
    /// to ensure concurrent threads don't fail validation due to empty cache.
    fn refresh_jwks_if_needed(&self) {
        // Read current cache_ttl from atomic (picks up changes from cache_ttl() builder)
        // Convert from milliseconds to Duration, preserving sub-second precision
        let current_cache_ttl = Duration::from_millis(self.cache_ttl_millis.load(Ordering::Acquire));
        // Check if refresh is needed (non-blocking read)
        let (needs_refresh, is_empty) = {
            if let Ok(guard) = self.cache.read() {
                (guard.0.elapsed() >= current_cache_ttl || guard.1.is_empty(), guard.1.is_empty())
            } else {
                // Lock poisoned, skip refresh
                return;
            }
        };
        
        if !needs_refresh {
            return;
        }
        
        // If cache is empty, do a blocking initial refresh to ensure first validation succeeds
        // After that, background refresh will keep it updated
        if is_empty {
            // Record cache timestamp BEFORE refresh to detect if refresh succeeded
            let cache_timestamp_before = {
                if let Ok(guard) = self.cache.read() {
                    guard.0
                } else {
                    // Lock poisoned, give up
                    return;
                }
            };
            
            // Try to claim the refresh - if we win, do blocking refresh
            // If we lose, another thread is refreshing - wait for it with exponential backoff
            if self
                .refresh_in_progress
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                // We won the race - do blocking initial refresh
                Self::refresh_jwks_internal(
                    &self.cache,
                    &self.jwks_url,
                    &self.refresh_in_progress,
                    &self.refresh_complete,
                    true, // We already claimed the flag
                );
                
                // Check if refresh succeeded by comparing timestamps
                // If timestamp unchanged, refresh failed - retry
                let cache_timestamp_after = {
                    if let Ok(guard) = self.cache.read() {
                        guard.0
                    } else {
                        // Lock poisoned, give up
                        return;
                    }
                };
                
                // Only retry if timestamp unchanged (refresh failed) AND cache still empty
                if cache_timestamp_after == cache_timestamp_before {
                    let still_empty = {
                        if let Ok(guard) = self.cache.read() {
                            guard.1.is_empty()
                        } else {
                            // Lock poisoned, give up
                            return;
                        }
                    };
                    
                    if still_empty {
                        // Refresh failed - retry once
                        if self
                            .refresh_in_progress
                            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                            .is_ok()
                        {
                            Self::refresh_jwks_internal(
                                &self.cache,
                                &self.jwks_url,
                                &self.refresh_in_progress,
                                &self.refresh_complete,
                                true, // We already claimed the flag
                            );
                        }
                    }
                }
            } else {
                // Another thread is refreshing - wait for it to complete using condition variable
                // This is more efficient than polling - threads are woken immediately when refresh completes
                
                // Record cache timestamp BEFORE waiting to detect if refresh succeeded
                let cache_timestamp_before = {
                    if let Ok(guard) = self.cache.read() {
                        guard.0
                    } else {
                        // Lock poisoned, give up
                        return;
                    }
                };
                
                let timeout = Duration::from_secs(2); // Allow 2s for refresh (400ms max + buffer for network issues)
                let (lock, cvar) = &*self.refresh_complete;
                let guard = lock.lock().unwrap();
                
                // Wait for refresh to complete, with timeout
                // The condvar will wake us immediately when refresh_in_progress becomes false
                let wait_result = cvar.wait_timeout_while(
                    guard,
                    timeout,
                    |_| {
                        // Continue waiting while refresh is in progress
                        self.refresh_in_progress.load(Ordering::Acquire)
                    },
                );
                
                let (wait_guard, wait_timeout_result) = match wait_result {
                    Ok(result) => result,
                    Err(_) => {
                        // Lock poisoned, give up
                        return;
                    }
                };
                
                drop(wait_guard);
                
                // Check if we timed out or if refresh completed
                if wait_timeout_result.timed_out() && self.refresh_in_progress.load(Ordering::Acquire) {
                    // Timeout - refresh may have failed, check if cache timestamp was updated
                    // If timestamp unchanged, refresh failed and we should retry
                    let cache_timestamp_after = {
                        if let Ok(guard) = self.cache.read() {
                            guard.0
                        } else {
                            // Lock poisoned, give up
                            return;
                        }
                    };
                    
                    // If timestamp didn't change, refresh failed (cache not updated)
                    // Only retry if cache is still empty AND timestamp unchanged
                    if cache_timestamp_after == cache_timestamp_before {
                        let still_empty = {
                            if let Ok(guard) = self.cache.read() {
                                guard.1.is_empty()
                            } else {
                                // Lock poisoned, give up
                                return;
                            }
                        };
                        
                        if still_empty {
                            // Cache timestamp unchanged and still empty - refresh failed, retry
                            if self
                                .refresh_in_progress
                                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                                .is_ok()
                            {
                                // We can now claim the refresh - do blocking refresh
                                Self::refresh_jwks_internal(
                                    &self.cache,
                                    &self.jwks_url,
                                    &self.refresh_in_progress,
                                    &self.refresh_complete,
                                    true, // We already claimed the flag
                                );
                            }
                        }
                    }
                    // If timestamp changed, refresh succeeded (even if empty keys) - return
                    return;
                }
                
                // Refresh completed (flag cleared) - check if refresh succeeded by comparing timestamps
                // If timestamp changed, refresh succeeded (even if empty keys) - don't retry
                // If timestamp unchanged, refresh failed - retry
                let cache_timestamp_after = {
                    if let Ok(guard) = self.cache.read() {
                        guard.0
                    } else {
                        // Lock poisoned, give up
                        return;
                    }
                };
                
                // Only retry if timestamp unchanged (refresh failed) AND cache still empty
                if cache_timestamp_after == cache_timestamp_before {
                    let still_empty = {
                        if let Ok(guard) = self.cache.read() {
                            guard.1.is_empty()
                        } else {
                            // Lock poisoned, give up
                            return;
                        }
                    };
                    
                    if still_empty {
                        // Cache timestamp unchanged and still empty - refresh failed, retry
                        if self
                            .refresh_in_progress
                            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                            .is_ok()
                        {
                            // We can now claim the refresh - do blocking refresh
                            Self::refresh_jwks_internal(
                                &self.cache,
                                &self.jwks_url,
                                &self.refresh_in_progress,
                                &self.refresh_complete,
                                true, // We already claimed the flag
                            );
                        }
                    }
                }
                // If timestamp changed, refresh succeeded (even if empty keys) - return
                // Return to allow caller to read the cache (populated or empty)
            }
        } else {
            // Cache exists but expired - trigger refresh in background (non-blocking)
            // P1: Atomically claim the right to spawn a refresh thread to avoid thread storm
            // Under load during expiry window, multiple requests would all see refresh_in_progress=false,
            // all pass the check, and all spawn threads that immediately return when refresh_jwks_internal
            // sees the flag is already set. This causes unbounded thread creation and CPU/memory pressure.
            //
            // Solution: Use compare_exchange to atomically check and set the flag BEFORE spawning.
            // Only one thread will successfully claim the refresh, preventing thread storms.
            if self
                .refresh_in_progress
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                // Another thread already claimed the refresh (or background thread is refreshing)
                // Skip spawning - the existing refresh will handle it
                // For expired cache, we can use stale data (graceful degradation)
                return;
            }
            
            // We successfully claimed the refresh - spawn thread to do the actual work
            // Note: refresh_jwks_internal will clear the flag when done, so we don't need to
            // handle that here. If refresh_jwks_internal fails, it also clears the flag.
            let cache = self.cache.clone();
            let jwks_url = self.jwks_url.clone();
            // Clone Arc references - one set for the spawned thread, one for error handling
            let refresh_in_progress_thread = self.refresh_in_progress.clone();
            let refresh_complete_thread = self.refresh_complete.clone();
            let refresh_in_progress_error = self.refresh_in_progress.clone();
            let refresh_complete_error = self.refresh_complete.clone();
            
            // CRITICAL: If thread::spawn panics (e.g., resource exhaustion), we must clear
            // the refresh_in_progress flag to prevent permanent deadlock. The spawned thread
            // is responsible for clearing it, but if spawning fails, that never happens.
            match thread::Builder::new()
                .spawn(move || {
                    // We've already atomically claimed the refresh (set flag to true above),
                    // so pass already_claimed=true to skip the flag check in refresh_jwks_internal
                    Self::refresh_jwks_internal(&cache, &jwks_url, &refresh_in_progress_thread, &refresh_complete_thread, true);
                }) {
                Ok(_) => {
                    // Thread spawned successfully - it will clear the flag when done
                }
                Err(e) => {
                    // Thread spawn failed (resource exhaustion, etc.) - clear flag to prevent deadlock
                    warn!("Failed to spawn JWKS refresh thread: {}. Clearing refresh_in_progress flag to prevent deadlock.", e);
                    refresh_in_progress_error.store(false, Ordering::Release);
                    // Notify any waiting threads that refresh won't happen
                    let (lock, cvar) = &*refresh_complete_error;
                    let _guard = lock.lock().unwrap();
                    cvar.notify_all();
                }
            }
        }
    }

    /// Get decoding key for a given key ID (kid)
    ///
    /// P1: Non-blocking - uses lock-free reads (RwLock) and triggers refresh in background.
    /// If refresh fails, uses stale cache (graceful degradation).
    pub(super) fn get_key_for(&self, kid: &str) -> Option<jsonwebtoken::DecodingKey> {
        // Trigger refresh if needed (non-blocking)
        self.refresh_jwks_if_needed();
        
        // Lock-free read (RwLock allows concurrent reads)
        if let Ok(guard) = self.cache.read() {
            guard.1.get(kid).cloned()
        } else {
            // Lock poisoned, return None
            None
        }
    }
}

impl Drop for JwksBearerProvider {
    /// Clean up background thread when provider is dropped
    ///
    /// Ensures the background refresh thread is properly stopped to prevent
    /// resource leaks and orphaned threads holding references to shared state.
    fn drop(&mut self) {
        self.stop_background_refresh();
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
