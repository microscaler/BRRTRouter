use oas3::spec::Operation;
use std::collections::HashMap;

use super::OriginValidation;
use crate::spec::RouteMeta;

/// Route-specific CORS policy extracted from OpenAPI `x-cors` extension
///
/// Determines how CORS should be handled for a specific route:
/// - `Inherit`: Use global CORS configuration (default)
/// - `Disabled`: Disable CORS for this route (no CORS headers)
/// - `Custom`: Use route-specific CORS configuration
#[derive(Debug, Clone)]
pub enum RouteCorsPolicy {
    /// Use global CORS configuration
    Inherit,
    /// Disable CORS for this route (no CORS headers will be added)
    Disabled,
    /// Use route-specific CORS configuration
    Custom(RouteCorsConfig),
}

/// Route-specific CORS configuration extracted from OpenAPI `x-cors` extension
///
/// This configuration can override the global CORS middleware settings
/// for specific routes defined in the OpenAPI specification.
///
/// **Note**: Origins are NOT included here - they come from config.yaml
/// and are merged during middleware initialization.
#[derive(Debug, Clone)]
pub struct RouteCorsConfig {
    /// Origin validation strategy for this route (set from config.yaml during initialization)
    pub(crate) origin_validation: OriginValidation,
    /// Allowed headers for this route
    pub allowed_headers: Vec<String>,
    /// Allowed HTTP methods for this route
    pub allowed_methods: Vec<http::Method>,
    /// Whether credentials are allowed
    pub allow_credentials: bool,
    /// Headers to expose to JavaScript
    pub expose_headers: Vec<String>,
    /// Preflight cache duration in seconds
    pub max_age: Option<u32>,
}

impl RouteCorsConfig {
    /// Create a default route CORS config (inherits from global)
    /// Origins will be set from config.yaml during middleware initialization
    pub fn default() -> Self {
        Self {
            origin_validation: OriginValidation::Exact(vec![]), // Will be set from config.yaml
            allowed_headers: vec!["Content-Type".into(), "Authorization".into()],
            allowed_methods: vec![
                http::Method::GET,
                http::Method::POST,
                http::Method::PUT,
                http::Method::DELETE,
                http::Method::OPTIONS,
            ],
            allow_credentials: false,
            expose_headers: vec![],
            max_age: None,
        }
    }

    /// Create a route CORS config with origins from config.yaml
    /// This is called during middleware initialization to merge config.yaml origins
    ///
    /// # Panics
    ///
    /// Panics if `allow_credentials` is `true` and:
    /// - `origins` contains `"*"` (wildcard with credentials is invalid per CORS spec)
    /// - `origins` is empty (empty origins with credentials is invalid - no origins can be validated)
    ///
    /// JSF Compliance: Panics only during initialization, never on hot path
    /// This method is only called during startup in templates/main.rs.txt
    #[allow(clippy::panic)]
    pub fn with_origins(mut self, origins: &[&str]) -> Self {
        // BUG FIX: Validate empty origins with credentials (same validation as CorsMiddlewareBuilder)
        if self.allow_credentials && origins.is_empty() {
            panic!(
                "CORS configuration error: Cannot use empty origins with allowCredentials: true. \
                Route has allowCredentials enabled but config.yaml has no origins configured. \
                When credentials are enabled, you must specify at least one origin in config.yaml."
            );
        }

        if origins.contains(&"*") {
            // Check if credentials are enabled - this combination is invalid per CORS spec
            if self.allow_credentials {
                panic!(
                    "CORS configuration error: Cannot use wildcard origin (*) with allowCredentials: true. \
                    Route has allowCredentials enabled but global CORS config has wildcard origins. \
                    When credentials are enabled, you must specify exact origins in config.yaml."
                );
            }
            self.origin_validation = OriginValidation::Wildcard;
        } else {
            let origins_vec: Vec<String> = origins.iter().map(|s| s.to_string()).collect();
            self.origin_validation = OriginValidation::Exact(origins_vec);
        }
        self
    }
}

