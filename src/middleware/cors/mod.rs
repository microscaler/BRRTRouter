mod builder;
mod error;
mod route_config;

pub use builder::CorsMiddlewareBuilder;
pub use error::CorsConfigError;
pub use route_config::{
    build_route_cors_map, extract_route_cors_config, RouteCorsConfig,
};

use std::sync::Arc;
use std::time::Duration;

use http::Method;
use regex::Regex;
use serde_json::Value;
use tracing::{debug, warn};

use crate::dispatcher::{HandlerRequest, HandlerResponse, HeaderVec};
use crate::middleware::Middleware;

/// CORS (Cross-Origin Resource Sharing) middleware
///
/// Handles preflight OPTIONS requests and adds CORS headers to responses.
/// Configurable with allowed origins, headers, and methods.
///
/// # Security
///
/// - Validates Origin header against allowed_origins whitelist
/// - Only adds CORS headers for valid cross-origin requests
/// - Skips CORS headers for same-origin requests
/// - Returns 403 Forbidden for invalid origins
/// - Supports credentials, exposed headers, and preflight caching
///
/// # Credentials
///
/// When `allow_credentials` is `true`, the `Access-Control-Allow-Credentials` header
/// is set to `true`. **Important**: When credentials are allowed, wildcard origin (`*`)
/// is not permitted by the CORS specification. The middleware will panic if this
/// invalid combination is detected.
///
/// # Usage
///
/// ## Builder Pattern (Recommended)
///
/// ```rust,ignore
/// use brrtrouter::middleware::CorsMiddlewareBuilder;
/// use http::Method;
///
/// let cors = CorsMiddlewareBuilder::new()
///     .allowed_origins(&["https://example.com"])
///     .allowed_methods(&[Method::GET, Method::POST])
///     .allow_credentials(true)
///     .build()
///     .expect("Invalid CORS configuration");
/// ```
///
/// ## Direct Construction
///
/// ```rust,ignore
/// use brrtrouter::middleware::CorsMiddleware;
/// use http::Method;
///
/// let cors = CorsMiddleware::new(
///     vec!["https://example.com".to_string()],
///     vec!["Content-Type".to_string()],
///     vec![Method::GET, Method::POST],
///     true,  // allow credentials
///     vec!["X-Total-Count".to_string()],  // expose headers
///     Some(3600),  // cache preflight for 1 hour
/// );
/// ```
/// Origin validation strategy
#[derive(Clone)]
pub enum OriginValidation {
    /// Exact string matching
    Exact(Vec<String>),
    /// Wildcard (allow all origins)
    Wildcard,
    /// Regex pattern matching
    Regex(Vec<Regex>),
    /// Custom validation function
    Custom(Arc<dyn Fn(&str) -> bool + Send + Sync>),
}

impl std::fmt::Debug for OriginValidation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OriginValidation::Exact(origins) => f.debug_tuple("Exact").field(origins).finish(),
            OriginValidation::Wildcard => write!(f, "Wildcard"),
            OriginValidation::Regex(patterns) => {
                f.debug_tuple("Regex")
                    .field(&patterns.iter().map(|re| re.as_str()).collect::<Vec<_>>())
                    .finish()
            }
            OriginValidation::Custom(_) => write!(f, "Custom(<function>)"),
        }
    }
}

impl OriginValidation {
    /// Check if an origin is allowed
    fn is_allowed(&self, origin: &str) -> bool {
        match self {
            OriginValidation::Exact(origins) => origins.iter().any(|o| o == origin),
            OriginValidation::Wildcard => true,
            OriginValidation::Regex(patterns) => patterns.iter().any(|re| re.is_match(origin)),
            OriginValidation::Custom(validator) => validator(origin),
        }
    }

    /// Check if wildcard is enabled (for credentials validation)
    fn is_wildcard(&self) -> bool {
        matches!(self, OriginValidation::Wildcard)
    }
}

