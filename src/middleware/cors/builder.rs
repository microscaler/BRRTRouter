use http::Method;
use regex::Regex;
use std::sync::Arc;

use super::{CorsConfigError, CorsMiddleware, OriginValidation};

/// Builder for creating `CorsMiddleware` with a fluent API
///
/// This builder provides an ergonomic way to configure CORS middleware,
/// similar to frameworks like Rocket-RS's `CorsOptions`.
///
/// # Example
///
/// ```rust,ignore
/// use brrtrouter::middleware::CorsMiddlewareBuilder;
/// use http::Method;
///
/// let cors = CorsMiddlewareBuilder::new()
///     .allowed_origins(&["https://example.com", "https://api.example.com"])
///     .allowed_methods(&[Method::GET, Method::POST, Method::PUT])
///     .allowed_headers(&["Content-Type", "Authorization", "X-Custom-Header"])
///     .allow_credentials(true)
///     .expose_headers(&["X-Total-Count", "X-Page-Number"])
///     .max_age(3600) // Cache preflight for 1 hour
///     .build()
///     .expect("Invalid CORS configuration");
/// ```
pub struct CorsMiddlewareBuilder {
    origin_validation: Option<OriginValidation>,
    allowed_headers: Vec<String>,
    allowed_methods: Vec<Method>,
    allow_credentials: bool,
    expose_headers: Vec<String>,
    max_age: Option<u32>,
}

