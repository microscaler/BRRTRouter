use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse};

/// Middleware for collecting Prometheus-compatible metrics
///
/// Tracks request counts, latency, stack usage, and authentication failures.
/// All counters use atomic operations for thread-safe updates without locks.
///
/// Metrics collected:
/// - Total request count
/// - Average latency (request processing time)
/// - Stack size and usage (for coroutine monitoring)
/// - Top-level request count (non-handler requests like /health, /metrics)
/// - Authentication failure count
pub struct MetricsMiddleware {
    request_count: AtomicUsize,
    total_latency_ns: AtomicU64,
    stack_size: AtomicUsize,
    used_stack: AtomicUsize,
    top_level_requests: AtomicUsize,
    auth_failures: AtomicUsize,
}

impl Default for MetricsMiddleware {
    fn default() -> Self {
        Self {
            request_count: AtomicUsize::new(0),
            total_latency_ns: AtomicU64::new(0),
            stack_size: AtomicUsize::new(0),
            used_stack: AtomicUsize::new(0),
            top_level_requests: AtomicUsize::new(0),
            auth_failures: AtomicUsize::new(0),
        }
    }
}

impl MetricsMiddleware {
    /// Create a new metrics middleware with all counters initialized to zero
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the total number of requests processed
    pub fn request_count(&self) -> usize {
        self.request_count.load(Ordering::Relaxed)
    }

    /// Calculate the average request latency
    ///
    /// Returns the mean processing time across all requests.
    /// Returns zero duration if no requests have been processed yet.
    pub fn average_latency(&self) -> Duration {
        let count = self.request_count.load(Ordering::Relaxed) as u64;
        if count == 0 {
            Duration::from_nanos(0)
        } else {
            Duration::from_nanos(self.total_latency_ns.load(Ordering::Relaxed) / count)
        }
    }

    /// Get coroutine stack size and peak usage
    ///
    /// # Returns
    ///
    /// A tuple of `(total_stack_size, peak_used_stack)`
    pub fn stack_usage(&self) -> (usize, usize) {
        (
            self.stack_size.load(Ordering::Relaxed),
            self.used_stack.load(Ordering::Relaxed),
        )
    }

    /// Increment the top-level request counter
    ///
    /// Call this for infrastructure endpoints like `/health`, `/metrics`, `/docs`
    /// that don't go through the handler dispatch system.
    pub fn inc_top_level_request(&self) {
        self.top_level_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the total number of top-level requests
    ///
    /// Top-level requests are those that bypass the handler system
    /// (e.g., health checks, metrics endpoints, static files).
    pub fn top_level_request_count(&self) -> usize {
        self.top_level_requests.load(Ordering::Relaxed)
    }

    pub fn inc_auth_failure(&self) {
        self.auth_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn auth_failures(&self) -> usize {
        self.auth_failures.load(Ordering::Relaxed)
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
            let co = may::coroutine::current();
            let size = co.stack_size();
            self.stack_size.store(size, Ordering::Relaxed);
            let used = 0; // Stack usage tracking not available in May coroutines
            self.used_stack.store(used, Ordering::Relaxed);
        } else {
            self.stack_size
                .store(may::config().get_stack_size(), Ordering::Relaxed);
            self.used_stack.store(0, Ordering::Relaxed);
        }
    }
}
