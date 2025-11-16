# Performance Metrics Collection Guide

## Overview

This guide explains the detailed performance metrics collection system for BRRTRouter's route matching and dispatching functionality. These metrics are essential for identifying performance bottlenecks and guiding optimization efforts.

## Metrics Categories

### 1. Route Matching Latency

**Definition**: Time taken to resolve a route for a given HTTP request, from path parsing to handler lookup.

**Measured in**: Microseconds (µs)

**Key Indicators**:
- **Average**: Typical route matching time
- **P50 (Median)**: 50% of requests are faster than this
- **P95**: 95% of requests are faster than this (good performance target)
- **P99**: 99% of requests are faster than this (tail latency)
- **Max**: Worst-case latency observed

**Performance Targets**:
- ✅ Excellent: P99 < 100µs (0.1ms)
- ⚠️ Good: P99 < 1000µs (1ms)
- ❌ Poor: P99 > 1000µs (1ms)

**Optimization Opportunities**:
- If P99 > 1ms: Consider radix tree depth optimization
- If Max >> P99: Investigate occasional GC pauses or lock contention
- If Average is high but P50 is low: Look for specific slow routes

**How It's Measured**:
```rust
let match_start = Instant::now();
let result = router.route(method, path);
let match_latency = match_start.elapsed().as_micros() as u64;
```

### 2. Lock Contention Times

**Definition**: Time spent waiting to acquire read/write locks during route matching and metrics collection.

**Measured in**: Microseconds (µs)

**Key Indicators**:
- **Average Lock Acquisition Time**: Typical time to get a lock
- **P99 Lock Acquisition Time**: 99th percentile lock wait time
- **Contention Events**: Number of times lock acquisition took > 100µs
- **Total Contention Time**: Cumulative time lost to lock waiting

**Performance Targets**:
- ✅ Excellent: P99 < 50µs, < 10 contentions/1000 requests
- ⚠️ Good: P99 < 100µs, < 50 contentions/1000 requests
- ❌ Poor: P99 > 100µs, > 100 contentions/1000 requests

**Optimization Opportunities**:
- High contention on metrics: Use lock-free counters (atomic operations)
- High contention on router: Ensure router is read-only after initialization
- High contention on dispatcher: Consider per-handler channels instead of global lock

**How It's Measured**:
```rust
let lock_start = Instant::now();
let data = shared_state.read(); // or write()
let lock_latency = lock_start.elapsed().as_micros() as u64;
let contention = lock_latency > 100; // Threshold for contention
```

### 3. Frequency of Matching Errors

**Definition**: Rate at which route matching fails (typically resulting in 404 responses).

**Measured in**: Percentage of total requests

**Key Indicators**:
- **Total Failed Matches**: Count of 404 responses
- **Match Error Rate**: (Failed / Total) × 100%
- **Error Distribution by Route**: Which paths have highest 404 rates

**Performance Targets**:
- ✅ Excellent: < 1% error rate (production traffic)
- ⚠️ Good: < 5% error rate (includes testing)
- ❌ Poor: > 5% error rate (indicates routing issues)

**Optimization Opportunities**:
- High 404 rate on valid routes: Bug in route matching logic
- High 404 rate on invalid routes: Expected behavior, but ensure 404 path is fast
- Spikes in 404s: Potential attack or misconfiguration

**How It's Measured**:
```rust
let success = router.route(method, path).is_some();
if !success {
    metrics.match_failures += 1;
    metrics.error_counts.entry(404).or_insert(0) += 1;
}
```

### 4. Handler Dispatch Latency

**Definition**: Time spent after route resolution to dispatch the request to the appropriate handler, including parameter extraction and channel communication.

**Measured in**: Microseconds (µs)

**Key Indicators**:
- **Average Dispatch Time**: Typical dispatch overhead
- **P95 Dispatch Time**: 95th percentile
- **P99 Dispatch Time**: 99th percentile

**Performance Targets**:
- ✅ Excellent: P99 < 50µs
- ⚠️ Good: P99 < 200µs
- ❌ Poor: P99 > 500µs

**Optimization Opportunities**:
- High dispatch time: Optimize parameter extraction
- Variability in dispatch: Check channel buffer sizes
- High P99 with low average: Investigate handler pool saturation

**How It's Measured**:
```rust
let request_start = Instant::now();
let match_latency = /* time for route matching */;
// ... dispatch to handler ...
let dispatch_latency = request_start.elapsed().as_micros() as u64 - match_latency;
```

