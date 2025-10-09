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

/// Default initialization for metrics middleware
///
/// Creates a new instance with all atomic counters set to zero.
/// Equivalent to `MetricsMiddleware::new()`.
impl Default for MetricsMiddleware {
    /// Create a metrics middleware with zeroed counters
    ///
    /// All metrics start at zero and increment as requests are processed.
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

    /// Increment the authentication failure counter
    pub fn inc_auth_failure(&self) {
        self.auth_failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the total number of authentication failures
    pub fn auth_failures(&self) -> usize {
        self.auth_failures.load(Ordering::Relaxed)
    }
}

/// Metrics collection middleware implementation
///
/// Automatically tracks request statistics using atomic operations for thread-safety.
/// This middleware is passive - it never blocks requests, only observes and records.
///
/// # Metrics Collected
///
/// - **Request count**: Total requests processed
/// - **Latency**: Average processing time (calculated from `after()`)
/// - **Stack usage**: Coroutine stack size and peak usage
/// - **Top-level requests**: Infrastructure endpoints (health, metrics, docs)
/// - **Auth failures**: Failed authentication attempts
///
/// # Performance
///
/// Uses `Ordering::Relaxed` for atomic operations to minimize overhead.
/// Metrics are eventually consistent but extremely low-cost to collect.
impl Middleware for MetricsMiddleware {
    /// Increment request counter before processing
    ///
    /// Called for every request that reaches the dispatcher.
    /// Increments the total request count atomically.
    ///
    /// # Arguments
    ///
    /// * `_req` - The incoming request (unused)
    ///
    /// # Returns
    ///
    /// Always returns `None` (never blocks requests)
    fn before(&self, _req: &HandlerRequest) -> Option<HandlerResponse> {
        self.request_count.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Record latency and stack metrics after processing
    ///
    /// Called after the handler completes. Updates:
    /// 1. Total latency (for average calculation)
    /// 2. Stack size and usage (if running in a coroutine)
    ///
    /// # Arguments
    ///
    /// * `_req` - The original request (unused)
    /// * `_res` - The response (unused)
    /// * `latency` - Time taken to process the request
    ///
    /// # Stack Tracking
    ///
    /// - If in coroutine context: Records actual stack size from coroutine
    /// - If not in coroutine: Records global stack size from May config
    /// - Used stack is always 0 (May doesn't expose actual usage)
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
