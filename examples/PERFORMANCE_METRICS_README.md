# Performance Metrics Load Testing

This directory contains comprehensive performance metrics load testing for BRRTRouter's route matching and dispatch functionality.

## Quick Start

### Run Basic Performance Test

```bash
# Start your BRRTRouter server first
cargo run --release --example pet_store -- \
  --spec examples/pet_store/doc/openapi.yaml \
  --addr 0.0.0.0:8080

# In another terminal, run the performance test
cargo run --release --example performance_metrics_load_test -- \
  --host http://localhost:8080 \
  --users 100 \
  --run-time 5m
```

### Generate Comprehensive Benchmark Report

```bash
# Automated report generation with all metrics
python3 scripts/generate_benchmark_report.py

# View the report
cat benchmark-reports/*/README.md
```

## Available Tests

### 1. `performance_metrics_load_test.rs`

Comprehensive load test with detailed instrumentation for:

- **Route Matching Latency** - Measures time to resolve routes (µs)
- **Lock Contention** - Tracks read/write lock acquisition times
- **Match Error Frequency** - Records 404 rates and error patterns
- **Handler Dispatch Latency** - Time from route match to handler execution
- **GC Delay Detection** - Identifies garbage collection impacts

**Test Scenarios:**
- Route Matching Performance (40%) - Complex parameterized routes
- Error Handling Performance (20%) - 404 and error cases
- Lock Contention Tests (20%) - Shared state access
- Dispatch Performance (15%) - Body parsing and dispatch
- Baseline Performance (5%) - Minimal routes

**Output:**
- Console summary with performance indicators (✅ ⚠️ ❌)
- JSON metrics file with detailed statistics
- HTML report from Goose

### 2. `api_load_test.rs`

Standard Goose load test covering all API endpoints with authentication.

**Use for:** General load testing and API validation

### 3. `adaptive_load_test.rs`

Automatically finds the breaking point by gradually increasing load.

**Use for:** Capacity planning and finding performance limits

## Scripts

### Compare Metrics Between Runs

```bash
# Run baseline
cargo run --release --example performance_metrics_load_test -- \
  --host http://localhost:8080 --users 100 --run-time 5m

# Save baseline
cp metrics-*.json baseline-metrics.json

# Make code changes...

# Run new test
cargo run --release --example performance_metrics_load_test -- \
  --host http://localhost:8080 --users 100 --run-time 5m

# Compare
python3 scripts/compare_metrics.py baseline-metrics.json metrics-*.json
```

### Generate Benchmark Report

```bash
# Basic report
python3 scripts/generate_benchmark_report.py

# With options
python3 scripts/generate_benchmark_report.py \
  --users 500 \
  --run-time 10m \
  --output-dir ./reports \
  --baseline baseline-metrics.json
```

## Metrics Reference

### Route Matching Latency

**What it measures:** Time to resolve a route from HTTP request to handler lookup

**Performance Targets:**
- ✅ Excellent: P99 < 100µs
- ⚠️ Good: P99 < 1000µs (1ms)
- ❌ Poor: P99 > 1000µs

**Optimization opportunities:**
- Radix tree depth optimization
- Parameter extraction efficiency
- Regex compilation caching

### Lock Contention Times

**What it measures:** Time waiting to acquire read/write locks

**Performance Targets:**
- ✅ Excellent: P99 < 50µs, < 10 contentions/1000 req
- ⚠️ Good: P99 < 100µs, < 50 contentions/1000 req
- ❌ Poor: P99 > 100µs, > 100 contentions/1000 req

**Optimization opportunities:**
- Lock-free atomic counters
- Read-only router after init
- Per-handler channels

### Match Error Frequency

**What it measures:** Rate of route matching failures (404s)

**Performance Targets:**
- ✅ Excellent: < 1% (production traffic)
- ⚠️ Good: < 5% (includes testing)
- ❌ Poor: > 5%

**Optimization opportunities:**
- Fast 404 path
- Route validation
- Error monitoring

### Handler Dispatch Latency

**What it measures:** Time from route match to handler execution

**Performance Targets:**
- ✅ Excellent: P99 < 50µs
- ⚠️ Good: P99 < 200µs
- ❌ Poor: P99 > 500µs

**Optimization opportunities:**
- Parameter extraction
- Channel buffer sizes
- Handler pool sizing

### GC Delay Detection

**What it measures:** Garbage collection pause impact on latency

**Performance Targets:**
- ✅ Excellent: < 5 delays/1000 req, max < 1ms
- ⚠️ Good: < 20 delays/1000 req, max < 10ms
- ❌ Poor: > 50 delays/1000 req, max > 10ms

**Optimization opportunities:**
- Reduce hot path allocations
- Use jemalloc
- Pre-allocate resources

## Test Configurations

### Development (Quick Check)
```bash
--users 10 --run-time 1m
```

### CI/CD (Automated Testing)
```bash
--users 50 --run-time 2m
```

### Standard Performance Test
```bash
--users 100 --run-time 5m
```

### High Concurrency
```bash
--users 500 --run-time 10m
```

### Extreme Load (Breaking Point)
```bash
--users 1000 --run-time 15m --hatch-rate 100
```

## Integration Tests

The metrics collection system is validated with integration tests:

```bash
cargo test --test performance_metrics_tests
```

Tests cover:
- Percentile calculations
- Error rate calculations
- Lock contention tracking
- GC delay detection
- Metrics serialization
- Performance thresholds

## Documentation

See [docs/PERFORMANCE_METRICS.md](../docs/PERFORMANCE_METRICS.md) for:
- Detailed metric definitions
- Performance optimization guide
- Integration with Prometheus
- Best practices
- Troubleshooting

## CI/CD Integration

Add to your CI pipeline:

```yaml
# .github/workflows/performance.yml
- name: Run Performance Test
  run: |
    # Start server
    cargo run --release --example pet_store &
    sleep 5
    
    # Run test
    cargo run --release --example performance_metrics_load_test -- \
      --host http://localhost:8080 \
      --users 50 \
      --run-time 2m
    
    # Check regression
    python3 scripts/compare_metrics.py \
      baseline-metrics.json \
      metrics-*.json \
      --max-regression 10 \
      --fail-on-regression
```

## Tips

1. **Always warm up** - First requests are slower due to lazy initialization
2. **Run multiple times** - Take median of 3+ runs for consistency
3. **Isolate tests** - Run on dedicated hardware without other processes
4. **Compare apples to apples** - Use same configuration when comparing
5. **Document changes** - Note what changed between test runs
6. **Monitor system** - Watch CPU, memory, and disk I/O during tests

## Troubleshooting

**Problem:** No metrics collected

**Solution:** Check that GLOBAL_COLLECTOR is properly initialized

---

**Problem:** Test fails to connect to server

**Solution:** Ensure server is running: `curl http://localhost:8080/health`

---

**Problem:** Metrics seem incorrect

**Solution:** Verify test configuration matches server capacity

---

**Problem:** High variance in results

**Solution:** Increase test duration or reduce system load

## Contributing

When adding new metrics:

1. Update `PerformanceMetrics` struct
2. Add collection code in test functions
3. Update summary calculation
4. Add tests in `tests/performance_metrics_tests.rs`
5. Document in `docs/PERFORMANCE_METRICS.md`
6. Update this README

## Support

For issues or questions:
- Check [docs/PERFORMANCE_METRICS.md](../docs/PERFORMANCE_METRICS.md)
- Review test output logs
- Open an issue on GitHub

---

**Last Updated:** 2025-11-16
