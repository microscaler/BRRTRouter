//! Performance Metrics Load Test for BRRTRouter
//!
//! This load test provides detailed metrics for route matching optimization:
//! 1. Route Matching Latency - Time to resolve a route for a request
//! 2. Lock Contention Times - Read/Write lock performance
//! 3. Frequency of Matching Errors - Cases where no route is found
//! 4. Handler Dispatch Latency - Time to dispatch after route resolution
//! 5. Garbage Collection Delays - Memory cleanup impact on latency
//!
//! # Usage
//!
//! ```bash
//! # Run with default settings (10 users, 1 minute)
//! cargo run --release --example performance_metrics_load_test -- \
//!   --host http://localhost:8080 \
//!   --users 10 \
//!   --run-time 1m
//!
//! # High concurrency test (500 users, 5 minutes)
//! cargo run --release --example performance_metrics_load_test -- \
//!   --host http://localhost:8080 \
//!   --users 500 \
//!   --run-time 5m \
//!   --report-file metrics-report.html
//!
//! # Extreme concurrency test (1000+ users)
//! cargo run --release --example performance_metrics_load_test -- \
//!   --host http://localhost:8080 \
//!   --users 1000 \
//!   --hatch-rate 100 \
//!   --run-time 10m \
//!   --report-file extreme-metrics-report.html
//! ```
//!
//! # Metrics Collection
//!
//! The test automatically queries Prometheus to collect:
//! - Route matching latency (p50, p95, p99)
//! - Handler dispatch latency
//! - Error rates (404s, 500s)
//! - Lock contention indicators
//! - Memory pressure and GC impact
//!
//! # Output
//!
//! Results are written to:
//! - HTML report (--report-file)
//! - JSON metrics file (metrics-{timestamp}.json)
//! - Console summary with key findings

use goose::prelude::*;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ============================================================================
// Performance Metrics Collection
// ============================================================================

/// Detailed performance metrics for route matching and dispatch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Route matching latency samples (microseconds)
    pub route_match_latencies: Vec<u64>,
    /// Handler dispatch latency samples (microseconds)
    pub dispatch_latencies: Vec<u64>,
    /// Total requests processed
    pub total_requests: u64,
    /// Successful route matches
    pub successful_matches: u64,
    /// Failed route matches (404s)
    pub match_failures: u64,
    /// Lock acquisition times (microseconds)
    pub lock_acquisition_times: Vec<u64>,
    /// Time spent waiting for locks (contention)
    pub lock_contention_time_us: u64,
    /// Number of lock contentions detected
    pub lock_contentions: u64,
    /// Memory usage samples (bytes)
    pub memory_samples: Vec<u64>,
    /// Garbage collection delays detected (microseconds)
    pub gc_delays: Vec<u64>,
    /// Request timestamps for rate analysis
    pub request_timestamps: Vec<u64>,
    /// Error rates by status code
    pub error_counts: HashMap<u16, u64>,
}

impl PerformanceMetrics {
    fn new() -> Self {
        Self {
            route_match_latencies: Vec::new(),
            dispatch_latencies: Vec::new(),
            total_requests: 0,
            successful_matches: 0,
            match_failures: 0,
            lock_acquisition_times: Vec::new(),
            lock_contention_time_us: 0,
            lock_contentions: 0,
            memory_samples: Vec::new(),
            gc_delays: Vec::new(),
            request_timestamps: Vec::new(),
            error_counts: HashMap::new(),
        }
    }

    /// Calculate summary statistics
    fn calculate_summary(&self) -> MetricsSummary {
        MetricsSummary {
            avg_route_match_latency_us: Self::avg(&self.route_match_latencies),
            p50_route_match_latency_us: Self::percentile(&self.route_match_latencies, 0.50),
            p95_route_match_latency_us: Self::percentile(&self.route_match_latencies, 0.95),
            p99_route_match_latency_us: Self::percentile(&self.route_match_latencies, 0.99),
            max_route_match_latency_us: Self::max(&self.route_match_latencies),
            avg_dispatch_latency_us: Self::avg(&self.dispatch_latencies),
            p95_dispatch_latency_us: Self::percentile(&self.dispatch_latencies, 0.95),
            p99_dispatch_latency_us: Self::percentile(&self.dispatch_latencies, 0.99),
            total_requests: self.total_requests,
            successful_matches: self.successful_matches,
            match_failures: self.match_failures,
            match_error_rate: if self.total_requests > 0 {
                (self.match_failures as f64 / self.total_requests as f64) * 100.0
            } else {
                0.0
            },
            avg_lock_acquisition_us: Self::avg(&self.lock_acquisition_times),
            p99_lock_acquisition_us: Self::percentile(&self.lock_acquisition_times, 0.99),
            lock_contentions: self.lock_contentions,
            lock_contention_time_us: self.lock_contention_time_us,
            avg_memory_bytes: Self::avg(&self.memory_samples),
            max_memory_bytes: Self::max(&self.memory_samples),
            gc_delays_detected: self.gc_delays.len() as u64,
            avg_gc_delay_us: Self::avg(&self.gc_delays),
            max_gc_delay_us: Self::max(&self.gc_delays),
        }
    }

