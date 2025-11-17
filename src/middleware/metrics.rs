use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;

use super::Middleware;
use crate::dispatcher::{HandlerRequest, HandlerResponse};

/// Histogram buckets for latency tracking (in seconds)
/// Buckets: 1ms, 5ms, 10ms, 50ms, 100ms, 500ms, 1s, 5s, 10s, +Inf
const HISTOGRAM_BUCKETS: &[f64] = &[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0];

/// Histogram metric for tracking request duration distribution
struct HistogramMetric {
    /// Bucket counts (one per bucket + one for +Inf)
    buckets: Vec<AtomicU64>,
    /// Sum of all observed values
    sum: AtomicU64,
    /// Total count of observations
    count: AtomicU64,
}

impl HistogramMetric {
    fn new() -> Self {
        let mut buckets = Vec::with_capacity(HISTOGRAM_BUCKETS.len() + 1);
        for _ in 0..=HISTOGRAM_BUCKETS.len() {
            buckets.push(AtomicU64::new(0));
        }
        Self {
            buckets,
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    /// Record a duration observation (in seconds)
    fn observe(&self, duration_secs: f64) {
        // Find the appropriate bucket
        let bucket_idx = HISTOGRAM_BUCKETS
            .iter()
            .position(|&b| duration_secs <= b)
            .unwrap_or(HISTOGRAM_BUCKETS.len());

        // Increment all buckets from this one to +Inf (cumulative histogram)
        for i in bucket_idx..self.buckets.len() {
            self.buckets[i].fetch_add(1, Ordering::Relaxed);
        }

        // Update sum and count
        let duration_nanos = (duration_secs * 1_000_000_000.0) as u64;
        self.sum.fetch_add(duration_nanos, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get bucket counts for Prometheus export
    fn get_buckets(&self) -> Vec<u64> {
        self.buckets
            .iter()
            .map(|b| b.load(Ordering::Relaxed))
            .collect()
    }

    /// Get sum in nanoseconds
    fn get_sum_ns(&self) -> u64 {
        self.sum.load(Ordering::Relaxed)
    }

    /// Get total count
    fn get_count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

/// Per-path metrics tracking
#[derive(Default)]
struct PathMetrics {
    count: AtomicUsize,
    total_latency_ns: AtomicU64,
    max_latency_ns: AtomicU64,
    min_latency_ns: AtomicU64,
}



impl PathMetrics {
    fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
            total_latency_ns: AtomicU64::new(0),
            max_latency_ns: AtomicU64::new(0),
            min_latency_ns: AtomicU64::new(u64::MAX), // Start high for min
        }
    }

    fn record(&self, latency_ns: u64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ns
            .fetch_add(latency_ns, Ordering::Relaxed);

        // Update max
        let mut current_max = self.max_latency_ns.load(Ordering::Relaxed);
        while latency_ns > current_max {
            match self.max_latency_ns.compare_exchange_weak(
                current_max,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }

        // Update min
        let mut current_min = self.min_latency_ns.load(Ordering::Relaxed);
        while latency_ns < current_min {
            match self.min_latency_ns.compare_exchange_weak(
                current_min,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_min = x,
            }
        }
    }
}

/// Middleware for collecting Prometheus-compatible metrics
///
/// Tracks request counts, latency, stack usage, and authentication failures.
/// Uses lock-free concurrent data structures (DashMap) for high-throughput scenarios.
///
/// Metrics collected:
/// - Total request count (with status code labels)
/// - Active requests (concurrent in-flight requests)
/// - Request duration histogram (for percentile calculations)
/// - Average latency (request processing time)
/// - Stack size and usage (for coroutine monitoring)
/// - Top-level request count (non-handler requests like /health, /metrics)
/// - Authentication failure count
/// - Per-path metrics (count, latency, min/max)
///
/// ## Performance Optimizations
///
/// This middleware uses sharded lock-free data structures (DashMap) instead of
/// traditional `RwLock<HashMap>` to reduce lock contention under high concurrency.
/// Each path's metrics are stored as atomic counters, allowing concurrent updates
/// without blocking. This design handles 5k+ requests per second efficiently.
pub struct MetricsMiddleware {
    request_count: AtomicUsize,
    total_latency_ns: AtomicU64,
    stack_size: AtomicUsize,
    used_stack: AtomicUsize,
    top_level_requests: AtomicUsize,
    auth_failures: AtomicUsize,
    /// Active requests currently being processed (incremented on start, decremented on completion)
    active_requests: AtomicI64,
    /// Per-path metrics for detailed monitoring (lock-free concurrent map)
    path_metrics: Arc<DashMap<String, Arc<PathMetrics>>>,
    /// Per-(path, status) request counts for status code tracking (lock-free concurrent map)
    status_metrics: Arc<DashMap<(String, u16), AtomicUsize>>,
    /// Histogram for request duration (for percentile calculations)
    duration_histogram: Arc<HistogramMetric>,
    /// Connection close events (client disconnects, timeouts, etc.)
    connection_closes: AtomicUsize,
    /// Connection errors (broken pipe, reset, etc.)
    connection_errors: AtomicUsize,
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
            active_requests: AtomicI64::new(0),
            path_metrics: Arc::new(DashMap::new()),
            status_metrics: Arc::new(DashMap::new()),
            duration_histogram: Arc::new(HistogramMetric::new()),
            connection_closes: AtomicUsize::new(0),
            connection_errors: AtomicUsize::new(0),
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

    /// Increment the connection close counter
    pub fn inc_connection_close(&self) {
        self.connection_closes.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the total number of connection closes
    pub fn connection_closes(&self) -> usize {
        self.connection_closes.load(Ordering::Relaxed)
    }

    /// Increment the connection error counter
    pub fn inc_connection_error(&self) {
        self.connection_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the total number of connection errors
    pub fn connection_errors(&self) -> usize {
        self.connection_errors.load(Ordering::Relaxed)
    }

    /// Get connection health ratio (successful requests vs connection issues)
    pub fn connection_health_ratio(&self) -> f64 {
        let total_requests = self.request_count() as f64;
        let total_issues = (self.connection_closes() + self.connection_errors()) as f64;
        if total_requests + total_issues > 0.0 {
            total_requests / (total_requests + total_issues)
        } else {
            1.0 // No data yet, assume healthy
        }
    }

    /// Record metrics for a specific path
    ///
    /// This is called internally by the middleware to track per-path statistics.
    /// Uses lock-free DashMap for concurrent access without contention.
    pub(crate) fn record_path_metrics(&self, path: &str, latency_ns: u64) {
        // Use DashMap's entry API for lock-free get-or-insert
        let metrics = self.path_metrics
            .entry(path.to_string())
            .or_insert_with(|| Arc::new(PathMetrics::new()))
            .clone();

        metrics.record(latency_ns);
    }

    /// Get all per-path metrics for Prometheus export
    ///
    /// Returns a snapshot of metrics for all paths that have been accessed.
    /// The returned HashMap maps path -> (count, avg_latency_ns, min_ns, max_ns).
    pub fn path_stats(&self) -> HashMap<String, (usize, u64, u64, u64)> {
        self.path_metrics.iter()
            .map(|entry| {
                let path = entry.key().clone();
                let pm = entry.value();
                let count = pm.count.load(Ordering::Relaxed);
                let total = pm.total_latency_ns.load(Ordering::Relaxed);
                let min = pm.min_latency_ns.load(Ordering::Relaxed);
                let max = pm.max_latency_ns.load(Ordering::Relaxed);
                let avg = if count > 0 { total / count as u64 } else { 0 };
                (path, (count, avg, min, max))
            })
            .collect()
    }

    /// Get the current number of active (in-flight) requests
    pub fn active_requests(&self) -> i64 {
        self.active_requests.load(Ordering::Relaxed)
    }

    /// Get all status code metrics for Prometheus export
    ///
    /// Returns a HashMap mapping (path, status_code) -> count
    pub fn status_stats(&self) -> HashMap<(String, u16), usize> {
        self.status_metrics.iter()
            .map(|entry| {
                let (path, status) = entry.key().clone();
                let count = entry.value().load(Ordering::Relaxed);
                ((path, status), count)
            })
            .collect()
    }

    /// Get histogram data for Prometheus export
    ///
    /// Returns (buckets, sum_ns, count) where buckets is a Vec of cumulative counts
    pub fn histogram_data(&self) -> (Vec<u64>, u64, u64) {
        let buckets = self.duration_histogram.get_buckets();
        let sum = self.duration_histogram.get_sum_ns();
        let count = self.duration_histogram.get_count();
        (buckets, sum, count)
    }

    /// Get histogram bucket boundaries (in seconds)
    pub fn histogram_buckets() -> &'static [f64] {
        HISTOGRAM_BUCKETS
    }

    /// Pre-register paths at service startup
    ///
    /// This method allows pre-registering known paths to avoid on-the-fly
    /// metric path creation during request handling in the steady state.
    /// This further reduces contention by ensuring metric structures exist
    /// before high-throughput request processing begins.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use brrtrouter::middleware::MetricsMiddleware;
    /// let metrics = MetricsMiddleware::new();
    /// metrics.pre_register_paths(&[
    ///     "/api/users",
    ///     "/api/posts",
    ///     "/health",
    /// ]);
    /// ```
    pub fn pre_register_paths<S: AsRef<str>>(&self, paths: &[S]) {
        for path in paths {
            let path_str = path.as_ref();
            self.path_metrics
                .entry(path_str.to_string())
                .or_insert_with(|| Arc::new(PathMetrics::new()));
        }
    }

    /// Record status code for a request
    ///
    /// Uses lock-free DashMap for concurrent access without contention.
    fn record_status(&self, path: &str, status: u16) {
        let key = (path.to_string(), status);
        
        // Use DashMap's entry API for lock-free get-or-insert
        self.status_metrics
            .entry(key)
            .or_insert_with(|| AtomicUsize::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Metrics collection middleware implementation
///
/// Automatically tracks request statistics using atomic operations for thread-safety.
/// This middleware is passive - it never blocks requests, only observes and records.
///
/// # Metrics Collected
///
/// - **Request count**: Total requests processed (with status code labels)
/// - **Active requests**: Current number of in-flight requests
/// - **Duration histogram**: Request duration distribution for percentile calculations
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
    /// Increment request counters before processing
    ///
    /// Called for every request that reaches the dispatcher.
    /// Increments the total request count and active requests atomically.
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
        self.active_requests.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Record latency, status, histogram, and stack metrics after processing
    ///
    /// Called after the handler completes. Updates:
    /// 1. Active requests (decrement)
    /// 2. Total latency (for average calculation)
    /// 3. Per-path latency and counters
    /// 4. Status code counters (for error rate tracking)
    /// 5. Duration histogram (for percentile calculations)
    /// 6. Stack size and usage (if running in a coroutine)
    ///
    /// # Arguments
    ///
    /// * `req` - The original request (used for path tracking)
    /// * `res` - The response (used for status code tracking)
    /// * `latency` - Time taken to process the request
    ///
    /// # Stack Tracking
    ///
    /// - If in coroutine context: Records actual stack size from coroutine
    /// - If not in coroutine: Records global stack size from May config
    /// - Used stack is always 0 (May doesn't expose actual usage)
    fn after(&self, req: &HandlerRequest, res: &mut HandlerResponse, latency: Duration) {
        // Decrement active requests
        self.active_requests.fetch_sub(1, Ordering::Relaxed);

        let latency_ns = latency.as_nanos() as u64;
        let latency_secs = latency.as_secs_f64();

        self.total_latency_ns
            .fetch_add(latency_ns, Ordering::Relaxed);

        // Record per-path metrics
        self.record_path_metrics(&req.path, latency_ns);

        // Record status code metrics
        self.record_status(&req.path, res.status);

        // Record duration histogram (for percentiles)
        self.duration_histogram.observe(latency_secs);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_path_metrics_same_path() {
        let metrics = MetricsMiddleware::new();
        let path = "/users/{id}";
        
        // Record metrics for the same path multiple times
        metrics.record_path_metrics(path, 1000);
        metrics.record_path_metrics(path, 2000);
        metrics.record_path_metrics(path, 3000);
        
        let stats = metrics.path_stats();
        assert_eq!(stats.len(), 1);
        assert!(stats.contains_key(path));
        
        let (count, avg, min, max) = stats.get(path).unwrap();
        assert_eq!(*count, 3);
        assert_eq!(*avg, 2000); // (1000 + 2000 + 3000) / 3
        assert_eq!(*min, 1000);
        assert_eq!(*max, 3000);
    }

    #[test]
    fn test_record_path_metrics_different_paths() {
        let metrics = MetricsMiddleware::new();
        
        // Record metrics for different paths
        metrics.record_path_metrics("/users/{id}", 1000);
        metrics.record_path_metrics("/pets", 2000);
        metrics.record_path_metrics("/users/{id}", 1500);
        
        let stats = metrics.path_stats();
        assert_eq!(stats.len(), 2);
        assert!(stats.contains_key("/users/{id}"));
        assert!(stats.contains_key("/pets"));
        
        let (count, avg, min, max) = stats.get("/users/{id}").unwrap();
        assert_eq!(*count, 2);
        assert_eq!(*avg, 1250); // (1000 + 1500) / 2
        assert_eq!(*min, 1000);
        assert_eq!(*max, 1500);
        
        let (count, avg, min, max) = stats.get("/pets").unwrap();
        assert_eq!(*count, 1);
        assert_eq!(*avg, 2000);
        assert_eq!(*min, 2000);
        assert_eq!(*max, 2000);
    }

    #[test]
    fn test_record_status_same_path_status() {
        let metrics = MetricsMiddleware::new();
        let path = "/users/{id}";
        
        // Record same status for same path multiple times
        metrics.record_status(path, 200);
        metrics.record_status(path, 200);
        metrics.record_status(path, 200);
        
        let stats = metrics.status_stats();
        assert_eq!(stats.len(), 1);
        assert_eq!(*stats.get(&(path.to_string(), 200)).unwrap(), 3);
    }

    #[test]
    fn test_record_status_different_statuses() {
        let metrics = MetricsMiddleware::new();
        let path = "/users/{id}";
        
        // Record different statuses for same path
        metrics.record_status(path, 200);
        metrics.record_status(path, 200);
        metrics.record_status(path, 404);
        metrics.record_status(path, 500);
        metrics.record_status(path, 500);
        metrics.record_status(path, 500);
        
        let stats = metrics.status_stats();
        assert_eq!(stats.len(), 3);
        assert_eq!(*stats.get(&(path.to_string(), 200)).unwrap(), 2);
        assert_eq!(*stats.get(&(path.to_string(), 404)).unwrap(), 1);
        assert_eq!(*stats.get(&(path.to_string(), 500)).unwrap(), 3);
    }

    #[test]
    fn test_record_status_different_paths() {
        let metrics = MetricsMiddleware::new();
        
        // Record statuses for different paths
        metrics.record_status("/users/{id}", 200);
        metrics.record_status("/pets", 200);
        metrics.record_status("/users/{id}", 404);
        
        let stats = metrics.status_stats();
        assert_eq!(stats.len(), 3);
        assert_eq!(*stats.get(&("/users/{id}".to_string(), 200)).unwrap(), 1);
        assert_eq!(*stats.get(&("/users/{id}".to_string(), 404)).unwrap(), 1);
        assert_eq!(*stats.get(&("/pets".to_string(), 200)).unwrap(), 1);
    }

    #[test]
    fn test_pre_register_paths() {
        let metrics = MetricsMiddleware::new();
        
        // Pre-register paths
        metrics.pre_register_paths(&[
            "/users/{id}",
            "/pets",
            "/health",
        ]);
        
        // Verify paths were registered (they exist in the map)
        let stats = metrics.path_stats();
        assert_eq!(stats.len(), 3);
        assert!(stats.contains_key("/users/{id}"));
        assert!(stats.contains_key("/pets"));
        assert!(stats.contains_key("/health"));
        
        // All should have zero counts initially
        for (_, (count, _, _, _)) in stats.iter() {
            assert_eq!(*count, 0);
        }
    }

    #[test]
    fn test_concurrent_path_metrics() {
        use std::sync::Arc;
        use std::thread;

        let metrics = Arc::new(MetricsMiddleware::new());
        let mut handles = vec![];

        // Spawn 10 threads, each recording 1000 metrics
        for thread_id in 0..10 {
            let metrics_clone = Arc::clone(&metrics);
            let handle = thread::spawn(move || {
                for i in 0..1000 {
                    let path = format!("/path{}", thread_id % 3); // 3 different paths
                    let latency = 1000 + (i % 100) as u64;
                    metrics_clone.record_path_metrics(&path, latency);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify the counts
        let stats = metrics.path_stats();
        assert_eq!(stats.len(), 3);
        
        // Each path should have been hit by 10/3 = 3-4 threads, 1000 times each
        let total_count: usize = stats.values().map(|(count, _, _, _)| count).sum();
        assert_eq!(total_count, 10000); // 10 threads * 1000 iterations
    }

    #[test]
    fn test_concurrent_status_metrics() {
        use std::sync::Arc;
        use std::thread;

        let metrics = Arc::new(MetricsMiddleware::new());
        let mut handles = vec![];

        // Spawn 10 threads, each recording 1000 status codes
        for thread_id in 0..10 {
            let metrics_clone = Arc::clone(&metrics);
            let handle = thread::spawn(move || {
                for i in 0..1000 {
                    let path = format!("/path{}", thread_id % 2); // 2 different paths
                    let status = if i % 3 == 0 { 200 } else if i % 3 == 1 { 404 } else { 500 };
                    metrics_clone.record_status(&path, status);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify the counts
        let stats = metrics.status_stats();
        
        // Should have 2 paths * 3 status codes = 6 entries
        assert_eq!(stats.len(), 6);
        
        let total_count: usize = stats.values().sum();
        assert_eq!(total_count, 10000); // 10 threads * 1000 iterations
    }

    #[test]
    fn test_high_contention_same_path() {
        use std::sync::Arc;
        use std::thread;

        let metrics = Arc::new(MetricsMiddleware::new());
        let mut handles = vec![];
        let path = "/hot/path"; // All threads hitting the same path

        // Spawn 20 threads, all recording to the same path
        for _ in 0..20 {
            let metrics_clone = Arc::clone(&metrics);
            let handle = thread::spawn(move || {
                for i in 0..500 {
                    let latency = 1000 + (i % 100) as u64;
                    metrics_clone.record_path_metrics(path, latency);
                    metrics_clone.record_status(path, 200);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify the counts
        let path_stats = metrics.path_stats();
        assert_eq!(path_stats.len(), 1);
        let (count, _, _, _) = path_stats.get(path).unwrap();
        assert_eq!(*count, 10000); // 20 threads * 500 iterations

        let status_stats = metrics.status_stats();
        assert_eq!(status_stats.len(), 1);
        assert_eq!(*status_stats.get(&(path.to_string(), 200)).unwrap(), 10000);
    }

    #[test]
    fn test_concurrent_pre_registration_and_recording() {
        use std::sync::Arc;
        use std::thread;

        let metrics = Arc::new(MetricsMiddleware::new());
        
        // Pre-register some paths
        metrics.pre_register_paths(&["/api/users", "/api/posts"]);
        
        let mut handles = vec![];

        // Spawn threads that record to both pre-registered and new paths
        for _ in 0..10 {
            let metrics_clone = Arc::clone(&metrics);
            let handle = thread::spawn(move || {
                for i in 0..500 {
                    // Mix of pre-registered and new paths
                    let path = if i % 3 == 0 {
                        "/api/users"
                    } else if i % 3 == 1 {
                        "/api/posts"
                    } else {
                        "/api/comments" // Not pre-registered
                    };
                    
                    let latency = 1000 + (i % 100) as u64;
                    metrics_clone.record_path_metrics(path, latency);
                    metrics_clone.record_status(path, 200);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all paths were recorded correctly
        let stats = metrics.path_stats();
        assert_eq!(stats.len(), 3);
        
        let total_count: usize = stats.values().map(|(count, _, _, _)| count).sum();
        assert_eq!(total_count, 5000); // 10 threads * 500 iterations
    }
}
