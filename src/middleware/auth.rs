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

impl Middleware for AuthMiddleware {
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

    fn after(&self, _req: &HandlerRequest, _res: &mut HandlerResponse, _latency: Duration) {}
}