pub struct CorsMiddleware {
    pub(crate) origin_validation: OriginValidation,
    pub(crate) allowed_headers: Vec<String>,
    pub(crate) allowed_methods: Vec<Method>,
    pub(crate) allow_credentials: bool,
    pub(crate) expose_headers: Vec<String>,
    pub(crate) max_age: Option<u32>,
    /// Route-specific CORS configurations keyed by handler name
    /// If a route has an `x-cors` extension in OpenAPI, it overrides global settings
    pub(crate) route_configs: std::collections::HashMap<String, RouteCorsConfig>,
}

impl CorsMiddleware {
    /// Create a new CORS middleware with specific configuration
    ///
    /// # Arguments
    ///
    /// * `allowed_origins` - List of allowed origins (e.g., `["https://example.com"]`)
    ///   - Use `["*"]` to allow all origins (insecure, not recommended for production)
    ///   - Only one origin is returned per response (CORS spec requirement)
    ///   - **Cannot use wildcard (`*`) with credentials** - will panic if both are set
    /// * `allowed_headers` - List of allowed headers (e.g., `["Content-Type", "Authorization"]`)
    /// * `allowed_methods` - List of allowed HTTP methods
    /// * `allow_credentials` - If `true`, sets `Access-Control-Allow-Credentials: true`
    ///   - Cannot be used with wildcard origin (`*`)
    /// * `expose_headers` - List of headers to expose to JavaScript (e.g., `["X-Total-Count"]`)
    /// * `max_age` - Preflight cache duration in seconds (e.g., `Some(3600)` for 1 hour)
    ///
    /// # Panics
    ///
    /// Panics if `allow_credentials` is `true` and `allowed_origins` contains `"*"`.
    /// This violates the CORS specification and is a security risk.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use brrtrouter::middleware::CorsMiddleware;
    /// use http::Method;
    ///
    /// let cors = CorsMiddleware::new(
    ///     vec!["https://example.com".to_string()],
    ///     vec!["Content-Type".to_string()],
    ///     vec![Method::GET, Method::POST],
    ///     true,  // allow credentials
    ///     vec!["X-Total-Count".to_string()],  // expose headers
    ///     Some(3600),  // cache preflight for 1 hour
    /// );
    /// ```
    pub fn new(
        allowed_origins: Vec<String>,
        allowed_headers: Vec<String>,
        allowed_methods: Vec<Method>,
        allow_credentials: bool,
        expose_headers: Vec<String>,
        max_age: Option<u32>,
    ) -> Self {
        // Determine origin validation strategy
        let origin_validation = if allowed_origins.iter().any(|o| o == "*") {
            OriginValidation::Wildcard
        } else {
            OriginValidation::Exact(allowed_origins)
        };

        // Validate: cannot use wildcard with credentials (CORS spec requirement)
        // JSF Compliance: Panics only during initialization, never on hot path
        // This method is only called during startup in templates/main.rs.txt
        if allow_credentials && origin_validation.is_wildcard() {
            #[allow(clippy::panic)]
            panic!(
                "CORS configuration error: Cannot use wildcard origin (*) with credentials. \
                When allow_credentials is true, you must specify exact origins."
            );
        }

        Self {
            origin_validation,
            allowed_headers,
            allowed_methods,
            allow_credentials,
            expose_headers,
            max_age,
            route_configs: std::collections::HashMap::new(),
        }
    }

    /// Create a route-aware CORS middleware with OpenAPI route configurations
    ///
    /// This constructor allows you to provide route-specific CORS configurations
    /// extracted from OpenAPI `x-cors` extensions. Routes with `x-cors` will
    /// use their specific config, others will use the global config.
    ///
    /// # Arguments
    ///
    /// * `global_config` - Global CORS configuration (used as fallback)
    /// * `route_configs` - Map of handler names to route-specific CORS configs
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use brrtrouter::middleware::{CorsMiddleware, RouteCorsConfig};
    /// use brrtrouter::spec::load_spec;
    /// use std::collections::HashMap;
    ///
    /// let (routes, _) = load_spec("openapi.yaml")?;
    /// let route_configs = build_route_cors_map(&routes);
    ///
    /// let global_cors = CorsMiddlewareBuilder::new()
    ///     .allowed_origins(&["https://example.com"])
    ///     .build()?;
    ///
    /// let cors = CorsMiddleware::with_route_configs(global_cors, route_configs);
    /// ```
    pub fn with_route_configs(
        global_config: CorsMiddleware,
        route_configs: std::collections::HashMap<String, RouteCorsConfig>,
    ) -> Self {
        Self {
            origin_validation: global_config.origin_validation,
            allowed_headers: global_config.allowed_headers,
            allowed_methods: global_config.allowed_methods,
            allow_credentials: global_config.allow_credentials,
            expose_headers: global_config.expose_headers,
            max_age: global_config.max_age,
            route_configs,
        }
    }

