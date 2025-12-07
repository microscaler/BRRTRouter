use crate::security::{SecurityProvider, SecurityRequest};
use crate::spec::SecurityScheme;
use std::collections::HashMap;
use std::time::{Duration, Instant};

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
        req.get_header(header_name).or_else(|| {
            req.get_header("authorization")
                .and_then(|h| h.strip_prefix("Bearer "))
        })
    }
}

/// Remote API key provider implementation
///
/// Validates API keys by making HTTP requests to an external verification service.
/// Implements caching to reduce latency and load on the verification endpoint.
///
/// # Validation Flow
///
/// 1. Verify security scheme is API Key with `location: header`
/// 2. Extract API key from configured header (or Authorization: Bearer)
/// 3. Check cache for recent validation result
/// 4. If cache miss or expired: Make HTTP GET to verification URL
/// 5. Cache result (success/failure) with TTL
/// 6. Return validation result
///
/// # Key Extraction Priority
///
/// 1. **Custom header**: `X-API-Key` (or configured via `header_name()`)
/// 2. **OpenAPI spec header**: Uses `name` from `securityScheme.name`
/// 3. **Authorization header**: Falls back to `Authorization: Bearer {key}`
///
/// # Verification Request
///
/// Makes HTTP GET to configured URL with key in `X-API-Key` header:
///
/// ```text
/// GET /verify HTTP/1.1
/// Host: api.example.com
/// X-API-Key: user_api_key_here
/// ```
///
/// Success: 2xx status code
/// Failure: Any other status code or timeout
///
/// # Caching
///
/// - **TTL**: Configurable (default: 60 seconds)
/// - **Storage**: In-memory HashMap with Mutex
/// - **Key**: API key string
/// - **Value**: (timestamp, valid: bool)
/// - **Eviction**: Lazy (checks on read)
///
/// # Usage
///
/// ```rust
/// use brrtrouter::security::RemoteApiKeyProvider;
/// use std::time::Duration;
///
/// let provider = RemoteApiKeyProvider::new("https://auth.example.com/verify")
///     .timeout_ms(1000)                             // 1 second timeout
///     .cache_ttl(Duration::from_secs(300))           // 5 minute cache
///     .header_name("X-Custom-Key");                 // Custom header
/// ```
///
/// # Performance
///
/// - **Cache hit**: ~1µs (HashMap lookup)
/// - **Cache miss**: ~50-500ms (HTTP request)
/// - Recommendation: Use longer TTL for trusted environments
impl SecurityProvider for RemoteApiKeyProvider {
    /// Validate an API key by making an HTTP request to verification service
    ///
    /// Uses caching to avoid repeated verification requests for the same key.
    ///
    /// # Arguments
    ///
    /// * `scheme` - Security scheme from OpenAPI spec (must be API Key)
    /// * `_scopes` - Required scopes (unused - API keys don't have scopes)
    /// * `req` - The security request containing headers
    ///
    /// # Returns
    ///
    /// - `true` - API key is valid (cached or verified remotely)
    /// - `false` - API key missing, invalid, or verification failed
    ///
    /// # Cache Behavior
    ///
    /// - Cached success: Returns true immediately
    /// - Cached failure: Returns false immediately
    /// - Expired cache: Makes new verification request
    /// - New key: Makes verification request and caches result
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
        if let Some((ts, ok)) = self
            .cache
            .lock()
            .expect("API key cache Mutex poisoned - critical error")
            .get(key)
            .cloned()
        {
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
            .expect("API key cache Mutex poisoned - critical error")
            .insert(key.to_string(), (Instant::now(), ok));
        ok
    }
}
