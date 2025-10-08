use std::time::Duration;

use tracing::info_span;

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse};

/// Middleware for distributed tracing using the `tracing` crate
///
/// Creates spans for each HTTP request with method, path, and handler information.
/// Automatically records request start/completion with latency metrics.
pub struct TracingMiddleware;

impl Middleware for TracingMiddleware {
    fn before(&self, req: &HandlerRequest) -> Option<HandlerResponse> {
        // Create and immediately record a span for this request
        let span = info_span!(
            "http_request",
            method = ?req.method,
            path = %req.path,
            handler = %req.handler_name
        );

        // Use the span to record the start event
        let _guard = span.enter();
        tracing::info!("Request started");

        None
    }

    fn after(&self, req: &HandlerRequest, res: &mut HandlerResponse, latency: Duration) {
        // Create a completed span for the response
        let span = info_span!(
            "http_response",
            method = ?req.method,
            path = %req.path,
            handler = %req.handler_name,
            status = res.status,
            latency_ms = latency.as_millis() as u64
        );

        // Use the span to record the completion event
        let _guard = span.enter();
        tracing::info!(
            status = res.status,
            latency_ms = latency.as_millis() as u64,
            "Request completed"
        );

        // Explicitly drop the guard to finish the span
        drop(_guard);
    }
}