    /// Create a new CORS middleware with regex pattern matching
    ///
    /// Allows origins that match any of the provided regex patterns.
    ///
    /// # Arguments
    ///
    /// * `origin_patterns` - Vector of regex patterns (e.g., `vec![r"^https://.*\.example\.com$"]`)
    /// * `allowed_headers` - List of allowed headers
    /// * `allowed_methods` - List of allowed HTTP methods
    /// * `allow_credentials` - If `true`, sets `Access-Control-Allow-Credentials: true`
    /// * `expose_headers` - List of headers to expose to JavaScript
    /// * `max_age` - Preflight cache duration in seconds
    ///
    /// # Panics
    ///
    /// Panics if any regex pattern is invalid or if `allow_credentials` is `true` with wildcard patterns.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use brrtrouter::middleware::CorsMiddleware;
    /// use http::Method;
    ///
    /// let cors = CorsMiddleware::with_regex_patterns(
    ///     vec![r"^https://.*\.example\.com$".to_string()],
    ///     vec!["Content-Type".to_string()],
    ///     vec![Method::GET, Method::POST],
    ///     false,
    ///     vec![],
    ///     None,
    /// );
    /// ```
    pub fn with_regex_patterns(
        origin_patterns: Vec<String>,
        allowed_headers: Vec<String>,
        allowed_methods: Vec<Method>,
        allow_credentials: bool,
        expose_headers: Vec<String>,
        max_age: Option<u32>,
    ) -> Self {
        // Compile regex patterns
        let patterns: Result<Vec<Regex>, _> = origin_patterns
            .iter()
            .map(|p| Regex::new(p))
            .collect();

        // JSF Compliance: Panics only during initialization, never on hot path
        // This method is only called during startup in templates/main.rs.txt
        let patterns = patterns.unwrap_or_else(|e| {
            #[allow(clippy::panic)]
            panic!("CORS configuration error: Invalid regex pattern: {}", e);
        });

        let origin_validation = OriginValidation::Regex(patterns);

        // Validate: cannot use wildcard with credentials
        // JSF Compliance: Panics only during initialization, never on hot path
        // This method is only called during startup in templates/main.rs.txt
        if allow_credentials && origin_validation.is_wildcard() {
            #[allow(clippy::panic)]
            panic!(
                "CORS configuration error: Cannot use wildcard patterns with credentials. \
                When allow_credentials is true, you must use exact origins or specific regex patterns."
            );
        }

        Self {
            origin_validation,
            allowed_headers,
            allowed_methods,
            allow_credentials,
            expose_headers,
            max_age,
            route_configs: std::collections::HashMap::new(),
        }
    }

    /// Create a new CORS middleware with custom validation function
    ///
    /// Allows origins based on a custom validation function.
    ///
    /// # Arguments
    ///
    /// * `validator` - Function that takes an origin string and returns `true` if allowed
    /// * `allowed_headers` - List of allowed headers
    /// * `allowed_methods` - List of allowed HTTP methods
    /// * `allow_credentials` - If `true`, sets `Access-Control-Allow-Credentials: true`
    /// * `expose_headers` - List of headers to expose to JavaScript
    /// * `max_age` - Preflight cache duration in seconds
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use brrtrouter::middleware::CorsMiddleware;
    /// use http::Method;
    ///
    /// let cors = CorsMiddleware::with_custom_validator(
    ///     |origin: &str| origin.ends_with(".example.com"),
    ///     vec!["Content-Type".to_string()],
    ///     vec![Method::GET, Method::POST],
    ///     false,
    ///     vec![],
    ///     None,
    /// );
    /// ```
    pub fn with_custom_validator<F>(
        validator: F,
        allowed_headers: Vec<String>,
        allowed_methods: Vec<Method>,
        allow_credentials: bool,
        expose_headers: Vec<String>,
        max_age: Option<u32>,
    ) -> Self
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        let origin_validation = OriginValidation::Custom(Arc::new(validator));

