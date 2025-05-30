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
        let guard = span.clone().entered();
        SPAN_GUARD.with(|g| {
            *g.borrow_mut() = Some((span, guard));
        });
        None
    }

    fn after(&self, _req: &HandlerRequest, res: &mut HandlerResponse, latency: Duration) {
        SPAN_GUARD.with(|g| {
            if let Some((span, _guard)) = g.borrow_mut().take() {
                span.record("status", &res.status);
                span.record("latency_ms", &(latency.as_millis() as u64));
                let stack_size = if may::coroutine::is_coroutine() {
                    may::coroutine::current().stack_size()
                } else {
                    may::config().get_stack_size()
                };
                // `may` does not expose used stack space programmatically
                // so emit zero as a placeholder.
                span.record("stack_size", &stack_size);
                span.record("used_stack", &0usize);
            }
        });
    }
}

