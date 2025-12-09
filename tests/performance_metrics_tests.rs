#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Tests for performance metrics collection
//!
//! These tests validate the metrics structures and calculations used in the
//! performance_metrics_load_test example.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Minimal copy of PerformanceMetrics for testing
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PerformanceMetrics {
    route_match_latencies: Vec<u64>,
    dispatch_latencies: Vec<u64>,
    total_requests: u64,
    successful_matches: u64,
    match_failures: u64,
    lock_acquisition_times: Vec<u64>,
    lock_contention_time_us: u64,
    lock_contentions: u64,
    memory_samples: Vec<u64>,
    gc_delays: Vec<u64>,
    request_timestamps: Vec<u64>,
    error_counts: HashMap<u16, u64>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetricsSummary {
    avg_route_match_latency_us: f64,
    p50_route_match_latency_us: u64,
    p95_route_match_latency_us: u64,
    p99_route_match_latency_us: u64,
    max_route_match_latency_us: u64,
    avg_dispatch_latency_us: f64,
    p95_dispatch_latency_us: u64,
    p99_dispatch_latency_us: u64,
    total_requests: u64,
    successful_matches: u64,
    match_failures: u64,
    match_error_rate: f64,
    avg_lock_acquisition_us: f64,
    p99_lock_acquisition_us: u64,
    lock_contentions: u64,
    lock_contention_time_us: u64,
    avg_memory_bytes: f64,
    max_memory_bytes: u64,
    gc_delays_detected: u64,
    avg_gc_delay_us: f64,
    max_gc_delay_us: u64,
}

#[test]
fn test_performance_metrics_empty() {
    let metrics = PerformanceMetrics::new();
    let summary = metrics.calculate_summary();

    assert_eq!(summary.avg_route_match_latency_us, 0.0);
    assert_eq!(summary.p50_route_match_latency_us, 0);
    assert_eq!(summary.p95_route_match_latency_us, 0);
    assert_eq!(summary.p99_route_match_latency_us, 0);
    assert_eq!(summary.max_route_match_latency_us, 0);
    assert_eq!(summary.total_requests, 0);
    assert_eq!(summary.match_error_rate, 0.0);
}

#[test]
fn test_performance_metrics_with_data() {
    let mut metrics = PerformanceMetrics::new();

    // Add some route match latencies (in microseconds)
    metrics.route_match_latencies = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
    metrics.total_requests = 10;
    metrics.successful_matches = 9;
    metrics.match_failures = 1;

    let summary = metrics.calculate_summary();

    // Average should be 55.0
    assert_eq!(summary.avg_route_match_latency_us, 55.0);

    // P50 should be around 50-60 (50th percentile of 10 values = index 5)
    assert!((summary.p50_route_match_latency_us as i64 - 50).abs() <= 10);

    // P95 should be around 90-100
    assert!(summary.p95_route_match_latency_us >= 90);

    // P99 should be 90 or 100 (with 10 samples)
    assert!(summary.p99_route_match_latency_us >= 90);

    // Max should be 100
    assert_eq!(summary.max_route_match_latency_us, 100);

    // Error rate should be 10%
    assert_eq!(summary.match_error_rate, 10.0);

    assert_eq!(summary.total_requests, 10);
    assert_eq!(summary.successful_matches, 9);
    assert_eq!(summary.match_failures, 1);
}

#[test]
fn test_percentile_calculation() {
    let mut metrics = PerformanceMetrics::new();

    // Create 100 samples from 1 to 100
    metrics.route_match_latencies = (1..=100).collect();

    let summary = metrics.calculate_summary();

    // P50 should be around 50
    assert!((summary.p50_route_match_latency_us as i64 - 50).abs() <= 5);

    // P95 should be around 95
    assert!((summary.p95_route_match_latency_us as i64 - 95).abs() <= 5);

    // P99 should be around 99
    assert!((summary.p99_route_match_latency_us as i64 - 99).abs() <= 5);

    // Max should be 100
    assert_eq!(summary.max_route_match_latency_us, 100);
}

#[test]
fn test_lock_contention_metrics() {
    let mut metrics = PerformanceMetrics::new();

    // Add lock acquisition times
    metrics.lock_acquisition_times = vec![50, 75, 100, 150, 200];
    metrics.lock_contentions = 2; // Two slow acquisitions
    metrics.lock_contention_time_us = 350; // 150 + 200

    let summary = metrics.calculate_summary();

    assert_eq!(summary.avg_lock_acquisition_us, 115.0); // (50+75+100+150+200)/5
    assert_eq!(summary.lock_contentions, 2);
    assert_eq!(summary.lock_contention_time_us, 350);
}

#[test]
fn test_gc_delay_detection() {
    let mut metrics = PerformanceMetrics::new();

    // Simulate GC delays
    metrics.gc_delays = vec![1000, 2000, 5000]; // in microseconds

    let summary = metrics.calculate_summary();

    assert_eq!(summary.gc_delays_detected, 3);
    assert!((summary.avg_gc_delay_us - 2666.666666666667).abs() < 0.001); // (1000+2000+5000)/3
    assert_eq!(summary.max_gc_delay_us, 5000);
}

#[test]
fn test_error_rate_calculation() {
    let mut metrics = PerformanceMetrics::new();

    // Test 0% error rate
    metrics.total_requests = 100;
    metrics.successful_matches = 100;
    metrics.match_failures = 0;
    assert_eq!(metrics.calculate_summary().match_error_rate, 0.0);

    // Test 5% error rate
    metrics.total_requests = 100;
    metrics.successful_matches = 95;
    metrics.match_failures = 5;
    assert_eq!(metrics.calculate_summary().match_error_rate, 5.0);

    // Test 100% error rate
    metrics.total_requests = 100;
    metrics.successful_matches = 0;
    metrics.match_failures = 100;
    assert_eq!(metrics.calculate_summary().match_error_rate, 100.0);
}

#[test]
fn test_memory_metrics() {
    let mut metrics = PerformanceMetrics::new();

    // Add memory samples (in bytes)
    metrics.memory_samples = vec![1_000_000, 2_000_000, 3_000_000, 4_000_000, 5_000_000];

    let summary = metrics.calculate_summary();

    assert_eq!(summary.avg_memory_bytes, 3_000_000.0);
    assert_eq!(summary.max_memory_bytes, 5_000_000);
}

#[test]
fn test_dispatch_latency_metrics() {
    let mut metrics = PerformanceMetrics::new();

    // Add dispatch latencies
    metrics.dispatch_latencies = vec![10, 20, 30, 40, 50, 100, 200];

    let summary = metrics.calculate_summary();

    assert!((summary.avg_dispatch_latency_us - 64.28571428571429).abs() < 0.01); // ~64.3
    assert!(summary.p95_dispatch_latency_us >= 100);
    assert!(summary.p99_dispatch_latency_us >= 100);
}

#[test]
fn test_metrics_serialization() {
    let mut metrics = PerformanceMetrics::new();
    metrics.route_match_latencies = vec![100, 200, 300];
    metrics.total_requests = 3;
    metrics.successful_matches = 3;

    // Test that metrics can be serialized to JSON
    let json = serde_json::to_string(&metrics).expect("Failed to serialize metrics");
    assert!(json.contains("route_match_latencies"));
    assert!(json.contains("total_requests"));

    // Test that metrics can be deserialized from JSON
    let deserialized: PerformanceMetrics =
        serde_json::from_str(&json).expect("Failed to deserialize metrics");
    assert_eq!(deserialized.route_match_latencies.len(), 3);
    assert_eq!(deserialized.total_requests, 3);
}

#[test]
fn test_metrics_summary_serialization() {
    let mut metrics = PerformanceMetrics::new();
    metrics.route_match_latencies = vec![100, 200, 300];
    metrics.total_requests = 3;
    metrics.successful_matches = 3;

    let summary = metrics.calculate_summary();

    // Test that summary can be serialized to JSON
    let json = serde_json::to_string(&summary).expect("Failed to serialize summary");
    assert!(json.contains("avg_route_match_latency_us"));
    assert!(json.contains("p99_route_match_latency_us"));

    // Test that summary can be deserialized from JSON
    let deserialized: MetricsSummary =
        serde_json::from_str(&json).expect("Failed to deserialize summary");
    assert_eq!(deserialized.total_requests, 3);
}

#[test]
fn test_high_percentile_with_outliers() {
    let mut metrics = PerformanceMetrics::new();

    // Most requests are fast, but a few outliers
    let mut latencies = vec![10; 95]; // 95 fast requests
    latencies.extend(vec![100, 200, 500, 1000, 10000]); // 5 outliers
    metrics.route_match_latencies = latencies;

    let summary = metrics.calculate_summary();

    // P50 should still be low (most requests are fast)
    assert!(summary.p50_route_match_latency_us < 50);

    // P95 should be around 100 (the first outlier)
    assert!(summary.p95_route_match_latency_us >= 10);

    // P99 should catch the outliers
    assert!(summary.p99_route_match_latency_us >= 100);

    // Max should be the worst outlier
    assert_eq!(summary.max_route_match_latency_us, 10000);
}

#[test]
fn test_performance_thresholds() {
    let mut metrics = PerformanceMetrics::new();

    // Simulate excellent performance (P99 < 100µs)
    metrics.route_match_latencies = vec![20, 30, 40, 50, 60];
    let summary = metrics.calculate_summary();
    assert!(
        summary.p99_route_match_latency_us < 100,
        "Should meet excellent threshold"
    );

    // Simulate good performance (P99 < 1000µs)
    metrics.route_match_latencies = vec![100, 200, 300, 400, 500];
    let summary = metrics.calculate_summary();
    assert!(
        summary.p99_route_match_latency_us < 1000,
        "Should meet good threshold"
    );

    // Simulate poor performance (P99 > 1000µs)
    metrics.route_match_latencies = vec![500, 1000, 1500, 2000, 2500];
    let summary = metrics.calculate_summary();
    assert!(
        summary.p99_route_match_latency_us > 1000,
        "Should exceed threshold"
    );
}