        Self {
            origin_validation,
            allowed_headers,
            allowed_methods,
            allow_credentials,
            expose_headers,
            max_age,
            route_configs: std::collections::HashMap::new(),
        }
    }

    /// Get route-specific CORS config for a handler, or return global config
    ///
    /// # Arguments
    ///
    /// * `handler_name` - The handler name to look up
    ///
    /// # Returns
    ///
    /// Route-specific config if found, otherwise uses global config
    fn get_route_config(&self, handler_name: &str) -> Option<&RouteCorsConfig> {
        self.route_configs.get(handler_name)
    }

    /// Create a new CORS middleware with legacy configuration (backward compatibility)
    ///
    /// This method maintains backward compatibility with the old API.
    /// For new code, prefer using `CorsMiddlewareBuilder` for a more ergonomic API.
    ///
    /// # Arguments
    ///
    /// * `allowed_origins` - List of allowed origins
    /// * `allowed_headers` - List of allowed headers
    /// * `allowed_methods` - List of allowed HTTP methods
    ///
    /// # Defaults
    ///
    /// - `allow_credentials`: `false`
    /// - `expose_headers`: empty
    /// - `max_age`: `None` (no preflight caching)
    pub fn new_legacy(
        allowed_origins: Vec<String>,
        allowed_headers: Vec<String>,
        allowed_methods: Vec<Method>,
    ) -> Self {
        Self::new(
            allowed_origins,
            allowed_headers,
            allowed_methods,
            false, // no credentials by default
            vec![], // no exposed headers
            None, // no preflight caching
        )
    }

    /// Validate an origin against the allowed origins list
    ///
    /// Returns the validated origin string if valid, None otherwise.
    /// Supports exact matching, wildcard, regex patterns, and custom validators.
    /// Uses route-specific config if available, otherwise falls back to global config.
    ///
    /// **JSF Compliance**: All configuration is pre-processed at startup.
    /// This method only performs O(1) HashMap lookups and string comparisons.
    /// The only allocation is for the return value (necessary for response headers).
    ///
    /// # Arguments
    ///
    /// * `origin` - The Origin header value from the request
    /// * `handler_name` - The handler name (for route-specific config lookup)
    ///
    /// # Returns
    ///
    /// * `Some(origin)` - If origin is allowed (returns the origin string to use in headers)
    /// * `None` - If origin is not allowed
    fn validate_origin(&self, origin: &str, handler_name: &str) -> Option<String> {
        // Check for route-specific config first
        let validation = if let Some(route_config) = self.get_route_config(handler_name) {
            &route_config.origin_validation
        } else {
            &self.origin_validation
        };

        if validation.is_allowed(origin) {
            // For wildcard, return "*", otherwise return the origin itself
            if validation.is_wildcard() {
                Some("*".to_string())
            } else {
                Some(origin.to_string())
            }
        } else {
            None
        }
    }

    /// Check if a request is same-origin (no CORS headers needed)
    ///
    /// Same-origin requests don't need CORS headers. This function extracts
    /// the server origin from the request and compares it to the request origin.
    ///
    /// # Arguments
    ///
    /// * `req` - The incoming request
    /// * `origin` - The Origin header value
    ///
    /// # Returns
    ///
    /// * `true` - Request is same-origin (skip CORS headers)
    /// * `false` - Request is cross-origin (add CORS headers)
    fn is_same_origin(&self, req: &HandlerRequest, origin: &str) -> bool {
        // Extract server origin from Host header
        let host = match req.get_header("host") {
            Some(h) => h,
            None => return false, // No Host header, assume cross-origin
        };

        // Parse origin to extract scheme and host:port
        // Origin format: scheme://host:port
        let origin_parts: Vec<&str> = origin.split("://").collect();
        if origin_parts.len() != 2 {
            return false; // Invalid origin format
        }

        let origin_host_port = origin_parts[1];
        let origin_host = origin_parts[1].split(':').next().unwrap_or(origin_host_port);

        // Compare host (case-insensitive per RFC)
        host.eq_ignore_ascii_case(origin_host) || host.eq_ignore_ascii_case(origin_host_port)
    }

    /// Validate a preflight request
    ///
    /// Checks that the requested method and headers are in the allowed lists.
    /// Uses route-specific config if available.
    ///
    /// # Arguments
    ///
    /// * `req` - The OPTIONS request
    /// * `origin` - The validated origin
    ///
    /// # Returns
    ///
    /// * `Some(response)` - Valid preflight request with CORS headers
    /// * `None` - Invalid preflight request (should return 403)
    fn handle_preflight(&self, req: &HandlerRequest, origin: &str) -> Option<HandlerResponse> {
        // Get route-specific config if available
        let (allowed_methods, allowed_headers, allow_credentials, max_age) =
            if let Some(route_config) = self.get_route_config(&req.handler_name) {
                (
                    &route_config.allowed_methods,
                    &route_config.allowed_headers,
                    route_config.allow_credentials,
                    route_config.max_age,
                )
            } else {
                (
                    &self.allowed_methods,
                    &self.allowed_headers,
                    self.allow_credentials,
                    self.max_age,
                )
            };
        // Extract requested method
        let requested_method = req.get_header("access-control-request-method")?;
        let requested_method = match requested_method.parse::<Method>() {
            Ok(m) => m,
            Err(_) => {
                warn!("CORS preflight: invalid Access-Control-Request-Method: {}", requested_method);
                return None;
            }
        };

        // Validate method
        if !allowed_methods.contains(&requested_method) {
            warn!(
                "CORS preflight: method {} not in allowed methods",
                requested_method.as_str()
            );
            return None;
        }

        // Extract and validate requested headers
        let requested_headers = req.get_header("access-control-request-headers");
        if let Some(headers_str) = requested_headers {
            let requested_headers_list: Vec<&str> = headers_str
                .split(',')
                .map(|h| h.trim())
                .collect();

            // Check if all requested headers are allowed
            // If allowed_headers contains "*", allow all
            let allow_all_headers = allowed_headers.iter().any(|h| h == "*");
            if !allow_all_headers {
                for header in &requested_headers_list {
                    if !allowed_headers
                        .iter()
                        .any(|h| h.eq_ignore_ascii_case(header))
                    {
                        warn!(
                            "CORS preflight: header '{}' not in allowed headers",
                            header
                        );
                        return None;
                    }
                }
            }
        }

        // Build preflight response with CORS headers
        let mut headers = HeaderVec::new();
        headers.push((
            std::sync::Arc::from("access-control-allow-origin"),
            origin.to_string(),
        ));
        headers.push((
            std::sync::Arc::from("access-control-allow-methods"),
            allowed_methods
                .iter()
                .map(|m| m.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        ));
        headers.push((
            std::sync::Arc::from("access-control-allow-headers"),
            allowed_headers.join(", "),
        ));

        // Add credentials header if enabled
        if allow_credentials {
            headers.push((
                std::sync::Arc::from("access-control-allow-credentials"),
                "true".to_string(),
            ));
        }

        // Add preflight cache duration if configured
        if let Some(age) = max_age {
            headers.push((
                std::sync::Arc::from("access-control-max-age"),
                age.to_string(),
            ));
        }

        // Add Vary: Origin header for dynamic origin validation
        headers.push((
            std::sync::Arc::from("vary"),
            "Origin".to_string(),
        ));

        Some(HandlerResponse::new(200, headers, Value::Null))
    }

    /// Create a permissive CORS middleware for development/testing
    ///
    /// This configuration allows all origins and is suitable for development
    /// and testing environments. **Do not use in production.**
    ///
    /// Configuration:
    /// - `allowed_origins`: `["*"]` (all origins)
    /// - `allowed_headers`: `["Content-Type", "Authorization"]`
    /// - `allowed_methods`: `GET, POST, PUT, DELETE, OPTIONS`
    /// - `allow_credentials`: `false` (cannot be true with wildcard)
    /// - `expose_headers`: `[]` (empty)
    /// - `max_age`: `None` (no preflight caching)
    ///
    /// # Example
    ///
    /// ```rust
    /// use brrtrouter::middleware::CorsMiddleware;
    ///
    /// let cors = CorsMiddleware::permissive();
    /// // Allows all origins - suitable for development only
    /// ```
    pub fn permissive() -> Self {
        Self {
            origin_validation: OriginValidation::Wildcard,
            allowed_headers: vec!["Content-Type".into(), "Authorization".into()],
            allowed_methods: vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ],
            allow_credentials: false, // Cannot be true with wildcard
            expose_headers: vec![],
            max_age: None,
            route_configs: std::collections::HashMap::new(),
        }
    }
}

