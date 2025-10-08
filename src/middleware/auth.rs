use std::collections::HashMap;
use std::time::Duration;

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse};

/// Simple token-based authentication middleware
///
/// Checks for an `Authorization` header matching the configured token.
/// Returns 401 Unauthorized if the token is missing or incorrect.
///
/// **Note**: This is a basic example middleware. For production use,
/// use the full security system with `SecurityProvider` implementations.
pub struct AuthMiddleware {
    token: String,
}

impl AuthMiddleware {
    /// Create a new auth middleware with the given token
    ///
    /// # Arguments
    ///
    /// * `token` - Expected authorization token value
    pub fn new(token: String) -> Self {
        Self { token }
    }
}

/// Simple token-based authentication middleware implementation
///
/// Provides basic authorization by checking if the `Authorization` header
/// matches the configured token. This is a teaching example - production
/// systems should use the full `SecurityProvider` system.
///
/// # Flow
///
/// 1. Check if `Authorization` header exists
/// 2. Compare header value with configured token (exact match)
/// 3. If match: Allow request to proceed
/// 4. If no match: Return 401 Unauthorized immediately
///
/// # Security Warning
///
/// This middleware:
/// - ❌ Does NOT use hashing or encryption
/// - ❌ Does NOT support token expiration
/// - ❌ Does NOT validate token format
/// - ✅ Only suitable for testing/examples
///
/// For production: Use `BearerJwtProvider`, `JwksBearerProvider`, or `RemoteApiKeyProvider`
impl Middleware for AuthMiddleware {
    /// Check authorization before handler execution
    ///
    /// Returns 401 if the Authorization header is missing or doesn't match the token.
    ///
    /// # Arguments
    ///
    /// * `req` - The incoming request
    ///
    /// # Returns
    ///
    /// - `None` - Token is valid, proceed to handler
    /// - `Some(401 response)` - Token is invalid or missing
    fn before(&self, req: &HandlerRequest) -> Option<HandlerResponse> {
        match req.headers.get("authorization") {
            Some(h) if h == &self.token => None,
            _ => Some(HandlerResponse {
                status: 401,
                headers: HashMap::new(),
                body: serde_json::json!({ "error": "Unauthorized" }),
            }),
        }
    }

    /// No-op after processing (auth happens in `before()`)
    ///
    /// This middleware performs all validation before handler execution,
    /// so there's nothing to do after the response is generated.
    ///
    /// # Arguments
    ///
    /// * `_req` - The original request (unused)
    /// * `_res` - The response (unused)
    /// * `_latency` - Request processing duration (unused)
    fn after(&self, _req: &HandlerRequest, _res: &mut HandlerResponse, _latency: Duration) {}
}