### 5. Garbage Collection Delays

**Definition**: Delays in request processing attributed to garbage collection pauses (detected when total latency significantly exceeds route matching + dispatch time).

**Measured in**: Microseconds (µs)

**Key Indicators**:
- **GC Delays Detected**: Count of suspected GC pauses
- **Average GC Delay**: Mean pause duration
- **Max GC Delay**: Worst pause observed

**Performance Targets**:
- ✅ Excellent: < 5 GC delays per 1000 requests, max < 1ms
- ⚠️ Good: < 20 GC delays per 1000 requests, max < 10ms
- ❌ Poor: > 50 GC delays per 1000 requests, max > 10ms

**Optimization Opportunities**:
- Frequent GC delays: Reduce allocations in hot paths
- Large GC delays: Tune memory limits or use jemalloc
- GC during high load: Consider pre-allocating resources

**How It's Measured**:
```rust
let expected_time = match_latency + dispatch_latency;
let actual_time = request_start.elapsed().as_micros() as u64;
if actual_time > expected_time + 1000 {
    let gc_delay = actual_time - expected_time;
    metrics.gc_delays.push(gc_delay);
}
```

## Running the Performance Tests

### Basic Test (Development)

```bash
# Quick 1-minute test with 10 concurrent users
cargo run --release --example performance_metrics_load_test -- \
  --host http://localhost:8080 \
  --users 10 \
  --run-time 1m
```

### Standard Performance Test

```bash
# 5-minute test with 100 concurrent users
cargo run --release --example performance_metrics_load_test -- \
  --host http://localhost:8080 \
  --users 100 \
  --hatch-rate 10 \
  --run-time 5m \
  --report-file performance-report.html
```

### High Concurrency Test

```bash
# 10-minute test with 500 concurrent users
cargo run --release --example performance_metrics_load_test -- \
  --host http://localhost:8080 \
  --users 500 \
  --hatch-rate 50 \
  --run-time 10m \
  --report-file high-concurrency-report.html
```

### Extreme Load Test

```bash
# 15-minute test with 1000+ concurrent users (find breaking point)
cargo run --release --example performance_metrics_load_test -- \
  --host http://localhost:8080 \
  --users 1000 \
  --hatch-rate 100 \
  --run-time 15m \
  --report-file extreme-load-report.html
```

## Interpreting Results

### Output Files

After each test run, you'll get:

1. **Console Summary**: Real-time metrics with performance indicators
2. **HTML Report**: Goose-generated report with request/response stats
3. **JSON Metrics**: Detailed metrics file (`metrics-{timestamp}.json`)

### Sample Output Interpretation

```
📊 Route Matching Metrics:
  ├─ Average Latency: 45.23 µs      ← Typical route match time
  ├─ P50 Latency: 42 µs              ← Half of requests are faster
  ├─ P95 Latency: 89 µs              ← 95% are faster (key metric)
  ├─ P99 Latency: 156 µs             ← 99% are faster (tail latency)
  └─ Max Latency: 3421 µs            ← Worst case (investigate if >> P99)

📈 Performance Analysis:
  ✅ Route matching latency is excellent (P99 < 1ms)
  ⚠️  WARNING: GC delays may be impacting performance (23 delays)
```

**What This Tells You**:
- Route matching is fast (P99 < 1ms) ✅
- But occasional GC pauses (max 3.4ms) are creating tail latency ⚠️
- **Action**: Investigate memory allocations in route matching code

### Common Performance Patterns

#### Pattern 1: Consistent Performance
```
P50: 40µs, P95: 85µs, P99: 120µs, Max: 150µs
```
**Interpretation**: Stable performance, no outliers
**Action**: ✅ No action needed

#### Pattern 2: Tail Latency Spikes
```
P50: 40µs, P95: 90µs, P99: 450µs, Max: 5000µs
```
**Interpretation**: Most requests fast, but some are very slow
**Action**: ⚠️ Investigate GC delays, lock contention, or specific slow routes

#### Pattern 3: High Baseline
```
P50: 800µs, P95: 1200µs, P99: 2500µs, Max: 3000µs
```
**Interpretation**: All requests are slow
**Action**: ❌ Optimize core route matching algorithm

## Integration with Prometheus

The metrics collected by the load test complement Prometheus metrics exposed by BRRTRouter:

