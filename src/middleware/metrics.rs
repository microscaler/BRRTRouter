use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse};

pub struct MetricsMiddleware {
    request_count: AtomicUsize,
    total_latency_ns: AtomicU64,
    stack_size: AtomicUsize,
    used_stack: AtomicUsize,
}

impl MetricsMiddleware {
    pub fn new() -> Self {
        Self {
            request_count: AtomicUsize::new(0),
            total_latency_ns: AtomicU64::new(0),
            stack_size: AtomicUsize::new(0),
            used_stack: AtomicUsize::new(0),
        }
    }

    pub fn request_count(&self) -> usize {
        self.request_count.load(Ordering::Relaxed)
    }

    pub fn average_latency(&self) -> Duration {
        let count = self.request_count.load(Ordering::Relaxed) as u64;
        if count == 0 {
            Duration::from_nanos(0)
        } else {
            Duration::from_nanos(self.total_latency_ns.load(Ordering::Relaxed) / count)
        }
    }

    pub fn stack_usage(&self) -> (usize, usize) {
        (
            self.stack_size.load(Ordering::Relaxed),
            self.used_stack.load(Ordering::Relaxed),
        )
    }
}

impl Middleware for MetricsMiddleware {
    fn before(&self, _req: &HandlerRequest) -> Option<HandlerResponse> {
        self.request_count.fetch_add(1, Ordering::Relaxed);
        None
    }

    fn after(&self, _req: &HandlerRequest, _res: &mut HandlerResponse, latency: Duration) {
        self.total_latency_ns
            .fetch_add(latency.as_nanos() as u64, Ordering::Relaxed);
        // record stack metrics for the current coroutine when available
        if may::coroutine::is_coroutine() {
            let size = may::coroutine::current().stack_size();
            self.stack_size.store(size, Ordering::Relaxed);
        } else {
            self.stack_size
                .store(may::config().get_stack_size(), Ordering::Relaxed);
        }
        // `may` only exposes used stack information via debug output when the
        // stack size is odd. Capturing that output is non-trivial in this
        // environment, so we store zero as a placeholder.
        self.used_stack.store(0, Ordering::Relaxed);
    }
}