impl CorsMiddlewareBuilder {
    /// Create a new builder with secure defaults
    ///
    /// Default configuration:
    /// - No origins allowed (empty list)
    /// - Common headers: `["Content-Type", "Authorization"]`
    /// - Common methods: `GET, POST, PUT, DELETE, OPTIONS`
    /// - Credentials: `false`
    /// - Exposed headers: empty
    /// - Max age: `None` (no preflight caching)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use brrtrouter::middleware::CorsMiddlewareBuilder;
    ///
    /// let cors = CorsMiddlewareBuilder::new()
    ///     .allowed_origins(&["https://example.com"])
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn new() -> Self {
        Self {
            origin_validation: None,
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

    /// Set allowed origins
    ///
    /// # Arguments
    ///
    /// * `origins` - Slice of origin strings (e.g., `&["https://example.com"]`)
    ///   - Use `&["*"]` to allow all origins (insecure, not recommended for production)
    ///   - Cannot be used with `allow_credentials(true)` - will return error in `build()`
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cors = CorsMiddlewareBuilder::new()
    ///     .allowed_origins(&["https://example.com", "https://api.example.com"])
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn allowed_origins(mut self, origins: &[&str]) -> Self {
        let origins_vec: Vec<String> = origins.iter().map(|s| s.to_string()).collect();
        if origins_vec.iter().any(|o| o == "*") {
            self.origin_validation = Some(OriginValidation::Wildcard);
        } else {
            self.origin_validation = Some(OriginValidation::Exact(origins_vec));
        }
        self
    }

    /// Set allowed origins using regex patterns
    ///
    /// Origins matching any of the provided regex patterns will be allowed.
    ///
    /// # Arguments
    ///
    /// * `patterns` - Slice of regex pattern strings
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cors = CorsMiddlewareBuilder::new()
    ///     .allowed_origins_regex(&[r"^https://.*\.example\.com$", r"^https://api\.example\.org$"])
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn allowed_origins_regex(mut self, patterns: &[&str]) -> Self {
        let compiled: Result<Vec<Regex>, _> = patterns.iter().map(|p| Regex::new(p)).collect();
        match compiled {
            Ok(regexes) => {
                self.origin_validation = Some(OriginValidation::Regex(regexes));
            }
            Err(e) => {
                panic!("CORS builder error: Invalid regex pattern: {}", e);
            }
        }
        self
    }

    /// Set custom origin validation function
    ///
    /// Allows origins based on a custom validation function.
    ///
    /// # Arguments
    ///
    /// * `validator` - Function that takes an origin string and returns `true` if allowed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cors = CorsMiddlewareBuilder::new()
    ///     .allowed_origins_custom(|origin| origin.ends_with(".example.com"))
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn allowed_origins_custom<F>(mut self, validator: F) -> Self
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        self.origin_validation = Some(OriginValidation::Custom(Arc::new(validator)));
        self
    }

    /// Set allowed HTTP methods
    ///
    /// # Arguments
    ///
    /// * `methods` - Slice of HTTP methods
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use http::Method;
    ///
    /// let cors = CorsMiddlewareBuilder::new()
    ///     .allowed_methods(&[Method::GET, Method::POST, Method::PUT, Method::DELETE])
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn allowed_methods(mut self, methods: &[Method]) -> Self {
        self.allowed_methods = methods.to_vec();
        self
    }

    /// Set allowed headers
    ///
    /// # Arguments
    ///
    /// * `headers` - Slice of header names (e.g., `&["Content-Type", "Authorization"]`)
    ///   - Use `&["*"]` to allow all headers
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cors = CorsMiddlewareBuilder::new()
    ///     .allowed_headers(&["Content-Type", "Authorization", "X-Custom-Header"])
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn allowed_headers(mut self, headers: &[&str]) -> Self {
        self.allowed_headers = headers.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Enable or disable credentials
    ///
    /// When enabled, sets `Access-Control-Allow-Credentials: true` header.
    /// **Important**: Cannot be used with wildcard origin (`*`).
    ///
    /// # Arguments
    ///
    /// * `allow` - If `true`, credentials are allowed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cors = CorsMiddlewareBuilder::new()
    ///     .allowed_origins(&["https://example.com"])
    ///     .allow_credentials(true)
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn allow_credentials(mut self, allow: bool) -> Self {
        self.allow_credentials = allow;
        self
    }

    /// Set headers to expose to JavaScript
    ///
    /// These headers will be accessible via JavaScript's `response.headers.get()`.
    ///
    /// # Arguments
    ///
    /// * `headers` - Slice of header names to expose
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cors = CorsMiddlewareBuilder::new()
    ///     .expose_headers(&["X-Total-Count", "X-Page-Number", "X-Rate-Limit"])
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn expose_headers(mut self, headers: &[&str]) -> Self {
        self.expose_headers = headers.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Set preflight cache duration
    ///
    /// Browsers will cache preflight responses for this duration, reducing
    /// the number of OPTIONS requests.
    ///
    /// # Arguments
    ///
    /// * `seconds` - Cache duration in seconds (e.g., `3600` for 1 hour)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cors = CorsMiddlewareBuilder::new()
    ///     .max_age(3600) // Cache for 1 hour
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn max_age(mut self, seconds: u32) -> Self {
        self.max_age = Some(seconds);
        self
    }

    /// Build the CORS middleware
    ///
    /// Validates the configuration and returns either a `CorsMiddleware` or
    /// a `CorsConfigError` if the configuration is invalid.
    ///
    /// # Errors
    ///
    /// Returns `CorsConfigError::WildcardWithCredentials` if `allow_credentials`
    /// is `true` and wildcard origin (`*`) is configured. This violates the
    /// CORS specification.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cors = CorsMiddlewareBuilder::new()
    ///     .allowed_origins(&["https://example.com"])
    ///     .allow_credentials(true)
    ///     .build()
    ///     .expect("Invalid CORS configuration");
    /// ```
    pub fn build(self) -> Result<CorsMiddleware, CorsConfigError> {
        // Default to empty origins if none specified
        let origin_validation = self.origin_validation.unwrap_or_else(|| {
            OriginValidation::Exact(vec![])
        });

        // Validate: cannot use wildcard with credentials
        if self.allow_credentials && origin_validation.is_wildcard() {
            return Err(CorsConfigError::WildcardWithCredentials);
        }

        // Extract origins for legacy API compatibility
        let allowed_origins = match &origin_validation {
            OriginValidation::Exact(origins) => origins.clone(),
            OriginValidation::Wildcard => vec!["*".into()],
            OriginValidation::Regex(_) => vec![], // Regex patterns handled separately
            OriginValidation::Custom(_) => vec![], // Custom validator handled separately
        };

        // Use appropriate constructor based on validation type
        let cors = match origin_validation {
            OriginValidation::Regex(patterns) => {
                let pattern_strings: Vec<String> = patterns
                    .iter()
                    .map(|re| re.as_str().to_string())
                    .collect();
                CorsMiddleware::with_regex_patterns(
                    pattern_strings,
                    self.allowed_headers,
                    self.allowed_methods,
                    self.allow_credentials,
                    self.expose_headers,
                    self.max_age,
                )
            }
            OriginValidation::Custom(validator) => {
                CorsMiddleware::with_custom_validator(
                    move |origin| validator(origin),
                    self.allowed_headers,
                    self.allowed_methods,
                    self.allow_credentials,
                    self.expose_headers,
                    self.max_age,
                )
            }
            _ => CorsMiddleware::new(
                allowed_origins,
                self.allowed_headers,
                self.allowed_methods,
                self.allow_credentials,
                self.expose_headers,
                self.max_age,
            ),
        };

        Ok(cors)
    }
}

impl Default for CorsMiddlewareBuilder {
    fn default() -> Self {
        Self::new()
    }
}