/// Default CORS policy - secure by default
///
/// The default configuration is secure and requires explicit origin configuration.
/// For development/testing, use `CorsMiddleware::permissive()` instead.
impl Default for CorsMiddleware {
    /// Create a default CORS middleware (secure configuration)
    ///
    /// Default configuration:
    /// - `allowed_origins`: `[]` (empty - no origins allowed, requires explicit configuration)
    /// - `allowed_headers`: `["Content-Type", "Authorization"]`
    /// - `allowed_methods`: `GET, POST, PUT, DELETE, OPTIONS`
    /// - `allow_credentials`: `false`
    /// - `expose_headers`: `[]` (empty)
    /// - `max_age`: `None` (no preflight caching)
    ///
    /// # Security
    ///
    /// This default is secure - it allows no origins by default, requiring explicit
    /// configuration. For development/testing, use `CorsMiddleware::permissive()`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use brrtrouter::middleware::CorsMiddleware;
    ///
    /// // Secure default - no origins allowed
    /// let cors = CorsMiddleware::default();
    ///
    /// // For development, use permissive
    /// let cors_dev = CorsMiddleware::permissive();
    /// ```
    fn default() -> Self {
        Self {
            origin_validation: OriginValidation::Exact(vec![]), // Empty - secure by default
            allowed_headers: vec!["Content-Type".into(), "Authorization".into()],
            allowed_methods: vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ],
            allow_credentials: false,
            expose_headers: vec![],
            max_age: None,
            route_configs: std::collections::HashMap::new(),
        }
    }
}

