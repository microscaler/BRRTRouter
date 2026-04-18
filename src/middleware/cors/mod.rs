//! Cross-origin resource sharing (CORS) for the HTTP pipeline.
//!
//! Provides preflight handling (`OPTIONS`), `Access-Control-*` response headers, and origin
//! validation (exact list, wildcard without credentials, regex, or custom predicate). OpenAPI
//! per-operation policy lives under the `x-cors` extension; see [`extract_route_cors_config`] and
//! [`merge_route_policies_with_global_origins`].
//!
//! # Primary types
//!
//! - [`CorsMiddleware`] — [`Middleware`](crate::middleware::Middleware) implementation.
//! - [`CorsMiddlewareBuilder`] — fluent builder; use [`CorsMiddlewareBuilder::build_with_routes`]
//!   when loading [`crate::spec::RouteMeta`] from an OpenAPI spec.
//! - [`OriginValidation`] — internal strategy for [`CorsMiddleware`] (exposed for advanced setups).
//!
//! # Metrics
//!
//! Call [`CorsMiddleware::with_metrics_sink`] with the same [`MetricsMiddleware`](crate::middleware::MetricsMiddleware)
//! `Arc` used on the dispatcher so CORS denials increment Prometheus counters (`brrtrouter_cors_*`)
//! exposed by [`metrics_endpoint`](crate::server::service::metrics_endpoint) on `GET /metrics`.
//!
//! # `Vary` merging
//!
//! [`CorsMiddleware::after`](CorsMiddleware::after) **merges** any existing handler `Vary` with CORS
//! tokens via [`merge_vary_field_value`](merge_vary_field_value). For gateways or non-CORS code paths,
//! call [`merge_vary_field_value`](merge_vary_field_value) yourself when combining values manually.
//!
//! # Documentation
//!
//! Deployment-focused notes (ingress/`Host`, RFC 7239 `Forwarded`, `X-Forwarded-*`, Private Network
//! Access, `Vary`, middleware ordering) live in `docs/CORS_OPERATIONS.md`. The architectural audit is
//! `docs/CORS_IMPLEMENTATION_AUDIT.md`.

mod builder;
mod error;
mod forwarded;
mod route_config;
mod vary_merge;

pub use builder::CorsMiddlewareBuilder;
pub use error::CorsConfigError;
pub use route_config::{
    build_route_cors_map, extract_route_cors_config, merge_route_policies_with_global_origins,
    RouteCorsConfig, RouteCorsPolicy,
};
pub use vary_merge::merge_vary_field_value;

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use http::Method;
use regex::Regex;
use tracing::{debug, warn};

use crate::dispatcher::{HandlerRequest, HandlerResponse, HeaderVec};
use crate::middleware::{MetricsMiddleware, Middleware};

/// First comma-separated value from `X-Forwarded-*` headers (typical reverse-proxy chains).
fn first_forwarded_token(s: &str) -> &str {
    s.split(',').next().unwrap_or("").trim()
}

/// Whether `host` already includes an explicit `:port` (handles bracketed IPv6).
fn host_has_explicit_port(host: &str) -> bool {
    if host.starts_with('[') {
        host.contains("]:")
    } else {
        host.rfind(':')
            .is_some_and(|i| host[i + 1..].parse::<u16>().is_ok())
    }
}

