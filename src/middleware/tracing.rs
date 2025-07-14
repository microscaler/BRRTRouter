use std::time::Duration;

use tracing::info_span;

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse};

pub struct TracingMiddleware;

impl Middleware for TracingMiddleware {
    fn before(&self, req: &HandlerRequest) -> Option<HandlerResponse> {
        // Create and enter span for this request
        let _span = info_span!(
            "request",
            method = ?req.method,
            path = %req.path,
            handler = %req.handler_name
        ).entered();
        
        // TODO: Implement proper span storage for May coroutines
        // The current implementation is simplified due to May coroutine threading model
        
        None
    }

    fn after(&self, _req: &HandlerRequest, res: &mut HandlerResponse, latency: Duration) {
        // TODO: Record span attributes when proper span storage is implemented
        // For now, just log the completion
        tracing::info!(
            status = res.status,
            latency_ms = latency.as_millis() as u64,
            "Request completed"
        );
    }
}