impl Middleware for CorsMiddleware {
    /// Handle CORS preflight requests (OPTIONS)
    ///
    /// Validates the Origin header and requested method/headers, then returns
    /// a preflight response with appropriate CORS headers. Invalid preflight
    /// requests return 403 Forbidden.
    ///
    /// # Arguments
    ///
    /// * `req` - The incoming request
    ///
    /// # Returns
    ///
    /// - `Some(response)` - For OPTIONS requests (preflight with CORS headers or 403)
    /// - `None` - For all other requests (proceed to handler)
    fn before(&self, req: &HandlerRequest) -> Option<HandlerResponse> {
        // Handle preflight (OPTIONS) requests
        if req.method == Method::OPTIONS {
            // Extract and validate Origin header
            let origin = match req.get_header("origin") {
                Some(o) => o,
                None => {
                    // No Origin header - not a CORS request, but still handle OPTIONS
                    // Return 200 OK without CORS headers
                    return Some(HandlerResponse::new(200, HeaderVec::new(), Value::Null));
                }
            };

            // Get handler name for route-specific config lookup
            let handler_name = req.handler_name.as_str();

            // Validate origin (uses route-specific config if available)
            let validated_origin = match self.validate_origin(origin, handler_name) {
                Some(o) => o,
                None => {
                    warn!("CORS preflight: invalid origin '{}'", origin);
                    // Return 403 Forbidden for invalid origin (no CORS headers)
                    return Some(HandlerResponse::new(403, HeaderVec::new(), Value::Null));
                }
            };

            // Handle preflight validation
            match self.handle_preflight(req, &validated_origin) {
                Some(response) => Some(response),
                None => {
                    // Invalid preflight request (method or headers not allowed)
                    Some(HandlerResponse::new(403, HeaderVec::new(), Value::Null))
                }
            }
        } else {
            // Non-OPTIONS request - validate origin but don't short-circuit
            // We'll add CORS headers in after() if origin is valid
            if let Some(origin) = req.get_header("origin") {
                if self.validate_origin(origin, &req.handler_name).is_none() {
                    warn!("CORS: invalid origin '{}'", origin);
                    // Return 403 Forbidden for invalid origin
                    return Some(HandlerResponse::new(403, HeaderVec::new(), Value::Null));
                }
            }
            None
        }
    }

