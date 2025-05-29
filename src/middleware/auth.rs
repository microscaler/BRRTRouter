use std::collections::HashMap;
use std::time::Duration;

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse};

pub struct AuthMiddleware {
    token: String,
}

impl AuthMiddleware {
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

