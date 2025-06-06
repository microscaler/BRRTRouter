use std::collections::HashMap;
use std::time::Duration;

use http::Method;
use serde_json::Value;

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse};

pub struct CorsMiddleware {
    allowed_origins: Vec<String>,
    allowed_headers: Vec<String>,
    allowed_methods: Vec<Method>,
}

impl CorsMiddleware {
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

impl Default for CorsMiddleware {
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

impl Middleware for CorsMiddleware {
    fn before(&self, req: &HandlerRequest) -> Option<HandlerResponse> {
        if req.method == Method::OPTIONS {
            Some(HandlerResponse {
                status: 204,
                headers: HashMap::new(),
                body: Value::Null,
            })
        } else {
            None
        }
    }

    fn after(&self, _req: &HandlerRequest, res: &mut HandlerResponse, _latency: Duration) {
        let origins = self.allowed_origins.join(", ");
        res.headers
            .insert("Access-Control-Allow-Origin".into(), origins);

        let headers = self.allowed_headers.join(", ");
        res.headers
            .insert("Access-Control-Allow-Headers".into(), headers);

        let methods = self
            .allowed_methods
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        res.headers
            .insert("Access-Control-Allow-Methods".into(), methods);
    }
}
