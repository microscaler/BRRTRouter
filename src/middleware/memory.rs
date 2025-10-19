//! Memory tracking middleware for OpenTelemetry integration
//!
//! This module provides comprehensive memory usage tracking that integrates
//! with OpenTelemetry metrics for Grafana/Prometheus visibility.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use memory_stats::memory_stats;

use crate::dispatcher::{HandlerRequest, HandlerResponse};
use crate::middleware::Middleware;

/// Get current process memory statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryStats {
    /// Resident Set Size (physical memory in use) in bytes
    pub rss_bytes: u64,
    /// Virtual memory size in bytes
    pub vss_bytes: u64,
    /// Heap allocated bytes (from allocator)
    pub heap_bytes: u64,
    /// Number of active allocations
    pub allocations: u64,
}

impl MemoryStats {
    /// Get current memory statistics for the process
    /// Returns ONLY real measured values, no estimates or fake data
    pub fn current() -> Self {
        let mut stats = MemoryStats::default();
        
        // Use memory-stats to get real memory usage
        // This works on Linux, macOS, and Windows
        if let Some(usage) = memory_stats() {
            // memory-stats provides actual physical and virtual memory in bytes
            stats.rss_bytes = usage.physical_mem as u64;
            stats.vss_bytes = usage.virtual_mem as u64;
            
            // We cannot accurately measure heap without allocator instrumentation
            // Set to 0 to indicate "not measured" rather than guessing
            stats.heap_bytes = 0;
            
            // Allocations count is not available without allocator hooks
            stats.allocations = 0;
        } else {
            // If we can't get real stats, report zeros - NOT fake data
            eprintln!("[memory] ERROR: Unable to get memory statistics for this platform");
            // Return all zeros to indicate measurement failure
            // Caller should handle this appropriately
        }
        
        stats
    }
}

/// Memory tracking middleware that exports metrics to OpenTelemetry
pub struct MemoryMiddleware {
    /// Baseline memory at service start
    baseline: MemoryStats,
    
    /// Current memory statistics
    current: RwLock<MemoryStats>,
    
    /// Peak memory statistics
    peak: RwLock<MemoryStats>,
    
    /// Memory growth since baseline (bytes)
    growth_bytes: AtomicU64,
    
    /// Number of measurements taken
    measurements: AtomicUsize,
    
    /// Per-handler memory usage tracking
    handler_memory: Arc<RwLock<HashMap<String, HandlerMemoryStats>>>,
    
    /// Last measurement time
    last_measurement: RwLock<Instant>,
}

#[derive(Debug, Clone, Default)]
struct HandlerMemoryStats {
    /// Total memory allocated by this handler
    total_allocated: u64,
    /// Number of invocations
    invocations: u64,
    /// Peak memory usage
    peak_usage: u64,
}

impl MemoryMiddleware {
    /// Create a new memory tracking middleware
    pub fn new() -> Self {
        let baseline = MemoryStats::current();
        
        Self {
            baseline,
            current: RwLock::new(baseline),
            peak: RwLock::new(baseline),
            growth_bytes: AtomicU64::new(0),
            measurements: AtomicUsize::new(0),
            handler_memory: Arc::new(RwLock::new(HashMap::new())),
            last_measurement: RwLock::new(Instant::now()),
        }
    }
    
    /// Update memory statistics
    pub fn update(&self) {
        let stats = MemoryStats::current();
        
        // Update current stats
        *self.current.write().unwrap() = stats;
        
        // Update peak if necessary
        let mut peak = self.peak.write().unwrap();
        if stats.rss_bytes > peak.rss_bytes {
            peak.rss_bytes = stats.rss_bytes;
        }
        if stats.heap_bytes > peak.heap_bytes {
            peak.heap_bytes = stats.heap_bytes;
        }
        
        // Calculate growth
        let growth = stats.rss_bytes.saturating_sub(self.baseline.rss_bytes);
        self.growth_bytes.store(growth, Ordering::Relaxed);
        
        // Increment measurement counter
        self.measurements.fetch_add(1, Ordering::Relaxed);
        
        // Update last measurement time
        *self.last_measurement.write().unwrap() = Instant::now();
    }
    
    /// Get current memory statistics
    pub fn current_stats(&self) -> MemoryStats {
        *self.current.read().unwrap()
    }
    
    /// Get peak memory statistics
    pub fn peak_stats(&self) -> MemoryStats {
        *self.peak.read().unwrap()
    }
    
    /// Get memory growth since baseline
    pub fn growth_bytes(&self) -> u64 {
        self.growth_bytes.load(Ordering::Relaxed)
    }
    