/// Effective `Host` authority (`host[:port]`) for same-origin checks.
///
/// When [`CorsMiddleware`] is configured with [`CorsMiddleware::with_trust_forwarded_host`],
/// uses (in order) RFC 7239 **`Forwarded`** (`host` / `proto`), then **`X-Forwarded-Host`** and
/// optional **`X-Forwarded-Port`**, when the request is from a trusted edge (configure only when
/// proxies strip or validate these headers).
fn effective_server_authority(req: &HandlerRequest, trust_forwarded: bool) -> Option<Cow<'_, str>> {
    if trust_forwarded {
        let forwarded_vals: Vec<&str> = req
            .headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("forwarded"))
            .map(|(_, v)| v.as_str())
            .collect();
        if let Some(auth) = forwarded::authority_from_forwarded_field_values(&forwarded_vals) {
            return Some(Cow::Owned(auth));
        }
        if let Some(fh_raw) = req.get_header("x-forwarded-host") {
            let host = first_forwarded_token(fh_raw);
            if !host.is_empty() {
                if !host_has_explicit_port(host) {
                    if let Some(fp_raw) = req.get_header("x-forwarded-port") {
                        let port = first_forwarded_token(fp_raw);
                        if port.parse::<u16>().is_ok() {
                            return Some(Cow::Owned(format!("{host}:{port}")));
                        }
                    }
                }
                return Some(Cow::Borrowed(host));
            }
        }
    }
    req.get_header("host").map(Cow::Borrowed)
}

/// Tokens CORS adds to `Vary` (stable order for [`vary_merge::merge_vary_field_value`]).
fn cors_vary_tokens(allow_private_network: bool) -> &'static [&'static str] {
    if allow_private_network {
        &["Origin", "Access-Control-Request-Private-Network"]
    } else {
        &["Origin"]
    }
}

/// CORS ([Cross-Origin Resource Sharing](https://fetch.spec.whatwg.org/#http-cors-protocol)) middleware.
///
/// Handles browser preflight (`OPTIONS` with `Origin` + `Access-Control-Request-Method`), adds
/// `Access-Control-*` headers on cross-origin responses, and can return **403** when the origin or
/// preflight negotiation fails (rejections are also logged via `tracing`).
///
/// OpenAPI route overrides are keyed by handler name; use [`merge_route_policies_with_global_origins`]
/// or [`CorsMiddlewareBuilder::build_with_routes`] so `x-cors: { ... }` routes receive deployment
/// origins (the spec object never lists origins).
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
/// How allowed [`Origin`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Origin) values are matched.
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
            OriginValidation::Regex(patterns) => f
                .debug_tuple("Regex")
                .field(&patterns.iter().map(|re| re.as_str()).collect::<Vec<_>>())
                .finish(),
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

/// Internal: classify OPTIONS after `Origin` is allowed — preflight success, not a preflight, or denied.
enum CorsPreflightOutcome {
    Success(HandlerResponse),
    /// No `Access-Control-Request-Method` — not a CORS preflight; let the route handler run.
    NotPreflight,
    /// Preflight attempted but policy rejects method/headers — respond 403 without CORS success headers.
    Denied,
}

pub struct CorsMiddleware {
    pub(crate) origin_validation: OriginValidation,
    pub(crate) allowed_headers: Vec<String>,
    pub(crate) allowed_methods: Vec<Method>,
    pub(crate) allow_credentials: bool,
    pub(crate) expose_headers: Vec<String>,
    pub(crate) max_age: Option<u32>,
    /// Route-specific CORS policies keyed by handler name
    /// If a route has an `x-cors` extension in OpenAPI, it determines CORS behavior:
    /// - `Inherit`: Use global CORS configuration (not stored, default behavior)
    /// - `Disabled`: Disable CORS for this route (no CORS headers)
    /// - `Custom(config)`: Use route-specific CORS configuration
    pub(crate) route_policies: std::collections::HashMap<String, RouteCorsPolicy>,
    /// When set, CORS denials increment [`MetricsMiddleware`] counters for `/metrics`.
    pub(crate) metrics_sink: Option<Arc<MetricsMiddleware>>,
    /// Use [`effective_server_authority`] (`X-Forwarded-Host` / `X-Forwarded-Port`) for same-origin detection.
    pub(crate) trust_forwarded_host: bool,
    /// [Private Network Access](https://wicg.github.io/private-network-access/): emit
    /// `Access-Control-Allow-Private-Network` when enabled and the request participates in PNA.
    pub(crate) allow_private_network_access: bool,
}

