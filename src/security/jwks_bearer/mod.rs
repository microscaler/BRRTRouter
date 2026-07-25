mod jwt_logger;
mod validation;

pub use jwt_logger::{DecisionSource, JwtLogFields, JwtStructuredLogger};

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
use tracing::{debug, error, info, warn, Level};
use url::Url;

use crate::security::{jwks_fetch_timeout_from_env, JWKS_FETCH_ATTEMPTS, JWKS_FETCH_TIMEOUT_ENV};

// Algorithms supported by jsonwebtoken's rust_crypto backend. Each provider should configure
// the smallest issuer-specific subset with `allowed_algorithms`; this full set remains the
// backward-compatible default for existing BRRTRouter consumers.
pub(super) const SUPPORTED_ALGORITHMS: &[jsonwebtoken::Algorithm] = &[
    jsonwebtoken::Algorithm::HS256,
    jsonwebtoken::Algorithm::HS384,
    jsonwebtoken::Algorithm::HS512,
    jsonwebtoken::Algorithm::RS256,
    jsonwebtoken::Algorithm::RS384,
    jsonwebtoken::Algorithm::RS512,
    jsonwebtoken::Algorithm::PS256,
    jsonwebtoken::Algorithm::PS384,
    jsonwebtoken::Algorithm::PS512,
    jsonwebtoken::Algorithm::ES256,
    jsonwebtoken::Algorithm::ES384,
    jsonwebtoken::Algorithm::EdDSA,
];

/// Dynamic status of a cryptographically valid JWT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JwtTokenStatus {
    /// Token is current and not revoked.
    Active,
    /// Token `jti` has been explicitly revoked.
    Revoked,
    /// Token version is older than authoritative subject or tenant state.
    Stale,
    /// The authoritative status dependency could not be queried.
    Unavailable,
    /// Required status claims are missing or malformed.
    Invalid,
}

/// Consumer-supplied dynamic token-status check.
///
/// BRRTRouter owns signature and standard-claim validation. Identity systems can attach this
/// hook for denylist and version checks that depend on their authoritative state.
pub trait JwtTokenStatusChecker: Send + Sync {
    /// Evaluate a cryptographically validated JWT's claims.
    fn check(&self, claims: &serde_json::Value) -> JwtTokenStatus;
}

impl<F> JwtTokenStatusChecker for F
where
    F: Fn(&serde_json::Value) -> JwtTokenStatus + Send + Sync,
{
    fn check(&self, claims: &serde_json::Value) -> JwtTokenStatus {
        self(claims)
    }
}

/// Fetch configuration and outcome counters shared by every code path that can refresh JWKS.
///
/// Lives behind an `Arc` because the background refresh thread, the on-demand refresh threads and
/// the request path all need it, while `JwksBearerProvider::refresh_jwks_internal` is an
/// associated function that only ever receives shared handles.
#[derive(Debug)]
pub(crate) struct JwksFetchState {
    /// Per-attempt HTTP timeout in milliseconds.
    ///
    /// Atomic (rather than a plain field) for the same reason `cache_ttl_millis` is: the
    /// background thread starts inside `new()`, so a builder call such as `fetch_timeout()` has to
    /// be visible to a thread that is already running.
    timeout_millis: AtomicU64,
    /// Whether a JWKS fetch has *ever* produced a parseable keyset.
    ///
    /// This is the difference between **degraded** (a refresh failed but a cached keyset is still
    /// being served, so most traffic still works) and **fatal** (no keyset has ever loaded, so
    /// every single token is rejected). Those are very different incidents and must not share a
    /// log line, a severity, or a readiness answer.
    ever_loaded: AtomicBool,
    /// Successful refreshes (HTTP fetch + JSON parse).
    fetch_success: AtomicU64,
    /// Failed refreshes (transport failure, non-2xx, or unparseable body).
    fetch_failure: AtomicU64,
    /// Failures since the last success. Reset to zero on success.
    consecutive_failures: AtomicU64,
}

impl JwksFetchState {
    fn new(timeout: Duration) -> Self {
        Self {
            timeout_millis: AtomicU64::new(timeout.as_millis() as u64),
            ever_loaded: AtomicBool::new(false),
            fetch_success: AtomicU64::new(0),
            fetch_failure: AtomicU64::new(0),
            consecutive_failures: AtomicU64::new(0),
        }
    }

    fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_millis.load(Ordering::Acquire))
    }

    fn set_timeout(&self, timeout: Duration) {
        self.timeout_millis
            .store(timeout.as_millis() as u64, Ordering::Release);
    }

    fn ever_loaded(&self) -> bool {
        self.ever_loaded.load(Ordering::Acquire)
    }

    /// Record a successful refresh; returns the failure streak it just ended.
    fn record_success(&self) -> u64 {
        self.ever_loaded.store(true, Ordering::Release);
        self.fetch_success.fetch_add(1, Ordering::Relaxed);
        self.consecutive_failures.swap(0, Ordering::AcqRel)
    }

    /// Record a failed refresh; returns the new consecutive-failure count.
    fn record_failure(&self) -> u64 {
        self.fetch_failure.fetch_add(1, Ordering::Relaxed);
        self.consecutive_failures.fetch_add(1, Ordering::AcqRel) + 1
    }
}

/// Observable state of a [`JwksBearerProvider`]'s key material.
///
/// Exposed so services can wire JWKS liveness into their own `/ready` probe — see
/// [`JwksBearerProvider::readiness`] and `AppService::set_readiness_check`. A router that has
/// never loaded a keyset cannot authenticate anything and should not be taking traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JwksHealth {
    /// A JWKS document has been fetched and parsed at least once since startup.
    pub ever_loaded: bool,
    /// Number of usable decoding keys currently cached.
    pub keys_cached: usize,
    /// Total successful refreshes.
    pub fetch_success: u64,
    /// Total failed refreshes.
    pub fetch_failure: u64,
    /// Failures since the last success (zero when healthy).
    pub consecutive_failures: u64,
}

impl JwksHealth {
    /// Whether the provider can validate tokens right now.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ever_loaded && self.keys_cached > 0
    }

    /// Whether the provider is serving a keyset that is no longer being refreshed successfully.
    ///
    /// Distinct from `!is_ready()`: degraded still authenticates existing keys, it just cannot
    /// pick up a key rotation.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        self.is_ready() && self.consecutive_failures > 0
    }
}

/// Whether an HTTP (non-TLS) JWKS URL host is permitted.
///
/// Loopback is allowed for local dev; `.svc.cluster.local` for in-cluster Kubernetes
/// service DNS (same trust boundary as the pod network).
pub(crate) fn jwks_http_host_allowed(host: &str) -> bool {
    host == "localhost" || host == "127.0.0.1" || host.ends_with(".svc.cluster.local")
}

/// JWKS-based Bearer provider for production integrations.
/// Fetches keys from a JWKS URL and validates JWTs (signature and claims).
pub struct JwksBearerProvider {
    jwks_url: String,
    pub(super) iss: Option<String>,
    pub(super) aud: Option<String>,
    pub(super) leeway_secs: u64,
    /// Algorithms accepted for this provider. This is configuration, not token input.
    pub(super) allowed_algorithms: Vec<jsonwebtoken::Algorithm>,
    token_status_checker: Option<Arc<dyn JwtTokenStatusChecker>>,
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
    // Unknown-kid refreshes bypass the normal JWKS TTL so newly rotated keys can be used
    // immediately. A cooldown prevents attacker-controlled kids from causing a fetch storm.
    unknown_kid_refresh_cooldown: Duration,
    last_unknown_kid_refresh: Mutex<Option<Instant>>,
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
    // JWKS fetch timeout + outcome counters, shared with every refresh path (background thread,
    // on-demand refresh threads, unknown-kid refresh).
    fetch_state: Arc<JwksFetchState>,
    // JWKS fetch metrics (HACK-101: poisoning defense)
    jwks_poisoning_rejected: AtomicU64,
    // Story 9.6: Structured JWT logging for audit trail
    pub(super) structured_logger: JwtStructuredLogger,
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
    /// JWKS URL must use HTTPS (validated in `new()`). HTTP is limited to loopback and
    /// in-cluster `.svc.cluster.local` hosts.
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

            // Only allow exact loopback or Kubernetes in-cluster service DNS.
            if !jwks_http_host_allowed(host) {
                panic!("JWKS URL must use HTTPS for security (HTTP only allowed for localhost/127.0.0.1 or *.svc.cluster.local). Got: {}", url_str);
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
        // Timeout default comes from configuration, not a literal in the refresh path — see
        // `crate::security::DEFAULT_JWKS_FETCH_TIMEOUT` for why the old 200ms was wrong.
        let fetch_state = Arc::new(JwksFetchState::new(jwks_fetch_timeout_from_env()));

        let provider = Self {
            jwks_url: url_str,
            iss: None,
            aud: None,
            leeway_secs: 30,
            allowed_algorithms: SUPPORTED_ALGORITHMS.to_vec(),
            token_status_checker: None,
            cache_ttl: Duration::from_secs(300),
            cache_ttl_millis: cache_ttl_millis.clone(),
            cache: cache.clone(),
            refresh_in_progress: refresh_in_progress.clone(),
            refresh_complete: refresh_complete.clone(),
            unknown_kid_refresh_cooldown: Duration::from_secs(1),
            last_unknown_kid_refresh: Mutex::new(None),
            claims_cache: std::sync::RwLock::new(LruCache::new(
                NonZeroUsize::new(1000).expect("claims_cache_size must be > 0"),
            )),
            claims_cache_size: 1000,
            cookie_name: None,
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            cache_evictions: AtomicU64::new(0),
            structured_logger: JwtStructuredLogger::new(),
            background_handle: Some(background_handle.clone()),
            shutdown: shutdown.clone(),
            fetch_state: Arc::clone(&fetch_state),
            jwks_poisoning_rejected: AtomicU64::new(0),
        };

        // Start background refresh task
        provider.start_background_refresh_internal(
            cache,
            refresh_in_progress,
            refresh_complete,
            shutdown,
            background_handle,
            cache_ttl_millis,
            fetch_state,
        );

        provider
    }