### Key Prometheus Queries

```promql
# P99 Route Matching Latency
histogram_quantile(0.99, rate(brrtrouter_route_match_duration_seconds_bucket[5m]))

# Error Rate
100 * (
  sum(rate(brrtrouter_requests_total{status!~"2.."}[5m]))
  /
  sum(rate(brrtrouter_requests_total[5m]))
)

# Active Requests (Lock Contention Indicator)
brrtrouter_active_requests

# Memory Usage
brrtrouter_memory_usage_bytes
```

### Correlating Load Test with Prometheus

1. Run the load test
2. Note the timestamp
3. Query Prometheus for the same time range
4. Compare metrics:
   - Load test P99 vs Prometheus P99
   - Load test error rate vs Prometheus error rate
   - Identify discrepancies

## Benchmark Report Generation

### Comparing Test Runs

```bash
# Run baseline test
cargo run --release --example performance_metrics_load_test -- \
  --host http://localhost:8080 \
  --users 100 \
  --run-time 5m

# Save baseline metrics
cp metrics-*.json baseline-metrics.json

# Make code changes...

# Run comparison test
cargo run --release --example performance_metrics_load_test -- \
  --host http://localhost:8080 \
  --users 100 \
  --run-time 5m

# Compare
python3 scripts/compare_metrics.py baseline-metrics.json metrics-*.json
```

### Creating a Benchmark Report

A complete benchmark report should include:

1. **Test Configuration**
   - User count, duration, ramp rate
   - Hardware specs (CPU, RAM)
   - Software versions (Rust, dependencies)

2. **Metrics Summary**
   - Route matching latency (P50, P95, P99)
   - Error rates
   - Lock contention stats
   - GC impact

3. **Performance Analysis**
   - Bottlenecks identified
   - Comparison to previous runs
   - Recommendations

4. **Raw Data**
   - Link to JSON metrics files
   - Prometheus dashboard snapshots
   - HTML reports

## Advanced Usage

### Custom Metrics Collection

You can extend the metrics collector to track additional metrics:

```rust
// In performance_metrics_load_test.rs

impl PerformanceMetrics {
    pub fn record_custom_metric(&mut self, name: &str, value: u64) {
        // Add to custom_metrics HashMap
        self.custom_metrics.entry(name.to_string())
            .or_insert(Vec::new())
            .push(value);
    }
}
```

### Automated Performance Regression Detection

Set up CI to run performance tests and fail if metrics regress:

```bash
# In .github/workflows/performance.yml

# Run test
cargo run --release --example performance_metrics_load_test -- \
  --host http://localhost:8080 \
  --users 100 \
  --run-time 2m

# Check P99 latency
python3 scripts/check_regression.py metrics-*.json \
  --max-p99-latency 1000 \
  --max-error-rate 1.0
```

## Troubleshooting

### Problem: No metrics collected
**Cause**: Global collector not properly initialized
**Solution**: Ensure `GLOBAL_COLLECTOR` is accessed in all test functions

### Problem: GC delays always zero
**Cause**: Detection threshold too high
**Solution**: Adjust threshold in `instrumented_*` functions (currently 1000µs)

### Problem: Lock contention not detected
**Cause**: Insufficient load or too fast locks
**Solution**: Increase user count or decrease contention threshold

### Problem: Metrics file not created
**Cause**: Permission error or disk full
**Solution**: Check write permissions and disk space

## Best Practices

1. **Run Consistently**: Use same hardware and configuration for comparisons
2. **Warm Up**: Add a warm-up period (first 30s) before collecting metrics
3. **Isolate Tests**: Run on dedicated hardware without other processes
4. **Multiple Runs**: Run at least 3 times and take median/average
5. **Document Changes**: Always note code changes between benchmark runs
6. **Monitor Resources**: Track CPU, memory, and disk I/O during tests

## Further Reading

- [Goose Load Testing Documentation](https://book.goose.rs/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [OpenTelemetry Metrics](https://opentelemetry.io/docs/concepts/signals/metrics/)
- [BRRTRouter Architecture](./ARCHITECTURE.md)

## Contributing

If you improve the metrics collection system:

1. Update this documentation
2. Add tests for new metrics
3. Update example outputs
4. Submit a PR with benchmark comparisons

---

**Last Updated**: 2025-11-16
**Version**: 1.0
**Maintainer**: BRRTRouter Team