impl CorsMiddleware {
    /// Global origin validation for this middleware (exact list, wildcard, regex, or custom).
    #[must_use]
    pub fn global_origin_validation(&self) -> &OriginValidation {
        &self.origin_validation
    }

    /// Link this middleware to [`MetricsMiddleware`] so CORS events update Prometheus counters
    /// (`brrtrouter_cors_origin_rejections_total`, `brrtrouter_cors_preflight_denials_total`,
    /// `brrtrouter_cors_route_disabled_total`) on
    /// [`GET /metrics`](crate::server::service::metrics_endpoint). Pass the **same** `Arc` used when
    /// registering [`MetricsMiddleware`] on the [`Dispatcher`](crate::dispatcher::Dispatcher).
    #[must_use]
    pub fn with_metrics_sink(mut self, m: Arc<MetricsMiddleware>) -> Self {
        self.metrics_sink = Some(m);
        self
    }

    /// When `true`, same-origin detection uses [`effective_server_authority`]: RFC 7239 **`Forwarded`**
    /// (`host` / `proto`), then **`X-Forwarded-Host`** / **`X-Forwarded-Port`**, so Envoy/nginx can
    /// match browser `Origin` to the public authority.
    ///
    /// **Security:** enable only when your edge overwrites or strips these headers from untrusted clients.
    #[must_use]
    pub fn with_trust_forwarded_host(mut self, trust: bool) -> Self {
        self.trust_forwarded_host = trust;
        self
    }

    /// Enable [Private Network Access](https://wicg.github.io/private-network-access/) response headers
    /// (`Access-Control-Allow-Private-Network: true`) for eligible preflight and cross-origin responses.
    #[must_use]
    pub fn with_allow_private_network_access(mut self, allow: bool) -> Self {
        self.allow_private_network_access = allow;
        self
    }

    fn record_cors_origin_rejection(&self) {
        if let Some(m) = &self.metrics_sink {
            m.inc_cors_origin_rejection();
        }
    }

    fn record_cors_preflight_denial(&self) {
        if let Some(m) = &self.metrics_sink {
            m.inc_cors_preflight_denial();
        }
    }