    /// Configure the per-attempt HTTP timeout for JWKS fetches.
    ///
    /// Default: [`crate::security::DEFAULT_JWKS_FETCH_TIMEOUT`], overridable process-wide with
    /// the `BRRTR_JWKS_FETCH_TIMEOUT_MS` environment variable.
    ///
    /// Two attempts are made per refresh, so the worst-case refresh duration is roughly
    /// `2 × timeout`. The previous value was hard-coded at 200ms, which was sized for a plaintext
    /// in-cluster hop and cannot cover a TLS handshake to a hostname behind an edge — see
    /// [`crate::security::DEFAULT_JWKS_FETCH_TIMEOUT`] for the full reasoning.
    ///
    /// # Note on ordering
    ///
    /// The background refresh thread starts in [`Self::new`], so the very first refresh may
    /// already be in flight using the configured default when this builder runs. The new value
    /// applies to every refresh after that.
    #[must_use]
    pub fn fetch_timeout(self, timeout: Duration) -> Self {
        self.fetch_state.set_timeout(timeout);
        self
    }

    /// Current JWKS key-material health, for metrics and readiness probes.
    #[must_use]
    pub fn jwks_health(&self) -> JwksHealth {
        JwksHealth {
            ever_loaded: self.fetch_state.ever_loaded(),
            keys_cached: self.cache.read().map(|guard| guard.1.len()).unwrap_or(0),
            fetch_success: self.fetch_state.fetch_success.load(Ordering::Relaxed),
            fetch_failure: self.fetch_state.fetch_failure.load(Ordering::Relaxed),
            consecutive_failures: self
                .fetch_state
                .consecutive_failures
                .load(Ordering::Relaxed),
        }
    }

    /// Readiness answer suitable for `AppService::set_readiness_check`.
    ///
    /// A provider that has never fetched a keyset rejects **every** token, so it must not be
    /// reported ready — that state used to be completely invisible: the fetch error went to
    /// `debug!` and callers saw only 401s.
    ///
    /// A *degraded* provider (cached keys present, refresh currently failing) still reports `Ok`
    /// on purpose: it can authenticate existing tokens, and pulling the pod out of the load
    /// balancer for it would turn a partial outage into a total one.
    ///
    /// # Wiring it to `GET /ready`
    ///
    /// ```text
    /// let jwks = Arc::new(JwksBearerProvider::new(jwks_url));
    /// service.register_security_provider("bearerAuth", Arc::clone(&jwks) as Arc<dyn SecurityProvider>);
    /// service.set_readiness_check(Some(Arc::new({
    ///     let jwks = Arc::clone(&jwks);
    ///     move || jwks.readiness()
    /// })));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a human-readable reason when no usable key material is available.
    pub fn readiness(&self) -> Result<(), String> {
        let health = self.jwks_health();
        if !health.ever_loaded {
            return Err(format!(
                "JWKS has never been fetched from {} ({} failed attempts): all JWT validation \
                 will fail. Check DNS, TLS trust and reachability, and {} if the endpoint is slow.",
                self.jwks_url, health.fetch_failure, JWKS_FETCH_TIMEOUT_ENV
            ));
        }
        if health.keys_cached == 0 {
            return Err(format!(
                "JWKS at {} was fetched but contains no usable keys: all JWT validation will fail",
                self.jwks_url
            ));
        }
        Ok(())
    }

