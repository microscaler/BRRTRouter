use std::time::Duration;

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse};

pub struct CorsMiddleware;

impl Middleware for CorsMiddleware {
    fn after(&self, _req: &HandlerRequest, res: &mut HandlerResponse, _latency: Duration) {
        res.headers
            .insert("Access-Control-Allow-Origin".into(), "*".into());
        res.headers.insert(
            "Access-Control-Allow-Headers".into(),
            "Content-Type, Authorization".into(),
        );
        res.headers.insert(
            "Access-Control-Allow-Methods".into(),
            "GET, POST, PUT, DELETE, OPTIONS".into(),
        );
    }
}