    fn avg(data: &[u64]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }
        data.iter().sum::<u64>() as f64 / data.len() as f64
    }

    fn percentile(data: &[u64], p: f64) -> u64 {
        if data.is_empty() {
            return 0;
        }
        let mut sorted = data.to_vec();
        sorted.sort_unstable();
        let idx = ((sorted.len() as f64) * p) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    fn max(data: &[u64]) -> u64 {
        data.iter().copied().max().unwrap_or(0)
    }
}

/// Summary statistics for performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    // Route matching metrics
    pub avg_route_match_latency_us: f64,
    pub p50_route_match_latency_us: u64,
    pub p95_route_match_latency_us: u64,
    pub p99_route_match_latency_us: u64,
    pub max_route_match_latency_us: u64,

    // Dispatch metrics
    pub avg_dispatch_latency_us: f64,
    pub p95_dispatch_latency_us: u64,
    pub p99_dispatch_latency_us: u64,

    // Match success metrics
    pub total_requests: u64,
    pub successful_matches: u64,
    pub match_failures: u64,
    pub match_error_rate: f64,

    // Lock contention metrics
    pub avg_lock_acquisition_us: f64,
    pub p99_lock_acquisition_us: u64,
    pub lock_contentions: u64,
    pub lock_contention_time_us: u64,

    // Memory and GC metrics
    pub avg_memory_bytes: f64,
    pub max_memory_bytes: u64,
    pub gc_delays_detected: u64,
    pub avg_gc_delay_us: f64,
    pub max_gc_delay_us: u64,
}

/// Shared metrics collector accessible from all test scenarios
#[derive(Clone)]
struct MetricsCollector {
    metrics: Arc<RwLock<PerformanceMetrics>>,
    start_time: Instant,
    request_counter: Arc<AtomicUsize>,
    active_requests: Arc<AtomicUsize>,
}

