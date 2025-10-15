# BRRTRouter Load Testing Guide 🚀

This guide covers two different load testing strategies using Goose, each designed for different use cases.

## Table of Contents

1. [Standard API Load Test](#standard-api-load-test) - Comprehensive endpoint testing with fixed load
2. [Adaptive Load Test](#adaptive-load-test) - **Auto-detects failure point using Prometheus**

---

## Prerequisites

```bash
# Ensure BRRTRouter is running
kubectl port-forward -n brrtrouter-dev svc/petstore 8080:8080 &

# Ensure Prometheus is accessible (for adaptive test)
kubectl port-forward -n brrtrouter-dev svc/prometheus 9090:9090 &

# Ensure Grafana is accessible (for visualization)
kubectl port-forward -n brrtrouter-dev svc/grafana 3000:3000 &
```

---

## 1. Standard API Load Test

**Use Case**: Quick validation, CI/CD pipelines, regression testing, comprehensive API testing

**File**: `examples/api_load_test.rs`

### Configuration (Command Line Arguments)

Goose load test configuration is done via command-line arguments following the `--` separator.

### Example Usage

```bash
# Quick test: 10 users for 30 seconds
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  --users 10 \
  --hatch-rate 2 \
  --run-time 30s

# Standard test: 50 users for 2 minutes with reports
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  --users 50 \
  --hatch-rate 10 \
  --run-time 2m \
  --no-reset-metrics \
  --report-file goose-report.html

# High load: 1000 users for 5 minutes
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  --users 1000 \
  --hatch-rate 50 \
  --run-time 5m

# Via Tilt UI: Click "run-goose-api-test" button
```

### API Endpoints Tested (Weighted)

- **Infrastructure (10%)**: `/health`, `/metrics`
- **Pet Store API (70%)**: `/pets`, `/pets/{id}`, `/pets?name={name}`
- **User API (15%)**: `/users`, `/users/{id}`
- **Static Resources (5%)**: `/openapi.yaml`, `/` (dashboard)

### Expected Output

```
╔══════════════════════════════════════════════════════════════════════════╗
║  BRRTRouter Goose Load Test 🦆 - Full API Surface                       ║
╚══════════════════════════════════════════════════════════════════════════╝

Configuration:
  Host: http://localhost:8080
  Users: 100
  Hatch Rate: 10 users/second (gradual ramp-up)
  Duration: 60s

API Endpoints tested:
  Infrastructure (10%):
    ✓ GET /health
    ✓ GET /metrics
  Pet Store API (70%):
    ✓ GET /pets (list all)
    ✓ GET /pets/{id} (get by ID)
    ✓ GET /pets?name={name} (search)
  ...
```

---

## 2. Adaptive Load Test ⭐

**Use Case**: Finding breaking point, capacity planning, SLA validation, automated performance regression detection

**File**: `examples/adaptive_load_test.rs`

### 🎯 What Makes This Special?

This test **runs in a continuous loop**, querying Prometheus after each cycle to check the error rate. It automatically increases load until the error rate reaches the threshold (default: 5%), identifying the exact breaking point where BRRTRouter or the underlying CPU limit is reached.

**Detailed Metrics**: After each cycle, Goose prints a comprehensive report (similar to the api_load_test output) showing per-scenario, per-transaction, and per-request metrics, followed by a Prometheus health check to determine if the load should continue increasing.

### Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `GOOSE_HOST` | `http://localhost:8080` | Target host |
| `PROMETHEUS_URL` | `http://localhost:9090` | Prometheus URL |
| `START_USERS` | `100` | Starting user count (lowered for faster discovery) |
| `MAX_USERS` | `50000` | Maximum user count (safety limit) |
| `RAMP_STEP` | `500` | Users to add per cycle |
| `HATCH_RATE` | `1000` | Users spawned per second (controls ramp-up speed) |
| `STAGE_DURATION` | `60` | Seconds per cycle (1 minute for faster discovery) |
| `ERROR_RATE_THRESHOLD` | `5.0` | Max error rate % (primary failure condition) |
| `P99_LATENCY_THRESHOLD` | `2.0` | Max p99 latency seconds (warning only) |
| `ACTIVE_REQUESTS_THRESHOLD` | `5000` | Max active requests (warning only) |

### Example Usage

```bash
# Default: Fast discovery with 1-minute cycles
# Ramp from 100 → 50,000 users in steps of 500 (60s cycles)
# Ramp-up time: 0.1s @ 100 users, 5s @ 5000 users
cargo run --release --example adaptive_load_test -- \
  --host http://localhost:8080

# High-load start: Skip low numbers if you know baseline capacity
START_USERS=2000 RAMP_STEP=1000 \
cargo run --release --example adaptive_load_test -- \
  --host http://localhost:8080

# Sustained load: Test stability over longer periods
START_USERS=1000 RAMP_STEP=500 STAGE_DURATION=300 \
cargo run --release --example adaptive_load_test -- \
  --host http://localhost:8080

# Aggressive stress: Fast ramp with high hatch rate
START_USERS=5000 RAMP_STEP=2000 HATCH_RATE=5000 STAGE_DURATION=30 \
cargo run --release --example adaptive_load_test -- \
  --host http://localhost:8080

# Conservative: Gradual ramp with tight SLA thresholds
START_USERS=50 RAMP_STEP=100 ERROR_RATE_THRESHOLD=1.0 P99_LATENCY_THRESHOLD=0.5 \
cargo run --release --example adaptive_load_test -- \
  --host http://localhost:8080

# Via Tilt UI: Click "run-goose-adaptive" button
```

**Understanding Ramp-Up**: Goose **does** ramp up within each test cycle:
- With `HATCH_RATE=1000` and `START_USERS=100` → ramp completes in 0.1s
- With `HATCH_RATE=1000` and 5000 users → ramp completes in 5s  
- After reaching target users, load is sustained for the remaining cycle duration
- Higher `HATCH_RATE` = faster ramp-up (but may cause initial spikes)

### How It Works (Continuous Loop)

1. **Cycle 1**: Start with `START_USERS` concurrent users
2. **Run Load**: Execute Goose attack for `STAGE_DURATION` seconds
3. **Query Prometheus**: Check error rate from metrics:
   - Error rate (% of non-2xx responses) - **PRIMARY THRESHOLD**
   - P99 latency (99th percentile response time) - warning only
   - Active requests (number of in-flight requests) - warning only
   - Throughput (requests per second) - for reporting
4. **Check Error Rate**:
   - If error rate **< 5%** (threshold): System is healthy, output report, increment users, loop back to step 2
   - If error rate **≥ 5%** (threshold): **LIMIT FOUND**, report breaking point and stop
5. **Output Report**: Display cycle results showing users, error rate, latency, throughput
6. **Increment Load**: Add `RAMP_STEP` users to `current_users`
7. **Loop**: Repeat from step 2 with new user count

The test runs continuously until it finds the **exact point where error rate crosses 5%**, indicating either BRRTRouter's limit or the CPU capacity limit.

### Expected Output

```
╔══════════════════════════════════════════════════════════════════════════╗
║  BRRTRouter Adaptive Load Test 🎯                                        ║
║  Continuous loop: incrementally increases load until failure             ║
╚══════════════════════════════════════════════════════════════════════════╝

Configuration:
  Target: http://localhost:8080
  Prometheus: http://localhost:9090
  Start Users: 100 (increment: 500 per cycle)
  Max Users: 50000 (safety limit)
  Hatch Rate: 1000 users/second
  Cycle Duration: 60s per load level
  Ramp-up Time: ~0s to reach 100 users

Failure Threshold:
  🎯 Error Rate ≥ 5.0% = LIMIT REACHED
  ⚠️  P99 Latency > 2.00s = WARNING
  ⚠️  Active Requests > 1000 = WARNING

Mode: CONTINUOUS - runs until error rate ≥ 5.0% or max users reached
Press Ctrl+C to stop manually

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Cycle 1 - Testing with 10 concurrent users
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

[Goose runs for 5 minutes and prints full detailed metrics report]
... (see Sample Goose Report section above for full output) ...

╭─────────────────────────────────────────────────────────────────────╮
│ 🔍 Checking Prometheus Metrics - Cycle 1                           │
╰─────────────────────────────────────────────────────────────────────╯
⏳ Waiting 5 seconds for Prometheus to scrape metrics...

📊 System Metrics:
  Error Rate: 0.00% (threshold: 5.0%)
  P99 Latency: 0.045s (threshold: 2.00s)
  Active Requests: 8 (threshold: 1000)
  Throughput: 124 req/s

✅ System healthy - increasing load

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Cycle 2 - Testing with 60 concurrent users
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📊 System Metrics:
  Error Rate: 0.12% (threshold: 5.0%)
  P99 Latency: 0.182s (threshold: 2.00s)
  Active Requests: 54 (threshold: 1000)
  Throughput: 847 req/s

✅ System healthy - increasing load

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Cycle 4 - Testing with 160 concurrent users
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📊 System Metrics:
  Error Rate: 2.34% (threshold: 5.0%)
  P99 Latency: 1.876s (threshold: 2.00s)
  Active Requests: 142 (threshold: 1000)
  Throughput: 2134 req/s

✅ System healthy - increasing load

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Cycle 5 - Testing with 210 concurrent users
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📊 System Metrics:
  Error Rate: 3.87% (threshold: 5.0%)
  P99 Latency: 2.347s (threshold: 2.00s)
  Active Requests: 198 (threshold: 1000)
  Throughput: 2687 req/s

⚠️  System functional but showing stress:
   ⚠️  P99 latency 2.347s > 2.00s
   Error rate still acceptable (3.87%), continuing ramp-up

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Cycle 6 - Testing with 260 concurrent users
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📊 System Metrics:
  Error Rate: 8.45% (threshold: 5.0%)
  P99 Latency: 3.124s (threshold: 2.00s)
  Active Requests: 243 (threshold: 1000)
  Throughput: 2547 req/s

🔴 LIMIT REACHED - Error Rate: 8.45% ≥ 5.0%

╔══════════════════════════════════════════════════════════════════════════╗
║  Breaking Point Identified 🎯                                            ║
╚══════════════════════════════════════════════════════════════════════════╝

Maximum Capacity Found:
  Breaking Point: 260 concurrent users
  Last Healthy Load: 210 users
  Peak Throughput: 2687 req/s (at 210 users)
  Current Error Rate: 8.45%

Recommendation:
  Set production limit to ~168 users (80% of last healthy)
  Expected throughput: ~2150 req/s
```

### Prometheus Queries Used

The adaptive test queries Prometheus with these PromQL expressions:

```promql
# Error Rate (% of non-2xx responses)
100 * (
    sum(rate(brrtrouter_requests_total{status!~"2.."}[30s])) /
    sum(rate(brrtrouter_requests_total[30s]))
)

# P99 Latency (99th percentile)
histogram_quantile(0.99, rate(brrtrouter_request_duration_seconds_bucket[30s]))

# Active Requests (current in-flight)
brrtrouter_active_requests

# Throughput (requests per second)
sum(rate(brrtrouter_requests_total[30s]))
```

---

## Comparison Matrix

| Feature | Simple | Continuous | Adaptive |
|---------|--------|------------|----------|
| **Duration** | Fixed | Infinite | Variable |
| **User Count** | Fixed | Fixed | Ramping |
| **Prometheus Integration** | ❌ | ❌ | ✅ |
| **Auto-detects Failure** | ❌ | ❌ | ✅ |
| **Best For** | CI/CD | Dashboards | Capacity Planning |
| **Requires Prometheus** | No | No | Yes |
| **Stops on Failure** | No | No | Yes |

---

## Recommended Workflow

### For Development

1. **Start continuous load** to populate Grafana:
   ```bash
   GOOSE_CONTINUOUS=true GOOSE_USERS=20 \
     cargo test --test goose_load_tests_simple test_load -- --ignored
   ```

2. **Open Grafana** at http://localhost:3000
   - Dashboard: "BRRTRouter - Unified Observability"
   - Watch metrics populate in real-time

3. **Make code changes** and watch metrics update via Tilt

4. **Press Ctrl+C** when done

### For Capacity Planning

1. **Run adaptive test** to find breaking point:
   ```bash
   cargo test --test goose_load_tests_adaptive test_adaptive_load -- --ignored
   ```

2. **Note the breaking point** (e.g., "210 users")

3. **Calculate headroom**:
   - Breaking point: 210 users
   - Safe capacity: 160 users (80% of breaking point)
   - Peak throughput: 2847 req/s

4. **Set SLA targets**:
   - Target: 160 users = 2278 req/s
   - Alert threshold: 180 users (90% capacity)

### For CI/CD

1. **Add simple load test** to pipeline:
   ```bash
   GOOSE_USERS=50 GOOSE_DURATION=30 \
     cargo test --test goose_load_tests_simple test_load -- --ignored
   ```

2. **Assert no failures** in test output

3. **Optional**: Query Prometheus for regression checks

---

## Troubleshooting

### Adaptive Test: "Could not query Prometheus"

**Cause**: Prometheus is not accessible or metrics haven't been scraped yet

**Solution**:
```bash
# Verify Prometheus is running
kubectl get pods -n monitoring

# Port-forward Prometheus
kubectl port-forward -n monitoring svc/prometheus 9090:9090

# Verify metrics exist
curl -s 'http://localhost:9090/api/v1/query?query=brrtrouter_active_requests' | jq
```

### Adaptive Test: All Metrics Show 0.0

**Cause**: Prometheus hasn't scraped BRRTRouter metrics yet

**Solution**:
```bash
# Verify /metrics endpoint works
curl http://localhost:8080/metrics | grep brrtrouter

# Check Prometheus scrape config
kubectl get cm -n monitoring prometheus-config -o yaml | grep brrtrouter

# Force Prometheus reload
kubectl rollout restart statefulset -n monitoring prometheus
```

### Load Test: All Requests Fail

**Cause**: Target host is not reachable

**Solution**:
```bash
# Test connectivity
curl http://localhost:8080/health

# Verify port-forward is running
ps aux | grep port-forward

# Restart port-forward
kubectl port-forward -n brrtrouter-dev svc/petstore 8080:8080
```

---

## Performance Tips

### Achieving High Throughput

1. **Use hatch rate** to avoid thundering herd:
   ```bash
   GOOSE_USERS=1000 GOOSE_HATCH_RATE=50  # Spawn 50 users/sec
   ```

2. **Scale BRRTRouter** for higher capacity:
   ```bash
   kubectl scale deployment petstore --replicas=3 -n brrtrouter-dev
   ```

3. **Run Goose from multiple machines** for even higher load

4. **Disable verbose output** in Goose (it's already minimal)

### Realistic Load Distribution

The tests are weighted to simulate realistic traffic:

- **70% API calls** (pets, users) - Most users interact with data
- **15% search** - Common user behavior
- **10% infrastructure** (/metrics, /health) - Monitoring systems
- **5% static resources** - Occasional dashboard loads

Adjust weights in the test files to match your production traffic patterns.

---

## Next Steps

1. ✅ **Metrics are now implemented** (active requests, histograms, status codes)
2. ✅ **Grafana dashboard is configured** to query these metrics
3. ✅ **Three load test variants** are ready (simple, continuous, adaptive)
4. 🎯 **Run adaptive test** to find your system's breaking point
5. 📊 **Use continuous test** to validate dashboard metrics
6. 🚀 **Integrate simple test** into CI/CD pipeline

---

## Example: Finding Your Limits

```bash
# Step 1: Start with low load (sanity check)
GOOSE_USERS=10 cargo test --test goose_load_tests_simple test_load -- --ignored

# Step 2: Increase to moderate load
GOOSE_USERS=100 cargo test --test goose_load_tests_simple test_load -- --ignored

# Step 3: Find exact breaking point automatically
cargo test --test goose_load_tests_adaptive test_adaptive_load -- --ignored

# Expected result: System fails at ~210 users
# Safe capacity: ~160 users (80% of breaking point)
# Peak throughput: ~2847 req/s
```

Now you know your limits! 🎯

