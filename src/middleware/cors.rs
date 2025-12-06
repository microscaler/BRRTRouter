use std::time::Duration;

use http::Method;
use serde_json::Value;

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse, HeaderVec};

/// CORS (Cross-Origin Resource Sharing) middleware
///
/// Handles preflight OPTIONS requests and adds CORS headers to responses.
/// Configurable with allowed origins, headers, and methods.
pub struct CorsMiddleware {
    allowed_origins: Vec<String>,
    allowed_headers: Vec<String>,
    allowed_methods: Vec<Method>,
}

impl CorsMiddleware {
    /// Create a new CORS middleware with specific configuration
    ///
    /// # Arguments
    ///
    /// * `allowed_origins` - List of allowed origins (e.g., `["https://example.com"]`)
    /// * `allowed_headers` - List of allowed headers (e.g., `["Content-Type", "Authorization"]`)
    /// * `allowed_methods` - List of allowed HTTP methods
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
    /// );
    /// ```
    pub fn new(
        allowed_origins: Vec<String>,
        allowed_headers: Vec<String>,
        allowed_methods: Vec<Method>,
    ) -> Self {
        Self {
            allowed_origins,
            allowed_headers,
            allowed_methods,
        }
    }
}

/// Default CORS policy allowing all origins and common methods
///
/// This implementation provides a permissive CORS configuration suitable for
/// development and testing. Production systems should use `CorsMiddleware::new()`
/// with specific origin restrictions.
impl Default for CorsMiddleware {
    /// Create a default CORS middleware
    ///
    /// Default configuration:
    /// - `allowed_origins`: `["*"]` (all origins)
    /// - `allowed_headers`: `["*"]` (all headers)
    /// - `allowed_methods`: `GET, POST, PUT, DELETE, PATCH, OPTIONS`
    ///
    /// # Example
    ///
    /// ```rust
    /// use brrtrouter::middleware::CorsMiddleware;
    ///
    /// let cors = CorsMiddleware::default();
    /// // Allows all origins, headers, and common HTTP methods
    /// ```
    fn default() -> Self {
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
    /// If the request is OPTIONS, immediately return a 200 response with CORS headers.
    /// For other methods, return None to allow the request to proceed to the handler.
    ///
    /// # Arguments
    ///
    /// * `req` - The incoming request
    ///
    /// # Returns
    ///
    /// - `Some(response)` - For OPTIONS requests (preflight)
    /// - `None` - For all other requests (proceed to handler)
    fn before(&self, req: &HandlerRequest) -> Option<HandlerResponse> {
        if req.method == Method::OPTIONS {
            Some(HandlerResponse::new(204, HeaderVec::new(), Value::Null))
        } else {
            None
        }
    }

    /// Add CORS headers to the response after handler execution
    ///
    /// Called for all non-OPTIONS requests. Adds CORS headers to allow cross-origin
    /// access based on the middleware configuration.
    ///
    /// # Arguments
    ///
    /// * `_req` - The original request (unused)
    /// * `res` - The response to modify (headers added in-place)
    /// * `_latency` - Request processing duration (unused)
    ///
    /// # Headers Added
    ///
    /// - `Access-Control-Allow-Origin`: First allowed origin or `*`
    /// - `Access-Control-Allow-Methods`: Comma-separated list of allowed methods
    /// - `Access-Control-Allow-Headers`: First allowed header or `*`
    fn after(&self, _req: &HandlerRequest, res: &mut HandlerResponse, _latency: Duration) {
        let origins = self.allowed_origins.join(", ");
        res.set_header("Access-Control-Allow-Origin", origins);

        let headers = self.allowed_headers.join(", ");
        res.set_header("Access-Control-Allow-Headers", headers);

        let methods = self
            .allowed_methods
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        res.set_header("Access-Control-Allow-Methods", methods);
    }
}