    fn record_cors_route_disabled(&self) {
        if let Some(m) = &self.metrics_sink {
            m.inc_cors_route_disabled();
        }
    }

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
    ///
    /// JSF Compliance: Panics only during initialization, never on hot path
    /// This method is only called during startup in templates/main.rs.txt
    #[allow(clippy::panic)]
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
        // This panic is intentional: invalid configuration should fail fast at startup
        if allow_credentials && origin_validation.is_wildcard() {
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
            route_policies: std::collections::HashMap::new(),
            metrics_sink: None,
            trust_forwarded_host: false,
            allow_private_network_access: false,
        }
    }

    /// Create a route-aware CORS middleware with OpenAPI route policies
    ///
    /// This constructor allows you to provide route-specific CORS policies
    /// extracted from OpenAPI `x-cors` extensions. Routes with `x-cors` will
    /// use their specific policy, others will use the global config.
    ///
    /// # Arguments
    ///
    /// * `global_config` - Global CORS configuration (used as fallback)
    /// * `route_policies` - Map of handler names to route-specific CORS policies
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use brrtrouter::middleware::{CorsMiddleware, RouteCorsPolicy};
    /// use brrtrouter::spec::load_spec;
    /// use std::collections::HashMap;
    ///
    /// let (routes, _) = load_spec("openapi.yaml")?;
    /// let route_policies = build_route_cors_map(&routes);
    ///
    /// let global_cors = CorsMiddlewareBuilder::new()
    ///     .allowed_origins(&["https://example.com"])
    ///     .build()?;
    ///
    /// let cors = CorsMiddleware::with_route_policies(global_cors, route_policies);
    /// ```
    pub fn with_route_policies(
        global_config: CorsMiddleware,
        route_policies: std::collections::HashMap<String, RouteCorsPolicy>,
    ) -> Self {
        Self {
            origin_validation: global_config.origin_validation,
            allowed_headers: global_config.allowed_headers,
            allowed_methods: global_config.allowed_methods,
            allow_credentials: global_config.allow_credentials,
            expose_headers: global_config.expose_headers,
            max_age: global_config.max_age,
            route_policies,
            metrics_sink: global_config.metrics_sink,
            trust_forwarded_host: global_config.trust_forwarded_host,
            allow_private_network_access: global_config.allow_private_network_access,
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
    ///
    /// JSF Compliance: Panics only during initialization, never on hot path
    /// This method is only called during startup in templates/main.rs.txt
    #[allow(clippy::panic)]
    pub fn with_regex_patterns(
        origin_patterns: Vec<String>,
        allowed_headers: Vec<String>,
        allowed_methods: Vec<Method>,
        allow_credentials: bool,
        expose_headers: Vec<String>,
        max_age: Option<u32>,
    ) -> Self {
        // Compile regex patterns
        let patterns: Result<Vec<Regex>, _> =
            origin_patterns.iter().map(|p| Regex::new(p)).collect();

        // This panic is intentional: invalid configuration should fail fast at startup
        let patterns = patterns.unwrap_or_else(|e| {
            panic!("CORS configuration error: Invalid regex pattern: {}", e);
        });

        let origin_validation = OriginValidation::Regex(patterns);

        // Validate: cannot use wildcard with credentials
        // This panic is intentional: invalid configuration should fail fast at startup
        if allow_credentials && origin_validation.is_wildcard() {
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
            route_policies: std::collections::HashMap::new(),
            metrics_sink: None,
            trust_forwarded_host: false,
            allow_private_network_access: false,
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
            route_policies: std::collections::HashMap::new(),
            metrics_sink: None,
            trust_forwarded_host: false,
            allow_private_network_access: false,
        }
    }

    /// Get route-specific CORS policy for a handler
    ///
    /// # Arguments
    ///
    /// * `handler_name` - The handler name to look up
    ///
    /// # Returns
    ///
    /// Route-specific policy if found, otherwise `Inherit` (use global config)
    fn get_route_policy(&self, handler_name: &str) -> RouteCorsPolicy {
        self.route_policies
            .get(handler_name)
            .cloned()
            .unwrap_or(RouteCorsPolicy::Inherit)
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
            false,  // no credentials by default
            vec![], // no exposed headers
            None,   // no preflight caching
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
    /// * `None` - If origin is not allowed or CORS is disabled for this route
    fn validate_origin(&self, origin: &str, handler_name: &str) -> Option<String> {
        // Check route-specific policy first
        let policy = self.get_route_policy(handler_name);
        let validation = match policy {
            RouteCorsPolicy::Disabled => {
                // CORS is disabled for this route - return None to prevent CORS headers
                return None;
            }
            RouteCorsPolicy::Inherit => {
                // Use global config
                &self.origin_validation
            }
            RouteCorsPolicy::Custom(ref route_config) => {
                // Use route-specific config (use ref to avoid move)
                &route_config.origin_validation
            }
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
    ///
    /// # Port Handling
    ///
    /// This function properly handles port comparison:
    /// - Default ports: 80 for http, 443 for https
    /// - Host header without port uses default port based on Origin scheme
    /// - Both hostname and port must match for same-origin
    fn is_same_origin(&self, req: &HandlerRequest, origin: &str) -> bool {
        let host_header = match effective_server_authority(req, self.trust_forwarded_host) {
            Some(c) => c,
            None => return false, // No Host / forwarded authority, assume cross-origin
        };
        let host_header: &str = host_header.as_ref();

        // Parse origin to extract scheme, hostname, and port
        // Origin format: scheme://hostname:port or scheme://hostname
        let origin_parts: Vec<&str> = origin.split("://").collect();
        if origin_parts.len() != 2 {
            return false; // Invalid origin format
        }

        let origin_scheme = origin_parts[0];
        let origin_authority = origin_parts[1];

        // Determine default port based on scheme
        let default_port = match origin_scheme {
            "https" => 443,
            "http" => 80,
            _ => return false, // Unknown scheme
        };

        // Extract hostname and port from origin
        // Handle IPv6 addresses: [::1]:8080 format
        let (origin_hostname, origin_port) = if origin_authority.starts_with('[') {
            // IPv6 address: look for ]: as port delimiter
            if let Some(close_bracket) = origin_authority.find(']') {
                let hostname = &origin_authority[..=close_bracket];
                if let Some(port_start) = origin_authority[close_bracket + 1..].find(':') {
                    let port_str = &origin_authority[close_bracket + 1 + port_start + 1..];
                    // Parse port, treating parse failures as distinct from valid port 0
                    let port = port_str.parse::<u16>().ok();
                    (hostname, port)
                } else {
                    (hostname, None)
                }
            } else {
                // Malformed IPv6 (no closing bracket) - treat as invalid
                return false;
            }
        } else if let Some(colon_pos) = origin_authority.find(':') {
            // IPv4 or hostname:port format
            let hostname = &origin_authority[..colon_pos];
            let port_str = &origin_authority[colon_pos + 1..];
            // Parse port, treating parse failures as distinct from valid port 0
            let port = port_str.parse::<u16>().ok();
            (hostname, port)
        } else {
            (origin_authority, None)
        };

        // Parse Host header: hostname:port or hostname
        // Handle IPv6 addresses: [::1]:8080 format
        let (host_hostname, host_port) = if host_header.starts_with('[') {
            // IPv6 address: look for ]: as port delimiter
            if let Some(close_bracket) = host_header.find(']') {
                let hostname = &host_header[..=close_bracket];
                if let Some(port_start) = host_header[close_bracket + 1..].find(':') {
                    let port_str = &host_header[close_bracket + 1 + port_start + 1..];
                    // Parse port, treating parse failures as distinct from valid port 0
                    let port = port_str.parse::<u16>().ok();
                    (hostname, port)
                } else {
                    (hostname, None)
                }
            } else {
                // Malformed IPv6 (no closing bracket) - treat as invalid
                return false;
            }
        } else if let Some(colon_pos) = host_header.find(':') {
            // IPv4 or hostname:port format
            let hostname = &host_header[..colon_pos];
            let port_str = &host_header[colon_pos + 1..];
            // Parse port, treating parse failures as distinct from valid port 0
            let port = port_str.parse::<u16>().ok();
            (hostname, port)
        } else {
            (host_header, None)
        };

        // Compare hostnames first (case-insensitive per RFC)
        if !host_hostname.eq_ignore_ascii_case(origin_hostname) {
            return false; // Different hostnames = different origins
        }

        // Compare ports with proper handling of explicit vs implicit ports
        // Per browser same-origin policy: ports must match exactly
        // - If Origin has explicit port and Host has no port: only match if Origin port is default
        // - If Origin has no port and Host has no port: match (both use default)
        // - If Origin has no port and Host has explicit port: only match if Host port is default
        // - If both have ports: compare directly
        match (origin_port, host_port) {
            (Some(origin_p), Some(host_p)) => {
                // Both have explicit ports - must match exactly
                origin_p == host_p
            }
            (Some(origin_p), None) => {
                // Origin has explicit port, Host has no port
                // Only match if Origin port is the default port
                origin_p == default_port
            }
            (None, Some(host_p)) => {
                // Origin has no port, Host has explicit port
                // Only match if Host port is the default port
                host_p == default_port
            }
            (None, None) => {
                // Neither has explicit port - both use default, so they match
                true
            }
        }
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
    /// * [`CorsPreflightOutcome::Success`] — Valid preflight with CORS headers
    /// * [`CorsPreflightOutcome::NotPreflight`] — Missing `Access-Control-Request-Method` (regular OPTIONS)
    /// * [`CorsPreflightOutcome::Denied`] — Preflight attempted but method/headers invalid or disallowed
    fn handle_preflight(&self, req: &HandlerRequest, origin: &str) -> CorsPreflightOutcome {
        // Get route-specific policy first
        let policy = self.get_route_policy(&req.handler_name);
        let (allowed_methods, allowed_headers, allow_credentials, max_age) = match policy {
            RouteCorsPolicy::Disabled => {
                // CORS is disabled — caller should have short-circuited; treat as non-preflight.
                return CorsPreflightOutcome::NotPreflight;
            }
            RouteCorsPolicy::Inherit => {
                // Use global config
                (
                    &self.allowed_methods,
                    &self.allowed_headers,
                    self.allow_credentials,
                    self.max_age,
                )
            }
            RouteCorsPolicy::Custom(ref route_config) => {
                // Use route-specific config (use ref to avoid move)
                (
                    &route_config.allowed_methods,
                    &route_config.allowed_headers,
                    route_config.allow_credentials,
                    route_config.max_age,
                )
            }
        };
        // Missing Access-Control-Request-Method => not a CORS preflight (regular OPTIONS).
        let acrm = match req.get_header("access-control-request-method") {
            Some(m) => m,
            None => return CorsPreflightOutcome::NotPreflight,
        };

        let requested_method = match acrm.parse::<Method>() {
            Ok(m) => m,
            Err(_) => {
                warn!(
                    "CORS preflight: invalid Access-Control-Request-Method: {}",
                    acrm
                );
                return CorsPreflightOutcome::Denied;
            }
        };

        // Validate method
        if !allowed_methods.contains(&requested_method) {
            warn!(
                "CORS preflight: method {} not in allowed methods",
                requested_method.as_str()
            );
            return CorsPreflightOutcome::Denied;
        }

        // Extract and validate requested headers
        let requested_headers = req.get_header("access-control-request-headers");
        if let Some(headers_str) = requested_headers {
            let requested_headers_list: Vec<&str> =
                headers_str.split(',').map(|h| h.trim()).collect();

            // Check if all requested headers are allowed
            // If allowed_headers contains "*", allow all
            let allow_all_headers = allowed_headers.iter().any(|h| h == "*");
            if !allow_all_headers {
                for header in &requested_headers_list {
                    if !allowed_headers
                        .iter()
                        .any(|h| h.eq_ignore_ascii_case(header))
                    {
                        warn!("CORS preflight: header '{}' not in allowed headers", header);
                        return CorsPreflightOutcome::Denied;
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

        let pna_requested = req
            .get_header("access-control-request-private-network")
            .is_some_and(|v| v.is_empty() || v.eq_ignore_ascii_case("true"));
        if self.allow_private_network_access && pna_requested {
            headers.push((
                std::sync::Arc::from("access-control-allow-private-network"),
                "true".to_string(),
            ));
        }

        // Vary for caches — merge with any prior tokens (preflight responses are usually fresh)
        let vary_merged = vary_merge::merge_vary_field_value(
            None,
            cors_vary_tokens(self.allow_private_network_access),
        );
        headers.push((std::sync::Arc::from("vary"), vary_merged));

        // Empty JSON object — not `null` — so OpenAPI response validation (`type: object`) passes
        // when middleware short-circuits OPTIONS (e.g. `options_user` documents `200` + JSON schema).
        CorsPreflightOutcome::Success(HandlerResponse::new(200, headers, serde_json::json!({})))
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
            route_policies: std::collections::HashMap::new(),
            metrics_sink: None,
            trust_forwarded_host: false,
            allow_private_network_access: false,
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
            route_policies: std::collections::HashMap::new(),
            metrics_sink: None,
            trust_forwarded_host: false,
            allow_private_network_access: false,
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
        // Check if CORS is disabled for this route
        if matches!(
            self.get_route_policy(&req.handler_name),
            RouteCorsPolicy::Disabled
        ) {
            self.record_cors_route_disabled();
            // CORS is disabled - for OPTIONS requests, return 200 OK without CORS headers
            if req.method == Method::OPTIONS {
                return Some(HandlerResponse::new(
                    200,
                    HeaderVec::new(),
                    serde_json::json!({}),
                ));
            }
            // For non-OPTIONS requests, proceed normally (no CORS headers will be added in after())
            return None;
        }

        // Handle preflight (OPTIONS) requests
        if req.method == Method::OPTIONS {
            // Extract and validate Origin header
            let origin = match req.get_header("origin") {
                Some(o) => o,
                None => {
                    // No Origin header - not a CORS request, but still handle OPTIONS
                    // Return 200 OK without CORS headers
                    return Some(HandlerResponse::new(
                        200,
                        HeaderVec::new(),
                        serde_json::json!({}),
                    ));
                }
            };

            // Get handler name for route-specific config lookup
            let handler_name = req.handler_name.as_str();

            // Validate origin (uses route-specific config if available)
            let validated_origin = match self.validate_origin(origin, handler_name) {
                Some(o) => o,
                None => {
                    warn!("CORS preflight: invalid origin '{}'", origin);
                    self.record_cors_origin_rejection();
                    // Return 403 Forbidden for invalid origin (no CORS headers)
                    return Some(HandlerResponse::error(
                        403,
                        "Origin not allowed by CORS policy",
                    ));
                }
            };

            // Handle preflight validation
            // BUG FIX: Distinguish between "not a preflight" (None) and "invalid preflight" (Some(403))
            // Per CORS spec: Missing Access-Control-Request-Method means it's a regular OPTIONS request,
            // not a preflight. Regular OPTIONS requests should proceed to handler or return 200/204.
            match self.handle_preflight(req, &validated_origin) {
                CorsPreflightOutcome::Success(response) => Some(response),
                CorsPreflightOutcome::NotPreflight => None, // Regular OPTIONS — handler may respond
                CorsPreflightOutcome::Denied => {
                    self.record_cors_preflight_denial();
                    Some(HandlerResponse::error(403, "CORS preflight request denied"))
                }
            }
        } else {
            // Non-OPTIONS request - validate origin but don't short-circuit
            // We'll add CORS headers in after() if origin is valid
            if let Some(origin) = req.get_header("origin") {
                if self.validate_origin(origin, &req.handler_name).is_none() {
                    warn!("CORS: invalid origin '{}'", origin);
                    self.record_cors_origin_rejection();
                    // Return 403 Forbidden for invalid origin
                    return Some(HandlerResponse::error(
                        403,
                        "Origin not allowed by CORS policy",
                    ));
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

        // Get route-specific policy first
        let policy = self.get_route_policy(&req.handler_name);

        // Check if CORS is disabled for this route
        if matches!(policy, RouteCorsPolicy::Disabled) {
            debug!(
                "CORS: disabled for route '{}', skipping CORS headers",
                req.handler_name
            );
            return;
        }

        // Get route-specific config if available
        // Extract config separately to avoid borrowing from temporary
        let (allowed_methods, allowed_headers, allow_credentials, expose_headers) =
            if let RouteCorsPolicy::Custom(ref route_config) = policy {
                // Use route-specific config
                (
                    &route_config.allowed_methods,
                    &route_config.allowed_headers,
                    route_config.allow_credentials,
                    &route_config.expose_headers,
                )
            } else {
                // Use global config (Inherit case)
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
                warn!(
                    "CORS: invalid origin '{}' in after() - should have been caught in before()",
                    origin
                );
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

        if self.allow_private_network_access {
            res.set_header("access-control-allow-private-network", "true".to_string());
        }

        let merged = vary_merge::merge_vary_field_value(
            res.get_header("vary"),
            cors_vary_tokens(self.allow_private_network_access),
        );
        res.set_header("vary", merged);
    }
}

#[cfg(test)]
mod cors_middleware_tests {
    use super::*;
    use crate::ids::RequestId;
    use crate::router::ParamVec;
    use may::sync::mpsc;
    use std::sync::Arc;

    fn test_request(
        method: Method,
        path: &str,
        handler_name: &str,
        headers: Vec<(&str, &str)>,
    ) -> HandlerRequest {
        let (__reply_raw, _) = mpsc::channel();
        let reply_tx = crate::dispatcher::HandlerReplySender::channel(__reply_raw);
        let mut hv = HeaderVec::new();
        for (k, v) in headers {
            hv.push((Arc::from(k), v.to_string()));
        }
        HandlerRequest {
            request_id: RequestId::new(),
            method,
            path: path.to_string(),
            handler_name: handler_name.to_string(),
            path_params: ParamVec::new(),
            query_params: ParamVec::new(),
            headers: hv,
            cookies: HeaderVec::new(),
            body: None,
            jwt_claims: None,
            reply_tx,
            queue_guard: None,
        }
    }

    fn cors_localhost_only() -> CorsMiddleware {
        CorsMiddleware::new(
            vec!["http://localhost:3000".to_string()],
            vec!["Content-Type".to_string()],
            vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ],
            false,
            vec![],
            None,
        )
    }

    /// Regression: browser POST with `application/x-www-form-urlencoded` and a disallowed
    /// `Origin` must short-circuit with JSON `{"error":...}`, not bare `null`, and must not use
    /// the default "OK" reason phrase for 403.
    #[test]
    fn post_form_urlencoded_invalid_origin_403_json_error_not_null() {
        let cors = cors_localhost_only();
        let req = test_request(
            Method::POST,
            "/form",
            "submit_form",
            vec![
                ("Origin", "https://evil.example"),
                ("Content-Type", "application/x-www-form-urlencoded"),
            ],
        );
        let early = cors
            .before(&req)
            .expect("invalid origin should short-circuit");
        assert_eq!(early.status, 403);
        assert!(
            !early.body.is_null(),
            "403 CORS rejection must not use Value::Null body (UI showed 4-byte null)"
        );
        assert_eq!(
            early.body.get("error").and_then(|v| v.as_str()),
            Some("Origin not allowed by CORS policy")
        );
    }

    #[test]
    fn options_preflight_invalid_origin_403_json_error_not_null() {
        let cors = cors_localhost_only();
        let req = test_request(
            Method::OPTIONS,
            "/form",
            "submit_form",
            vec![
                ("Origin", "https://evil.example"),
                ("Access-Control-Request-Method", "POST"),
            ],
        );
        let early = cors
            .before(&req)
            .expect("invalid origin should short-circuit");
        assert_eq!(early.status, 403);
        assert!(!early.body.is_null());
        assert_eq!(
            early.body.get("error").and_then(|v| v.as_str()),
            Some("Origin not allowed by CORS policy")
        );
    }

    #[test]
    fn post_form_urlencoded_allowed_origin_does_not_short_circuit() {
        let cors = cors_localhost_only();
        let req = test_request(
            Method::POST,
            "/form",
            "submit_form",
            vec![
                ("Origin", "http://localhost:3000"),
                ("Content-Type", "application/x-www-form-urlencoded"),
            ],
        );
        assert!(cors.before(&req).is_none());
    }

    /// Pet store `POST /webhooks` (`register_webhook`) from the API explorer with a bad `Origin`
    /// must return the same JSON error shape as other CORS rejections — not `null` / `403 OK`.
    #[test]
    fn post_json_webhooks_invalid_origin_403_json_error_not_null() {
        let cors = cors_localhost_only();
        let req = test_request(
            Method::POST,
            "/webhooks",
            "register_webhook",
            vec![
                ("Origin", "https://evil.example"),
                ("Content-Type", "application/json"),
            ],
        );
        let early = cors
            .before(&req)
            .expect("invalid origin should short-circuit");
        assert_eq!(early.status, 403);
        assert!(!early.body.is_null());
        assert_eq!(
            early.body.get("error").and_then(|v| v.as_str()),
            Some("Origin not allowed by CORS policy")
        );
        assert!(
            early.get_header("content-type").is_some(),
            "HandlerResponse::json must set Content-Type for API clients"
        );
    }
}