/// Extract CORS policy from OpenAPI `x-cors` extension
///
/// Supports multiple formats:
/// - Object: `x-cors: { allowedHeaders: [...], allowCredentials: true }` → `Custom(config)`
/// - String: `x-cors: "inherit"` → `Inherit` (uses global config)
/// - Boolean: `x-cors: false` → `Disabled` (no CORS headers)
/// - Missing/None: → `Inherit` (uses global config)
///
/// # Arguments
///
/// * `operation` - The OpenAPI operation definition
///
/// # Returns
///
/// * `RouteCorsPolicy::Inherit` - If extension is not present or is `"inherit"` (use global config)
/// * `RouteCorsPolicy::Disabled` - If extension is `false` (disable CORS for this route)
/// * `RouteCorsPolicy::Custom(config)` - If extension is an object (use route-specific config)
pub fn extract_route_cors_config(operation: &Operation) -> RouteCorsPolicy {
    let cors_ext = match operation.extensions.get("x-cors") {
        Some(ext) => ext,
        None => return RouteCorsPolicy::Inherit, // No extension = inherit global config
    };

    // Handle boolean false - disable CORS for this route
    if let Some(false) = cors_ext.as_bool() {
        return RouteCorsPolicy::Disabled;
    }

    // Handle string "inherit" - use global config
    if let Some("inherit") = cors_ext.as_str() {
        return RouteCorsPolicy::Inherit;
    }

    // Handle object configuration
    if let Some(obj) = cors_ext.as_object() {
        let mut config = RouteCorsConfig::default();

        // Note: Origins are NOT extracted from OpenAPI x-cors extension.
        // Origins should be configured in config.yaml (environment-specific).
        // The x-cors extension can only override other CORS settings (methods, headers, credentials, etc.).

        // Extract allowed headers
        if let Some(headers_val) = obj.get("allowedHeaders") {
            if let Some(headers_array) = headers_val.as_array() {
                config.allowed_headers = headers_array
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
            }
        }

        // Extract allowed methods
        if let Some(methods_val) = obj.get("allowedMethods") {
            if let Some(methods_array) = methods_val.as_array() {
                config.allowed_methods = methods_array
                    .iter()
                    .filter_map(|v| v.as_str().and_then(|s| s.parse::<http::Method>().ok()))
                    .collect();
            }
        }

        // Extract credentials
        if let Some(creds) = obj.get("allowCredentials").and_then(|v| v.as_bool()) {
            config.allow_credentials = creds;
        }

        // Extract expose headers
        if let Some(expose_val) = obj.get("exposeHeaders") {
            if let Some(expose_array) = expose_val.as_array() {
                config.expose_headers = expose_array
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
            }
        }

        // Extract max age
        if let Some(age) = obj.get("maxAge").and_then(|v| v.as_u64()) {
            config.max_age = Some(age as u32);
        }

        // Note: Origin validation is not set here - origins come from config.yaml
        // and will be merged during middleware initialization. We only extract other settings.

        return RouteCorsPolicy::Custom(config);
    }

    // Default to inherit if extension is present but not recognized
    RouteCorsPolicy::Inherit
}

/// Build a map of route-specific CORS policies from route metadata
///
/// Creates a lookup map keyed by handler name for efficient route-specific
/// CORS handling. Routes with `cors_policy` set will use their specific policy:
/// - `Inherit`: Falls back to global CORS middleware
/// - `Disabled`: No CORS headers will be added
/// - `Custom(config)`: Uses route-specific CORS configuration
///
/// **JSF Compliance**: This function is called ONCE at startup/initialization time.
/// All route-specific CORS policies are extracted and pre-processed before the
/// service starts handling requests. The resulting HashMap is used for O(1) lookups
/// in the hot path with no runtime parsing or allocation.
///
/// # Arguments
///
/// * `routes` - Vector of route metadata from OpenAPI spec
///
/// # Returns
///
/// A HashMap mapping handler names to their route-specific CORS policies
pub fn build_route_cors_map(routes: &[RouteMeta]) -> HashMap<String, RouteCorsPolicy> {
    let mut map = HashMap::new();

    for route in routes {
        // Only store non-Inherit policies (Inherit is the default, no need to store it)
        match &route.cors_policy {
            RouteCorsPolicy::Inherit => {
                // Skip - inherit is the default behavior
            }
            policy => {
                map.insert(route.handler_name.to_string(), policy.clone());
            }
        }
    }

    map
}