    /// How long a thread that lost the refresh race should wait for the winner.
    ///
    /// Derived from the configured timeout rather than fixed: a constant 2s wait was fine when a
    /// refresh could not take longer than `2 × 200ms`, but it would make every waiter give up
    /// before the refresh it is waiting for could possibly have finished once the timeout became
    /// TLS-sized.
    fn refresh_wait_timeout(&self) -> Duration {
        self.fetch_state
            .timeout()
            .saturating_mul(JWKS_FETCH_ATTEMPTS)
            .saturating_add(Duration::from_millis(500))
            .max(Duration::from_secs(2))
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

    /// Restrict JWT algorithms accepted by this provider.
    ///
    /// The token header is untrusted input. Consumers SHOULD configure the smallest set that
    /// their issuer publishes, for example only [`jsonwebtoken::Algorithm::EdDSA`]. Algorithms
    /// outside BRRTRouter's supported set are rejected during startup configuration.
    ///
    /// # Panics
    ///
    /// Panics when `algorithms` is empty or contains an unsupported algorithm. This builder is
    /// intended for startup configuration, where an invalid security policy must fail fast.
    #[allow(clippy::panic)]
    pub fn allowed_algorithms(mut self, algorithms: &[jsonwebtoken::Algorithm]) -> Self {
        if algorithms.is_empty() {
            panic!("allowed_algorithms must contain at least one algorithm");
        }
        if let Some(unsupported) = algorithms
            .iter()
            .find(|algorithm| !SUPPORTED_ALGORITHMS.contains(algorithm))
        {
            panic!("unsupported JWT algorithm in allowed_algorithms: {unsupported:?}");
        }
        self.allowed_algorithms = algorithms.to_vec();
        self
    }

    /// Configure the minimum interval between forced JWKS refreshes for unknown key IDs.
    ///
    /// Unknown kids bypass the normal JWKS cache TTL to support key rotation. The cooldown
    /// bounds attacker-triggered network traffic. The default is one second.
    pub fn unknown_kid_refresh_cooldown(mut self, cooldown: Duration) -> Self {
        self.unknown_kid_refresh_cooldown = cooldown;
        self
    }

    /// Attach a dynamic denylist/version checker.
    ///
    /// The checker runs after cryptographic and standard-claim validation on both cache hits and
    /// misses. Any result other than [`JwtTokenStatus::Active`] rejects the token.
    pub fn token_status_checker(mut self, checker: Arc<dyn JwtTokenStatusChecker>) -> Self {
        self.token_status_checker = Some(checker);
        self
    }

    pub(super) fn algorithm_allowed(&self, algorithm: jsonwebtoken::Algorithm) -> bool {
        self.allowed_algorithms.contains(&algorithm)
    }

    pub(super) fn check_token_status(&self, claims: &serde_json::Value) -> JwtTokenStatus {
        self.token_status_checker
            .as_ref()
            .map_or(JwtTokenStatus::Active, |checker| checker.check(claims))
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
        self.cache_ttl_millis
            .store(ttl.as_millis() as u64, Ordering::Release);
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
    #[allow(clippy::too_many_arguments)]
    fn start_background_refresh_internal(
        &self,
        cache: Arc<RwLock<(Instant, HashMap<String, jsonwebtoken::DecodingKey>)>>,
        refresh_in_progress: Arc<AtomicBool>,
        refresh_complete: Arc<(Mutex<()>, Condvar)>,
        shutdown: Arc<AtomicBool>,
        handle_lock: Arc<RwLock<Option<JoinHandle<()>>>>,
        cache_ttl_millis: Arc<std::sync::atomic::AtomicU64>,
        fetch_state: Arc<JwksFetchState>,
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
                    &fetch_state,
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
                        &fetch_state,
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
                        &fetch_state,
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
    /// * `fetch_state` - Shared fetch timeout and outcome counters.
    ///
    /// # Diagnostics
    ///
    /// Both failure exits used to `return` after nothing louder than the transport layer's
    /// `debug!` (the JSON parse failure logged nothing at all). A JWKS endpoint that never once
    /// answered therefore looked identical, at default log levels, to a healthy one — the only
    /// symptom was that every token got a 401. Failures are now reported at `WARN`/`ERROR`, and
    /// crucially they distinguish:
    ///
    /// * **fatal** — nothing has ever loaded, so *every* token is rejected; and
    /// * **degraded** — a cached keyset is still being served, so only newly rotated keys fail.
    fn refresh_jwks_internal(
        cache: &Arc<RwLock<(Instant, HashMap<String, jsonwebtoken::DecodingKey>)>>,
        jwks_url: &str,
        refresh_in_progress: &Arc<AtomicBool>,
        refresh_complete: &Arc<(Mutex<()>, Condvar)>,
        already_claimed: bool,
        fetch_state: &Arc<JwksFetchState>,
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
        let fetch_timeout = fetch_state.timeout();
        let fetch_options = crate::http::HttpFetchOptions {
            // Configuration, not a literal. This was `Duration::from_millis(200)`, sized for a
            // plaintext in-cluster hop; it cannot cover a TLS handshake to a hostname behind an
            // edge. See `crate::security::DEFAULT_JWKS_FETCH_TIMEOUT`.
            timeout: fetch_timeout,
            max_body_bytes: 256 * 1024,
            extra_headers: Vec::new(),
        };
        let body = match crate::http::fetch_get_text_with_retry(
            jwks_url,
            &fetch_options,
            JWKS_FETCH_ATTEMPTS,
        ) {
            Some(b) => b,
            None => {
                Self::report_refresh_failure(
                    cache,
                    jwks_url,
                    fetch_state,
                    fetch_timeout,
                    refresh_start,
                    "JWKS endpoint could not be fetched",
                );
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
            Err(error) => {
                // A 2xx body that is not JSON means we reached *something* but not the issuer —
                // a captive portal, an edge error page, or the wrong route. Previously this exit
                // was completely silent, which is the hardest possible version to diagnose.
                Self::report_refresh_failure(
                    cache,
                    jwks_url,
                    fetch_state,
                    fetch_timeout,
                    refresh_start,
                    &format!("JWKS response was not valid JSON: {error}"),
                );
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
                let crv = k.get("crv").and_then(|v| v.as_str()).unwrap_or("");

                // JOSE member values are CASE-SENSITIVE. RFC 7518 (JWA) fixes
                // "oct"/"RSA"/"EC" and the alg codes; RFC 8037 fixes "OKP" and
                // "Ed25519"/"EdDSA". We match EXACTLY and reject wrong casing
                // rather than tolerate it — leniency lets a producer's casing
                // bug hide and surface only as an opaque downstream 401. Wrong
                // casing is diagnosed loudly below so it fails with a precise
                // message instead.

                // HMAC (oct) keys for HS* algorithms (RFC 7518).
                if kty == "oct" {
                    if matches!(alg, "HS256" | "HS384" | "HS512") {
                        if let Some(kval) = k.get("k").and_then(|v| v.as_str()) {
                            match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(kval) {
                                Ok(secret) => {
                                    new_map.insert(
                                        kid.to_string(),
                                        jsonwebtoken::DecodingKey::from_secret(&secret),
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(kid, alg, error = %e, "JWKS: oct key 'k' not base64url; rejected");
                                }
                            }
                        } else {
                            tracing::warn!(kid, alg, "JWKS: oct key missing 'k'; rejected");
                        }
                    } else {
                        tracing::warn!(
                            kid,
                            kty,
                            alg,
                            "JWKS: oct key has unsupported/miscased alg; rejected"
                        );
                    }
                    continue;
                }

                // RSA public keys for RS*/PS* algorithms (RFC 7518).
                if kty == "RSA" {
                    if matches!(
                        alg,
                        "RS256" | "RS384" | "RS512" | "PS256" | "PS384" | "PS512"
                    ) {
                        match (
                            k.get("n").and_then(|v| v.as_str()),
                            k.get("e").and_then(|v| v.as_str()),
                        ) {
                            (Some(n), Some(e)) => {
                                if let Ok(dk) = jsonwebtoken::DecodingKey::from_rsa_components(n, e)
                                {
                                    new_map.insert(kid.to_string(), dk);
                                } else {
                                    tracing::warn!(
                                        kid,
                                        alg,
                                        "JWKS: RSA key components invalid; rejected"
                                    );
                                }
                            }
                            _ => tracing::warn!(kid, alg, "JWKS: RSA key missing n/e; rejected"),
                        }
                    } else {
                        tracing::warn!(
                            kid,
                            kty,
                            alg,
                            "JWKS: RSA key has unsupported/miscased alg; rejected"
                        );
                    }
                    continue;
                }

                // EC public keys for ES* algorithms (RFC 7518).
                if kty == "EC" {
                    if matches!(alg, "ES256" | "ES384") {
                        match (
                            k.get("x").and_then(|v| v.as_str()),
                            k.get("y").and_then(|v| v.as_str()),
                        ) {
                            (Some(x), Some(y)) => {
                                if let Ok(dk) = jsonwebtoken::DecodingKey::from_ec_components(x, y)
                                {
                                    new_map.insert(kid.to_string(), dk);
                                } else {
                                    tracing::warn!(
                                        kid,
                                        alg,
                                        "JWKS: EC key components invalid; rejected"
                                    );
                                }
                            }
                            _ => tracing::warn!(kid, alg, "JWKS: EC key missing x/y; rejected"),
                        }
                    } else {
                        tracing::warn!(
                            kid,
                            kty,
                            alg,
                            "JWKS: EC key has unsupported/miscased alg; rejected"
                        );
                    }
                    continue;
                }

                // OKP public keys for EdDSA (Ed25519), RFC 8037. STRICT:
                //  - kty MUST be exactly "OKP"
                //  - crv MUST be exactly "Ed25519" (RFC 8037: crv is REQUIRED)
                //  - alg is OPTIONAL, but if present MUST be exactly "EdDSA"
                if kty == "OKP" {
                    if crv != "Ed25519" {
                        tracing::warn!(kid, kty, crv, "JWKS: OKP key crv must be exactly \"Ed25519\" (RFC 8037, case-sensitive); rejected");
                        continue;
                    }
                    if !alg.is_empty() && alg != "EdDSA" {
                        tracing::warn!(
                            kid,
                            kty,
                            alg,
                            "JWKS: OKP key alg must be \"EdDSA\" or omitted (RFC 8037); rejected"
                        );
                        continue;
                    }
                    match k.get("x").and_then(|v| v.as_str()) {
                        Some(x) => match jsonwebtoken::DecodingKey::from_ed_components(x) {
                            Ok(dk) => {
                                new_map.insert(kid.to_string(), dk);
                            }
                            Err(e) => {
                                tracing::warn!(kid, kty, crv, error = %e, "JWKS: OKP key 'x' invalid; rejected");
                            }
                        },
                        None => {
                            tracing::warn!(kid, kty, crv, "JWKS: OKP key missing 'x'; rejected")
                        }
                    }
                    continue;
                }

                // No exact kty match. Detect a wrong-CASE near-miss (e.g.
                // "okp" vs "OKP") and reject it with a precise, actionable
                // message — this is exactly the diagnostic that was missing
                // when a producer served a non-RFC-cased JWKS.
                let rfc_kty = ["OKP", "RSA", "EC", "oct"]
                    .into_iter()
                    .find(|c| kty.eq_ignore_ascii_case(c));
                match rfc_kty {
                    Some(correct) => tracing::warn!(
                        kid, kty, crv, alg, expected = correct,
                        "JWKS: kty has non-RFC casing (JOSE member values are case-sensitive); key REJECTED — the producer must emit the exact RFC casing"
                    ),
                    None => tracing::warn!(
                        kid, kty, crv, alg,
                        "JWKS: unrecognized kty; key rejected, tokens with this kid will 401"
                    ),
                }
            }
        }

        let key_count = new_map.len();
        let refresh_duration = refresh_start.elapsed();

        if let Ok(mut guard) = cache.write() {
            *guard = (Instant::now(), new_map);
        }

        let ended_failure_streak = fetch_state.record_success();

        refresh_in_progress.store(false, Ordering::Release);

        // Notify all waiting threads that refresh has completed
        // This wakes them immediately instead of waiting for their next poll
        let (lock, cvar) = &**refresh_complete;
        let _guard = lock.lock().unwrap();
        cvar.notify_all();

        if key_count == 0 {
            // A parseable but empty keyset is a legitimate response (see the note in
            // `refresh_jwks_if_needed`), yet it still means nothing can be validated. Say so.
            warn!(
                jwks_url = %jwks_url,
                elapsed_ms = refresh_duration.as_millis() as u64,
                "JWKS refresh succeeded but the document contains no usable keys; all JWT validation will fail"
            );
        } else if ended_failure_streak > 0 {
            // Recovery is as operationally interesting as the failure — it closes the incident.
            info!(
                jwks_url = %jwks_url,
                keys = key_count,
                elapsed_ms = refresh_duration.as_millis() as u64,
                recovered_after_failures = ended_failure_streak,
                "JWKS refresh recovered after consecutive failures"
            );
        }

        debug!(
            "JWKS refresh completed in {:?} (keys: {})",
            refresh_duration, key_count
        );
    }

    /// Log a failed JWKS refresh at a severity that reflects its operational impact.
    ///
    /// `ERROR` when no keyset has ever loaded (fatal: every token is rejected), `WARN` when a
    /// cached keyset is still being served (degraded: only key rotation is affected). The message
    /// names the URL, elapsed time, attempt count and configured timeout so the first log line an
    /// operator reads already distinguishes DNS/TLS/edge/upstream instead of leaving them to
    /// eliminate each by hand.
    fn report_refresh_failure(
        cache: &Arc<RwLock<(Instant, HashMap<String, jsonwebtoken::DecodingKey>)>>,
        jwks_url: &str,
        fetch_state: &Arc<JwksFetchState>,
        fetch_timeout: Duration,
        refresh_start: Instant,
        reason: &str,
    ) {
        let consecutive_failures = fetch_state.record_failure();
        let elapsed_ms = refresh_start.elapsed().as_millis() as u64;
        let timeout_ms = fetch_timeout.as_millis() as u64;

        let (cached_keys, cache_age_ms) = cache
            .read()
            .map(|guard| (guard.1.len(), guard.0.elapsed().as_millis() as u64))
            .unwrap_or((0, 0));

        if !fetch_state.ever_loaded() || cached_keys == 0 {
            error!(
                jwks_url = %jwks_url,
                reason,
                elapsed_ms,
                attempts = JWKS_FETCH_ATTEMPTS,
                timeout_ms,
                consecutive_failures,
                ever_loaded = fetch_state.ever_loaded(),
                "JWKS refresh failed and no keyset is available: ALL JWT validation will fail with 401 until this succeeds. \
                 Check DNS resolution, TLS trust and reachability of the JWKS URL; if the endpoint is simply slow, raise \
                 BRRTR_JWKS_FETCH_TIMEOUT_MS (the timeout used here) or JwksBearerProvider::fetch_timeout."
            );
        } else {
            warn!(
                jwks_url = %jwks_url,
                reason,
                elapsed_ms,
                attempts = JWKS_FETCH_ATTEMPTS,
                timeout_ms,
                consecutive_failures,
                cached_keys,
                cache_age_ms,
                "JWKS refresh failed; serving the cached keyset (DEGRADED). Existing tokens still validate, but tokens \
                 signed with a newly rotated key will be rejected until a refresh succeeds."
            );
        }
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
        let current_cache_ttl =
            Duration::from_millis(self.cache_ttl_millis.load(Ordering::Acquire));
        // Check if refresh is needed (non-blocking read).
        //
        // **Do not** use `|| guard.1.is_empty()` here: a successful JWKS fetch may legitimately
        // return `{"keys":[]}` — we still update the cache timestamp in `refresh_jwks_internal`.
        // If we always refreshed whenever keys were empty, every `validate()` would re-fetch,
        // breaking `test_jwks_empty_cache_no_retry_on_successful_empty_response` and wasting
        // traffic. Failed refreshes leave the old timestamp in place, so TTL expiry still
        // triggers another attempt.
        let (needs_refresh, is_empty) = {
            if let Ok(guard) = self.cache.read() {
                (guard.0.elapsed() >= current_cache_ttl, guard.1.is_empty())
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
                    &self.fetch_state,
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
                                &self.fetch_state,
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

                // Derived from the configured fetch timeout, not fixed. This used to be a flat
                // `Duration::from_secs(2)` with the comment "400ms max + buffer" — true only while
                // the fetch timeout was hard-coded at 200ms. With a TLS-sized timeout a fixed 2s
                // wait expires before the refresh being waited on can possibly have finished.
                let timeout = self.refresh_wait_timeout();
                let (lock, cvar) = &*self.refresh_complete;
                let guard = lock.lock().unwrap();

                // Wait for refresh to complete, with timeout
                // The condvar will wake us immediately when refresh_in_progress becomes false
                let wait_result = cvar.wait_timeout_while(guard, timeout, |_| {
                    // Continue waiting while refresh is in progress
                    self.refresh_in_progress.load(Ordering::Acquire)
                });

                let (wait_guard, wait_timeout_result) = match wait_result {
                    Ok(result) => result,
                    Err(_) => {
                        // Lock poisoned, give up
                        return;
                    }
                };

                drop(wait_guard);

                // Check if we timed out or if refresh completed
                if wait_timeout_result.timed_out()
                    && self.refresh_in_progress.load(Ordering::Acquire)
                {
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
                                    &self.fetch_state,
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
                                &self.fetch_state,
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
            let fetch_state_thread = Arc::clone(&self.fetch_state);

            // CRITICAL: If thread::spawn panics (e.g., resource exhaustion), we must clear
            // the refresh_in_progress flag to prevent permanent deadlock. The spawned thread
            // is responsible for clearing it, but if spawning fails, that never happens.
            match thread::Builder::new().spawn(move || {
                // We've already atomically claimed the refresh (set flag to true above),
                // so pass already_claimed=true to skip the flag check in refresh_jwks_internal
                Self::refresh_jwks_internal(
                    &cache,
                    &jwks_url,
                    &refresh_in_progress_thread,
                    &refresh_complete_thread,
                    true,
                    &fetch_state_thread,
                );
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

    /// Force one bounded JWKS refresh after an unknown `kid` cache miss.
    ///
    /// The refresh is globally coalesced by `refresh_in_progress` and rate-limited by
    /// `unknown_kid_refresh_cooldown`. A caller that loses the refresh race waits for the
    /// in-flight refresh instead of starting another network request.
    fn refresh_jwks_for_unknown_kid(&self) {
        let should_start = match self.last_unknown_kid_refresh.lock() {
            Ok(mut last_refresh) => {
                let cooldown_elapsed = last_refresh
                    .as_ref()
                    .is_none_or(|instant| instant.elapsed() >= self.unknown_kid_refresh_cooldown);
                if cooldown_elapsed {
                    *last_refresh = Some(Instant::now());
                }
                cooldown_elapsed
            }
            Err(_) => false,
        };

        if should_start
            && self
                .refresh_in_progress
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
        {
            Self::refresh_jwks_internal(
                &self.cache,
                &self.jwks_url,
                &self.refresh_in_progress,
                &self.refresh_complete,
                true,
                &self.fetch_state,
            );
            return;
        }

        if self.refresh_in_progress.load(Ordering::Acquire) {
            let (lock, cvar) = &*self.refresh_complete;
            if let Ok(guard) = lock.lock() {
                // Scaled to the configured fetch timeout for the same reason as above.
                let timeout = self.refresh_wait_timeout();
                let _ = cvar.wait_timeout_while(guard, timeout, |_| {
                    self.refresh_in_progress.load(Ordering::Acquire)
                });
            }
        }
    }

    /// Get decoding key for a given key ID (kid)
    ///
    /// P1: Non-blocking - uses lock-free reads (RwLock) and triggers refresh in background.
    /// If refresh fails, uses stale cache (graceful degradation).
    pub(super) fn get_key_for(&self, kid: &str) -> Option<jsonwebtoken::DecodingKey> {
        let span = tracing::span!(Level::DEBUG, "jwks_cache", kid = kid,);
        let _guard = span.enter();

        // Trigger refresh if needed (non-blocking)
        self.refresh_jwks_if_needed();

        // Lock-free read (RwLock allows concurrent reads)
        let cache_hit = if let Ok(guard) = self.cache.read() {
            let result = guard.1.get(kid).cloned();
            span.record("cache_hit", result.is_some());
            result.is_some()
        } else {
            span.record("cache_hit", false);
            false
        };

        if cache_hit {
            span.record("cache_hit", true);
            // Record cache age from the cache timestamp
            if let Ok(guard) = self.cache.read() {
                let age = guard.0.elapsed();
                span.record("cache_age_seconds", age.as_secs_f64());
            }
        }

        // Re-read to return the actual key.
        if let Ok(guard) = self.cache.read() {
            if let Some(key) = guard.1.get(kid).cloned() {
                return Some(key);
            }
        }

        // A cache can be fresh while the issuer has just rotated to a new kid. Force a
        // coalesced, rate-limited refresh rather than rejecting until the normal TTL expires.
        self.refresh_jwks_for_unknown_kid();

        self.cache
            .read()
            .ok()
            .and_then(|guard| guard.1.get(kid).cloned())
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
/// - **EC/OKP**: ES256 and EdDSA (asymmetric keys)
///
/// Consumers can restrict this default compatibility set with
/// [`JwksBearerProvider::allowed_algorithms`].
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
        // Story 9.5: token.validation span — child of jwt_validation (if available)
        // Records token_size_bytes, header_size_bytes, token_version, result
        let token = self.extract_token(req);
        let token_size = token.as_ref().map_or(0f64, |t| t.len() as f64);
        let header_size = req
            .get_header("Authorization")
            .map_or(0f64, |h| h.len() as f64);

        let span = tracing::span!(
            tracing::Level::DEBUG,
            "token.validation",
            token_size_bytes = token_size,
            header_size_bytes = header_size,
            token_version = 0u64,
            result = tracing::field::Empty,
        );
        let _guard = span.enter();

        let result = validation::validate_token_impl(self, scheme, scopes, req);

        // Record result in span attribute
        span.record("result", if result { "valid" } else { "invalid" });

        // Record token version from claims if available
        if let Some(t) = &token {
            if let Ok(header) = jsonwebtoken::decode_header(t) {
                if let Some(kid) = &header.kid {
                    // Try to get token version from claims cache
                    if let Ok(mut cache) = self.claims_cache.write() {
                        let cache_key: Arc<str> = Arc::from(format!("{}|{}", t, kid));
                        if let Some((_, claims, _)) = cache.get::<Arc<str>>(&cache_key.into()) {
                            if let Some(ver) = claims.get("ver") {
                                if let Some(v) = ver.as_u64() {
                                    span.record("token_version", v);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Structured log on validation failure (WARN level)
        if !result {
            tracing::warn!(
                event = "token_validation_failed",
                token_size_bytes = token_size,
                header_size_bytes = header_size,
                route = tracing::field::Empty, // would need route context
                "Token validation failed"
            );
        }

        result
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