    /// Add CORS headers to the response after handler execution
    ///
    /// Called for all non-OPTIONS requests. Validates the Origin header and adds
    /// CORS headers only for valid cross-origin requests. Same-origin requests
    /// skip CORS headers.
    ///
    /// # Arguments
    ///
    /// * `req` - The original request (used to extract Origin header)
    /// * `res` - The response to modify (headers added in-place)
    /// * `_latency` - Request processing duration (unused)
    ///
    /// # Headers Added
    ///
    /// - `Access-Control-Allow-Origin`: Single validated origin (never comma-separated)
    /// - `Access-Control-Allow-Methods`: Comma-separated list of allowed methods
    /// - `Access-Control-Allow-Headers`: Comma-separated list of allowed headers
    /// - `Vary: Origin`: Indicates response varies based on Origin header
    fn after(&self, req: &HandlerRequest, res: &mut HandlerResponse, _latency: Duration) {
        // Extract Origin header
        let origin = match req.get_header("origin") {
            Some(o) => o,
            None => {
                // No Origin header - not a CORS request, skip CORS headers
                return;
            }
        };

        // Check if same-origin (skip CORS headers for same-origin requests)
        if self.is_same_origin(req, origin) {
            debug!("CORS: same-origin request, skipping CORS headers");
            return;
        }

        // Get route-specific config if available
        let (allowed_methods, allowed_headers, allow_credentials, expose_headers) =
            if let Some(route_config) = self.get_route_config(&req.handler_name) {
                (
                    &route_config.allowed_methods,
                    &route_config.allowed_headers,
                    route_config.allow_credentials,
                    &route_config.expose_headers,
                )
            } else {
                (
                    &self.allowed_methods,
                    &self.allowed_headers,
                    self.allow_credentials,
                    &self.expose_headers,
                )
            };

        // Validate origin (uses route-specific config if available)
        let validated_origin = match self.validate_origin(origin, &req.handler_name) {
            Some(o) => o,
            None => {
                // Invalid origin - should have been caught in before(), but log and skip
                warn!("CORS: invalid origin '{}' in after() - should have been caught in before()", origin);
                return;
            }
        };

        // Set CORS headers (only one origin per response - CORS spec requirement)
        res.set_header("access-control-allow-origin", validated_origin);

        // Set allowed methods
        let methods = allowed_methods
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        res.set_header("access-control-allow-methods", methods);

        // Set allowed headers
        let headers = allowed_headers.join(", ");
        res.set_header("access-control-allow-headers", headers);

        // Add credentials header if enabled
        if allow_credentials {
            res.set_header("access-control-allow-credentials", "true".to_string());
        }

        // Add exposed headers if configured
        if !expose_headers.is_empty() {
            let exposed = expose_headers.join(", ");
            res.set_header("access-control-expose-headers", exposed);
        }

        // Add Vary: Origin header for dynamic origin validation (RFC requirement)
        res.set_header("vary", "Origin".to_string());
    }
}