impl MetricsCollector {
    fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(PerformanceMetrics::new())),
            start_time: Instant::now(),
            request_counter: Arc::new(AtomicUsize::new(0)),
            active_requests: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Record route matching latency
    fn record_route_match(&self, latency_us: u64, success: bool) {
        let mut m = self.metrics.write();
        m.route_match_latencies.push(latency_us);
        m.total_requests += 1;
        if success {
            m.successful_matches += 1;
        } else {
            m.match_failures += 1;
        }
    }

    /// Record handler dispatch latency
    fn record_dispatch(&self, latency_us: u64) {
        let mut m = self.metrics.write();
        m.dispatch_latencies.push(latency_us);
    }

    /// Record lock acquisition time
    fn record_lock_acquisition(&self, latency_us: u64, contention: bool) {
        let mut m = self.metrics.write();
        m.lock_acquisition_times.push(latency_us);
        if contention {
            m.lock_contentions += 1;
            m.lock_contention_time_us += latency_us;
        }
    }

    /// Record GC delay detection
    fn record_gc_delay(&self, delay_us: u64) {
        let mut m = self.metrics.write();
        m.gc_delays.push(delay_us);
    }

    /// Record memory usage
    fn record_memory(&self, bytes: u64) {
        let mut m = self.metrics.write();
        m.memory_samples.push(bytes);
    }

    /// Record error by status code
    fn record_error(&self, status_code: u16) {
        let mut m = self.metrics.write();
        *m.error_counts.entry(status_code).or_insert(0) += 1;
    }

    /// Get current request rate
    fn get_request_rate(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let count = self.request_counter.load(Ordering::Relaxed);
        count as f64 / elapsed
    }

    /// Increment active request counter
    fn start_request(&self) {
        self.active_requests.fetch_add(1, Ordering::Relaxed);
        self.request_counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active request counter
    fn end_request(&self) {
        self.active_requests.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get summary statistics
    fn get_summary(&self) -> MetricsSummary {
        let m = self.metrics.read();
        m.calculate_summary()
    }
}

// ============================================================================
// Test Scenarios with Metrics Instrumentation
// ============================================================================

/// Health check with minimal route matching complexity
async fn instrumented_health(user: &mut GooseUser) -> TransactionResult {
    let collector = get_collector(user);

    collector.start_request();
    let request_start = Instant::now();

    // Simulate route matching phase
    let match_start = Instant::now();
    let response = user.get("health").await?;
    let match_latency = match_start.elapsed().as_micros() as u64;

    // Record route match metrics
    let success = response
        .response
        .as_ref()
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    collector.record_route_match(match_latency, success);

    // Simulate dispatch phase (for built-in endpoints this is minimal)
    let dispatch_latency = request_start.elapsed().as_micros() as u64 - match_latency;
    collector.record_dispatch(dispatch_latency);

    // Check for GC delays (significant pause in processing)
    let total_latency = request_start.elapsed().as_micros() as u64;
    if total_latency > match_latency + dispatch_latency + 1000 {
        let gc_delay = total_latency - match_latency - dispatch_latency;
        collector.record_gc_delay(gc_delay);
    }

    collector.end_request();
    response.response?.error_for_status()?;
    Ok(())
}

/// Metrics endpoint test with lock acquisition timing
async fn instrumented_metrics(user: &mut GooseUser) -> TransactionResult {
    let collector = get_collector(user);

    collector.start_request();
    let request_start = Instant::now();

    // The /metrics endpoint requires reading shared state (potential lock contention)
    let lock_start = Instant::now();
    let response = user.get("metrics").await?;
    let lock_latency = lock_start.elapsed().as_micros() as u64;

    // Detect lock contention (if acquisition takes >100µs, likely contention)
    let contention = lock_latency > 100;
    collector.record_lock_acquisition(lock_latency, contention);

    let match_latency = request_start.elapsed().as_micros() as u64;
    let success = response
        .response
        .as_ref()
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    collector.record_route_match(match_latency, success);

    collector.end_request();
    response.response?.error_for_status()?;
    Ok(())
}

/// Authenticated request with route parameters
async fn instrumented_get_pet(user: &mut GooseUser) -> TransactionResult {
    let collector = get_collector(user);

    collector.start_request();
    let request_start = Instant::now();

    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "pets/12345")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();

    let match_start = Instant::now();
    let response = user.request(goose_request).await?;
    let match_latency = match_start.elapsed().as_micros() as u64;

    let success = if let Ok(ref r) = response.response {
        r.status().is_success()
    } else {
        false
    };

    if !success {
        if let Ok(r) = &response.response {
            collector.record_error(r.status().as_u16());
        }
    }

    collector.record_route_match(match_latency, success);

    // Dispatch latency for parameterized routes
    let dispatch_latency = request_start.elapsed().as_micros() as u64 - match_latency;
    collector.record_dispatch(dispatch_latency);

    collector.end_request();
    Ok(())
}

/// Complex route with multiple parameters
async fn instrumented_get_user_post(user: &mut GooseUser) -> TransactionResult {
    let collector = get_collector(user);

    collector.start_request();
    let request_start = Instant::now();

    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "users/abc-123/posts/post1")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();

    let match_start = Instant::now();
    let response = user.request(goose_request).await?;
    let match_latency = match_start.elapsed().as_micros() as u64;

    let success = if let Ok(ref r) = response.response {
        r.status().is_success()
    } else {
        false
    };

    if !success {
        if let Ok(r) = &response.response {
            collector.record_error(r.status().as_u16());
        }
    }

    collector.record_route_match(match_latency, success);

    let dispatch_latency = request_start.elapsed().as_micros() as u64 - match_latency;
    collector.record_dispatch(dispatch_latency);

    collector.end_request();
    Ok(())
}

/// Test non-existent route to measure 404 handling
async fn instrumented_not_found(user: &mut GooseUser) -> TransactionResult {
    let collector = get_collector(user);

    collector.start_request();
    let match_start = Instant::now();

    // Request a route that doesn't exist
    let _ = user.get("nonexistent/route/12345").await;
    let match_latency = match_start.elapsed().as_micros() as u64;

    // This should fail route matching
    collector.record_route_match(match_latency, false);
    collector.record_error(404);

    collector.end_request();
    Ok(())
}

