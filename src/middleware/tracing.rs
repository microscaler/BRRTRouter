use std::cell::RefCell;
use std::time::Duration;

use tracing::{info_span, span::EnteredSpan, Span};

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse};

thread_local! {
    static SPAN_GUARD: RefCell<Option<(Span, EnteredSpan)>> = RefCell::new(None);
}

pub struct TracingMiddleware;

impl Middleware for TracingMiddleware {
    fn before(&self, req: &HandlerRequest) -> Option<HandlerResponse> {
        let span = info_span!(
            "request",
            method = ?req.method,
            path = %req.path,
            handler = %req.handler_name
        );
        let guard = span.enter();
        static SPAN_GUARD: std::sync::RwLock<Option<(Span, tracing::span::Entered<'static>)>> =
            std::sync::RwLock::new(None);
        None
    }

    fn after(&self, _req: &HandlerRequest, res: &mut HandlerResponse, latency: Duration) {
        SPAN_GUARD.with(|g| {
            if let Some((span, _guard)) = g.borrow_mut().take() {
                span.record("status", &res.status);
                span.record("latency_ms", &(latency.as_millis() as u64));
            }
        });
    }
}
