use std::time::Duration;

use http::Method;
use serde_json::Value;
use tracing::{debug, warn};

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse, HeaderVec};

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
pub struct CorsMiddleware {
    allowed_origins: Vec<String>,
    allowed_headers: Vec<String>,
    allowed_methods: Vec<Method>,
    allow_credentials: bool,
    expose_headers: Vec<String>,
    max_age: Option<u32>,
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
        // Validate: cannot use wildcard with credentials (CORS spec requirement)
        if allow_credentials && allowed_origins.iter().any(|o| o == "*") {
            panic!(
                "CORS configuration error: Cannot use wildcard origin (*) with credentials. \
                When allow_credentials is true, you must specify exact origins."
            );
        }

        Self {
            allowed_origins,
            allowed_headers,
            allowed_methods,
            allow_credentials,
            expose_headers,
            max_age,
        }
    }

    /// Create a new CORS middleware with legacy configuration (backward compatibility)
    ///
    /// This method maintains backward compatibility with the old API.
    /// For new code, prefer using `CorsMiddleware::new()` with explicit credential/expose/max_age parameters,
    /// or use `CorsMiddlewareBuilder` for a more ergonomic API.
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
    /// Handles wildcard "*" origin (allows all origins).
    ///
    /// # Arguments
    ///
    /// * `origin` - The Origin header value from the request
    ///
    /// # Returns
    ///
    /// * `Some(origin)` - If origin is allowed (returns the origin string to use in headers)
    /// * `None` - If origin is not allowed
    fn validate_origin(&self, origin: &str) -> Option<String> {
        // Check for wildcard "*" in allowed origins
        if self.allowed_origins.iter().any(|o| o == "*") {
            return Some("*".to_string());
        }

        // Exact match against allowed origins
        if self.allowed_origins.iter().any(|o| o == origin) {
            return Some(origin.to_string());
        }

        None
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
        if !self.allowed_methods.contains(&requested_method) {
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
            let allow_all_headers = self.allowed_headers.iter().any(|h| h == "*");
            if !allow_all_headers {
                for header in &requested_headers_list {
                    if !self
                        .allowed_headers
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
            self.allowed_methods
                .iter()
                .map(|m| m.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        ));
        headers.push((
            std::sync::Arc::from("access-control-allow-headers"),
            self.allowed_headers.join(", "),
        ));

        // Add credentials header if enabled
        if self.allow_credentials {
            headers.push((
                std::sync::Arc::from("access-control-allow-credentials"),
                "true".to_string(),
            ));
        }

        // Add preflight cache duration if configured
        if let Some(age) = self.max_age {
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
            allowed_origins: vec![], // Empty - secure by default
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
        }
    }
}

impl CorsMiddleware {
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
            allowed_origins: vec!["*".into()],
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
        }
    }
}

/// CORS middleware implementation
///
/// Handles CORS preflight (OPTIONS) requests and adds CORS headers to all responses.
/// This ensures browsers can make cross-origin requests according to the configured policy.
///
/// # CORS Flow
///
/// 1. **Preflight (OPTIONS)**: Returns 200 with CORS headers, no handler invoked
/// 2. **Actual Request**: Handler executes, CORS headers added to response in `after()`
///
/// # Security
///
/// - Default config allows all origins (`*`) - restrict in production
/// - Validates Origin header against `allowed_origins`
/// - Exposes CORS headers via `Access-Control-Expose-Headers`
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

            // Validate origin
            let validated_origin = match self.validate_origin(origin) {
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
                if self.validate_origin(origin).is_none() {
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

        // Validate origin
        let validated_origin = match self.validate_origin(origin) {
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
        let methods = self
            .allowed_methods
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        res.set_header("access-control-allow-methods", methods);

        // Set allowed headers
        let headers = self.allowed_headers.join(", ");
        res.set_header("access-control-allow-headers", headers);

        // Add credentials header if enabled
        if self.allow_credentials {
            res.set_header("access-control-allow-credentials", "true".to_string());
        }

        // Add exposed headers if configured
        if !self.expose_headers.is_empty() {
            let exposed = self.expose_headers.join(", ");
            res.set_header("access-control-expose-headers", exposed);
        }

        // Add Vary: Origin header for dynamic origin validation (RFC requirement)
        res.set_header("vary", "Origin".to_string());
    }
}