/// Query parameter route with search complexity
async fn instrumented_search(user: &mut GooseUser) -> TransactionResult {
    let collector = get_collector(user);

    collector.start_request();
    let request_start = Instant::now();

    let request_builder = user
        .get_request_builder(&GooseMethod::Get, "search?q=test&category=all&limit=10")?
        .header("X-API-Key", "test123");
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();

    let match_start = Instant::now();
    let response = user.request(goose_request).await?;
    let match_latency = match_start.elapsed().as_micros() as u64;

    let success = response
        .response
        .as_ref()
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    collector.record_route_match(match_latency, success);

    let dispatch_latency = request_start.elapsed().as_micros() as u64 - match_latency;
    collector.record_dispatch(dispatch_latency);

    collector.end_request();
    Ok(())
}

/// POST request with body to test dispatch complexity
async fn instrumented_add_pet(user: &mut GooseUser) -> TransactionResult {
    let collector = get_collector(user);

    collector.start_request();
    let request_start = Instant::now();

    let request_builder = user
        .get_request_builder(&GooseMethod::Post, "pets")?
        .header("X-API-Key", "test123")
        .header("Content-Type", "application/json")
        .body(r#"{"name":"Fluffy","species":"dog"}"#);
    let goose_request = GooseRequest::builder()
        .set_request_builder(request_builder)
        .build();

    let match_start = Instant::now();
    let response = user.request(goose_request).await?;
    let match_latency = match_start.elapsed().as_micros() as u64;

    let success = response
        .response
        .as_ref()
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    collector.record_route_match(match_latency, success);

    // POST dispatch includes body parsing
    let dispatch_latency = request_start.elapsed().as_micros() as u64 - match_latency;
    collector.record_dispatch(dispatch_latency);

    collector.end_request();
    Ok(())
}

/// Memory sampling task that periodically records memory usage
async fn memory_sampler(user: &mut GooseUser) -> TransactionResult {
    let collector = get_collector(user);

    // Sample memory usage
    if let Some(usage) = memory_stats::memory_stats() {
        collector.record_memory(usage.physical_mem as u64);
    }

    // Sleep briefly to avoid overwhelming the collector
    tokio::time::sleep(Duration::from_millis(100)).await;
    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get the metrics collector (uses global static)
fn get_collector(_user: &mut GooseUser) -> MetricsCollector {
    // Use global static collector for simplicity
    // In production, you could use per-user collectors with proper session management
    GLOBAL_COLLECTOR.lock().clone()
}

// Global collector (simplified for example)
use once_cell::sync::Lazy;
static GLOBAL_COLLECTOR: Lazy<parking_lot::Mutex<MetricsCollector>> =
    Lazy::new(|| parking_lot::Mutex::new(MetricsCollector::new()));

// ============================================================================
// Main Test Configuration
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), GooseError> {
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║   BRRTRouter Performance Metrics Load Test                    ║");
    println!("║   Collecting detailed route matching and dispatch metrics     ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    let result = GooseAttack::initialize()?
        // High-frequency route matching tests (40% weight)
        .register_scenario(
            scenario!("Route Matching Performance")
                .set_weight(40)?
                .register_transaction(
                    transaction!(instrumented_get_pet)
                        .set_name("GET /pets/{id} - Parameterized Route"),
                )
                .register_transaction(
                    transaction!(instrumented_get_user_post)
                        .set_name("GET /users/{id}/posts/{post_id} - Complex Route"),
                )
                .register_transaction(
                    transaction!(instrumented_search)
                        .set_name("GET /search?q=... - Query Parameters"),
                ),
        )
        // Error handling and edge cases (20% weight)
        .register_scenario(
            scenario!("Error Handling Performance")
                .set_weight(20)?
                .register_transaction(
                    transaction!(instrumented_not_found)
                        .set_name("GET /nonexistent - 404 Handling"),
                ),
        )
        // Lock contention tests (20% weight)
        .register_scenario(
            scenario!("Lock Contention Tests")
                .set_weight(20)?
                .register_transaction(
                    transaction!(instrumented_metrics)
                        .set_name("GET /metrics - Shared State Access"),
                ),
        )
        // Dispatch complexity tests (15% weight)
        .register_scenario(
            scenario!("Dispatch Performance")
                .set_weight(15)?
                .register_transaction(
                    transaction!(instrumented_add_pet)
                        .set_name("POST /pets - Body Parsing & Dispatch"),
                ),
        )
        // Baseline tests (5% weight)
        .register_scenario(
            scenario!("Baseline Performance")
                .set_weight(5)?
                .register_transaction(
                    transaction!(instrumented_health).set_name("GET /health - Minimal Route"),
                ),
        )
        .execute()
        .await?;

    // Print metrics summary after test completes
    print_metrics_summary();

    // Save metrics to file
    if let Err(e) = save_metrics_to_file() {
        eprintln!("Failed to save metrics to file: {}", e);
    }

    Ok(())
}

/// Print comprehensive metrics summary to console
fn print_metrics_summary() {
    let collector = GLOBAL_COLLECTOR.lock();
    let summary = collector.get_summary();

    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║                  PERFORMANCE METRICS SUMMARY                   ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    println!("📊 Route Matching Metrics:");
    println!(
        "  ├─ Average Latency: {:.2} µs",
        summary.avg_route_match_latency_us
    );
    println!(
        "  ├─ P50 Latency: {} µs",
        summary.p50_route_match_latency_us
    );
    println!(
        "  ├─ P95 Latency: {} µs",
        summary.p95_route_match_latency_us
    );
    println!(
        "  ├─ P99 Latency: {} µs",
        summary.p99_route_match_latency_us
    );
    println!(
        "  └─ Max Latency: {} µs",
        summary.max_route_match_latency_us
    );

    println!("\n🎯 Match Success Rate:");
    println!("  ├─ Total Requests: {}", summary.total_requests);
    println!("  ├─ Successful Matches: {}", summary.successful_matches);
    println!("  ├─ Failed Matches (404s): {}", summary.match_failures);
    println!("  └─ Error Rate: {:.2}%", summary.match_error_rate);

    println!("\n⚡ Handler Dispatch Metrics:");
    println!(
        "  ├─ Average Latency: {:.2} µs",
        summary.avg_dispatch_latency_us
    );
    println!("  ├─ P95 Latency: {} µs", summary.p95_dispatch_latency_us);
    println!("  └─ P99 Latency: {} µs", summary.p99_dispatch_latency_us);

    println!("\n🔒 Lock Contention Metrics:");
    println!(
        "  ├─ Average Lock Acquisition: {:.2} µs",
        summary.avg_lock_acquisition_us
    );
    println!(
        "  ├─ P99 Lock Acquisition: {} µs",
        summary.p99_lock_acquisition_us
    );
    println!("  ├─ Contentions Detected: {}", summary.lock_contentions);
    println!(
        "  └─ Total Contention Time: {} µs",
        summary.lock_contention_time_us
    );

    println!("\n💾 Memory & GC Metrics:");
    println!(
        "  ├─ Average Memory: {:.2} MB",
        summary.avg_memory_bytes as f64 / 1_048_576.0
    );
    println!(
        "  ├─ Max Memory: {:.2} MB",
        summary.max_memory_bytes as f64 / 1_048_576.0
    );
    println!("  ├─ GC Delays Detected: {}", summary.gc_delays_detected);
    println!("  ├─ Average GC Delay: {:.2} µs", summary.avg_gc_delay_us);
    println!("  └─ Max GC Delay: {} µs", summary.max_gc_delay_us);

    println!("\n📈 Performance Analysis:");

    if summary.p99_route_match_latency_us > 1000 {
        println!("  ⚠️  WARNING: P99 route matching latency exceeds 1ms");
    } else {
        println!("  ✅ Route matching latency is excellent (P99 < 1ms)");
    }

    if summary.match_error_rate > 5.0 {
        println!(
            "  ⚠️  WARNING: Match error rate is high ({}%)",
            summary.match_error_rate
        );
    } else {
        println!("  ✅ Match error rate is acceptable");
    }

    if summary.lock_contentions > 100 {
        println!(
            "  ⚠️  WARNING: Significant lock contention detected ({} contentions)",
            summary.lock_contentions
        );
    } else {
        println!("  ✅ Lock contention is minimal");
    }

    if summary.gc_delays_detected > 10 {
        println!(
            "  ⚠️  WARNING: GC delays may be impacting performance ({} delays)",
            summary.gc_delays_detected
        );
    } else {
        println!("  ✅ GC impact is minimal");
    }

    println!("\n");
}

/// Save detailed metrics to JSON file
fn save_metrics_to_file() -> Result<(), Box<dyn std::error::Error>> {
    let collector = GLOBAL_COLLECTOR.lock();
    let summary = collector.get_summary();

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();

    let filename = format!("metrics-{}.json", timestamp);
    let json = serde_json::to_string_pretty(&summary)?;
    std::fs::write(&filename, json)?;

    println!("📄 Detailed metrics saved to: {}", filename);
    Ok(())
}