    /// Export metrics in Prometheus format
    pub fn export_metrics(&self) -> String {
        self.update(); // Ensure fresh data
        
        let current = self.current_stats();
        let peak = self.peak_stats();
        let growth = self.growth_bytes();
        
        let mut output = String::with_capacity(2048);
        
        // Current memory metrics
        output.push_str("# HELP process_memory_rss_bytes Resident Set Size in bytes\n");
        output.push_str("# TYPE process_memory_rss_bytes gauge\n");
        output.push_str(&format!("process_memory_rss_bytes {}\n", current.rss_bytes));
        
        output.push_str("# HELP process_memory_vss_bytes Virtual memory size in bytes\n");
        output.push_str("# TYPE process_memory_vss_bytes gauge\n");
        output.push_str(&format!("process_memory_vss_bytes {}\n", current.vss_bytes));
        
        // Only export heap metrics if we have real data
        if current.heap_bytes > 0 {
            output.push_str("# HELP process_memory_heap_bytes Heap allocated bytes\n");
            output.push_str("# TYPE process_memory_heap_bytes gauge\n");
            output.push_str(&format!("process_memory_heap_bytes {}\n", current.heap_bytes));
        }
        
        // Peak memory metrics
        output.push_str("# HELP process_memory_peak_rss_bytes Peak RSS in bytes\n");
        output.push_str("# TYPE process_memory_peak_rss_bytes gauge\n");
        output.push_str(&format!("process_memory_peak_rss_bytes {}\n", peak.rss_bytes));
        
        // Only export peak heap if we have real data
        if peak.heap_bytes > 0 {
            output.push_str("# HELP process_memory_peak_heap_bytes Peak heap in bytes\n");
            output.push_str("# TYPE process_memory_peak_heap_bytes gauge\n");
            output.push_str(&format!("process_memory_peak_heap_bytes {}\n", peak.heap_bytes));
        }
        
        // Growth metrics
        output.push_str("# HELP process_memory_growth_bytes Memory growth since startup\n");
        output.push_str("# TYPE process_memory_growth_bytes gauge\n");
        output.push_str(&format!("process_memory_growth_bytes {}\n", growth));
        
        // Baseline metrics
        output.push_str("# HELP process_memory_baseline_rss_bytes RSS at startup\n");
        output.push_str("# TYPE process_memory_baseline_rss_bytes gauge\n");
        output.push_str(&format!("process_memory_baseline_rss_bytes {}\n", self.baseline.rss_bytes));
        
        // Per-handler invocation counts (we can't accurately measure per-handler memory)
        let handler_stats = self.handler_memory.read().unwrap();
        if !handler_stats.is_empty() {
            output.push_str("# HELP handler_invocations_total Number of invocations per handler\n");
            output.push_str("# TYPE handler_invocations_total counter\n");
            
            for (handler, stats) in handler_stats.iter() {
                output.push_str(&format!(
                    "handler_invocations_total{{handler=\"{}\"}} {}\n",
                    handler, stats.invocations
                ));
            }
        }
        
        // Measurement metadata
        output.push_str("# HELP memory_measurements_total Number of memory measurements taken\n");
        output.push_str("# TYPE memory_measurements_total counter\n");
        output.push_str(&format!(
            "memory_measurements_total {}\n",
            self.measurements.load(Ordering::Relaxed)
        ));
        
        output
    }
    
    /// Log memory statistics with tracing
    pub fn log_stats(&self) {
        let current = self.current_stats();
        let growth = self.growth_bytes();
        
        // Warn if memory is growing rapidly
        let growth_mb = growth / (1024 * 1024);
        if growth_mb > 100 {
            tracing::warn!(
                rss_mb = current.rss_bytes / (1024 * 1024),
                heap_mb = current.heap_bytes / (1024 * 1024),
                growth_mb = growth_mb,
                "High memory growth detected"
            );
        } else {
            tracing::info!(
                rss_mb = current.rss_bytes / (1024 * 1024),
                heap_mb = current.heap_bytes / (1024 * 1024),
                growth_mb = growth_mb,
                "Memory statistics"
            );
        }
    }
}

impl Default for MemoryMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl Middleware for MemoryMiddleware {
    fn before(&self, _req: &HandlerRequest) -> Option<HandlerResponse> {
        // Record memory before handler execution
        let _before = MemoryStats::current();
        
        // Store in request context (would need request context support)
        // For now, just update general stats
        self.update();
        
        None
    }
    
    fn after(&self, req: &HandlerRequest, _res: &mut HandlerResponse, _latency: Duration) {
        // Update overall memory stats after handler execution
        self.update();
        
        // Track handler invocation count
        let mut handler_stats = self.handler_memory.write().unwrap();
        let stats = handler_stats
            .entry(req.handler_name.clone())
            .or_insert_with(HandlerMemoryStats::default);
        
        stats.invocations += 1;
        
        // NOTE: We cannot accurately attribute memory to specific handlers
        // without request-scoped allocator tracking. We only track invocation
        // counts here. Memory growth patterns must be analyzed at the 
        // aggregate level.
        
        // Periodic logging (every 100 requests)
        if self.measurements.load(Ordering::Relaxed) % 100 == 0 {
            self.log_stats();
        }
    }
}

/// Background task to periodically update memory metrics
pub fn start_memory_monitor(middleware: Arc<MemoryMiddleware>) {
    std::thread::spawn(move || {
        loop {
            // Update every 10 seconds
            std::thread::sleep(Duration::from_secs(10));
            
            middleware.update();
            middleware.log_stats();
            
            // Export to OpenTelemetry (would need OTLP client)
            // For now, metrics are available via /metrics endpoint
        }
    });
}
