# Goose Load Testing in BRRTRouter

## Overview

BRRTRouter includes comprehensive load testing using [Goose](https://book.goose.rs/), a Rust load testing framework. This provides **superior API coverage** compared to wrk, testing ALL OpenAPI endpoints including authenticated routes.

## Why Goose?

### Advantages Over wrk

| Feature | Goose | wrk |
|---------|-------|-----|
| **Authenticated endpoints** | ✅ Full support | ❌ Limited |
| **All OpenAPI routes** | ✅ Tests every endpoint | ❌ Manual scripts needed |
| **Memory leak detection** | ✅ Sustained multi-minute tests | ⚠️  Short bursts only |
| **Per-endpoint metrics** | ✅ Detailed breakdown | ❌ Aggregated only |
| **Static file testing** | ✅ Included | ❌ Separate tests |
| **Failure detection** | ✅ Automatic CI checks | ⚠️  Manual inspection |
| **ASCII metrics** | ✅ CI/CD friendly output | ⚠️  Limited |
| **HTML reports** | ✅ Interactive visualizations | ❌ None |

### What Goose Tests That wrk Doesn't

1. **Authenticated API Endpoints**
   - `GET /pets` with `X-API-Key` header
   - `GET /users` with authentication
   - All secured routes from OpenAPI spec

2. **Full OpenAPI Spec Coverage**
   - Every endpoint defined in `openapi.yaml`
   - Path parameters (`/pets/{id}`, `/users/{id}`)
   - Query parameters with validation

3. **Static File Serving**
   - OpenAPI spec (`/openapi.yaml`)
   - Swagger UI (`/docs`)
   - CSS, JavaScript, and other assets

4. **Memory Leak Detection**
   - Sustained 2-minute load tests (vs wrk's 60-second bursts)
   - Multiple concurrent scenarios
   - Resource usage tracking

## GitHub Actions Integration

### Automatic CI Testing

Every PR and push runs a comprehensive Goose load test:

```yaml
# .github/workflows/ci.yml
goose-load-test:
  needs: e2e-docker
  runs-on: ubuntu-latest
  services:
    petstore:
      image: ${{ needs.e2e-docker.outputs.image }}
      ports:
        - 8080:8080
```

**Configuration:**
- **Duration**: 2 minutes
- **Users**: 20 concurrent
- **Hatch Rate**: 5 users/second
- **Endpoints**: All OpenAPI routes + static files

**Artifacts uploaded (7-day retention):**
- `goose-ascii-metrics` - Full ASCII metrics + summary
- `goose-html-report` - Interactive HTML with charts
- `goose-json-report` - Machine-readable metrics

### ASCII Metrics Example

```
=== PER SCENARIO METRICS ===
Name                     |  # users |  # times run | scenarios/s | iterations
1: Built-in Endpoints    |        4 |           40 |        0.33 |      10.00
2: Pet API               |        6 |           60 |        0.50 |      10.00
3: User API              |        6 |           60 |        0.50 |      10.00
4: Static Files          |        3 |           30 |        0.25 |      10.00
5: Static Assets         |        1 |           10 |        0.08 |      10.00
Aggregated               |       20 |          200 |        1.67 |      10.00

=== PER TRANSACTION METRICS ===
Name                     |   # times run |        # fails |  trans/s |  fail/s
1: GET /health           |            40 |         0 (0%) |     0.33 |    0.00
2: GET /metrics          |            40 |         0 (0%) |     0.33 |    0.00
3: GET /pets (auth)      |            60 |         0 (0%) |     0.50 |    0.00
4: GET /pets/{id} (auth) |            60 |         0 (0%) |     0.50 |    0.00
5: GET /users (auth)     |            60 |         0 (0%) |     0.50 |    0.00
6: GET /users/{id} (auth)|            60 |         0 (0%) |     0.50 |    0.00
7: GET /openapi.yaml     |            30 |         0 (0%) |     0.25 |    0.00
8: GET /docs (Swagger)   |            30 |         0 (0%) |     0.25 |    0.00
Aggregated               |           380 |         0 (0%) |     3.17 |    0.00
```

## Running Locally

### Basic Load Test

```bash
# Start the service
cd examples/pet_store
cargo run -- --spec doc/openapi.yaml --port 8080 --test-api-key test123

# In another terminal, run load test
cd ../..
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  --users 20 \
  --hatch-rate 5 \
  --run-time 2m \
  --no-reset-metrics \
  --header "X-API-Key: test123" \
  --report-file goose-report.html
```

### Quick Test (30 seconds)

```bash
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  -u10 -r2 -t30s \
  --header "X-API-Key: test123"
```

### Stress Test (100 users, 5 minutes)

```bash
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  -u100 -r10 -t5m \
  --no-reset-metrics \
  --header "X-API-Key: test123" \
  --report-file stress-test.html
```

## Test Scenarios

The load test includes 5 weighted scenarios:

1. **Built-in Endpoints** (20% weight)
   - `GET /health` - Health check
   - `GET /metrics` - Prometheus metrics

2. **Pet API** (30% weight)
   - `GET /pets` - List all pets (authenticated)
   - `GET /pets/{id}` - Get specific pet (authenticated)

3. **User API** (30% weight)
   - `GET /users` - List all users (authenticated)
   - `GET /users/{id}` - Get specific user (authenticated)

4. **Static Files** (15% weight)
   - `GET /openapi.yaml` - OpenAPI specification
   - `GET /docs` - Swagger UI documentation

5. **Static Assets** (5% weight)
   - `GET /css/styles.css` - CSS files
   - `GET /js/app.js` - JavaScript files

## Report Formats

Goose generates reports in multiple formats:

### HTML Report (Interactive)

```bash
--report-file goose-report.html
```

Features:
- **eCharts visualizations** (requires CDN)
- Request graphs over time
- Response time analysis
- Status code breakdown
- Transaction and scenario metrics
- User ramp-up/down visualization

### Markdown Report

```bash
--report-file goose-report.md
```

### JSON Report (Machine-Readable)

```bash
--report-file goose-metrics.json
```

### Multiple Formats

```bash
--report-file report.html --report-file metrics.json
```

## Key Flags

According to [Goose metrics documentation](https://book.goose.rs/getting-started/metrics.html):

- `-u`, `--users` - Number of concurrent users (e.g., `-u20`)
- `-r`, `--hatch-rate` - Users to spawn per second (e.g., `-r5`)
- `-t`, `--run-time` - Duration (e.g., `-t2m`, `-t30s`, `-t5m`)
- `--no-reset-metrics` - Include startup metrics (recommended)
- `--header` - Add global header to all requests
- `--report-file` - Generate report (extension determines format)
- `--host` - Target host URL

## CI/CD Integration

### Failure Detection

The GitHub Actions workflow automatically checks for failures:

```bash
if grep -q "Aggregated.*0 (0%)" goose-metrics.txt; then
  echo "✅ No failures detected"
else
  echo "❌ Load test had failures"
  exit 1
fi
```

### Metrics Extraction

A summary is automatically generated for PR comments:

```markdown
## 📊 Goose Load Test Results

**Test Configuration:**
- Duration: 2 minutes
- Users: 20 concurrent
- Hatch Rate: 5 users/second

**Endpoints Tested:**
- ✅ GET /health
- ✅ GET /metrics
- ✅ GET /pets (authenticated)
- ✅ GET /pets/{id} (authenticated)
- ✅ GET /users (authenticated)
- ✅ GET /users/{id} (authenticated)
- ✅ GET /openapi.yaml
- ✅ GET /docs (Swagger UI)
- ✅ Static files (CSS, JS)
```

## Best Practices

1. **Always use `--no-reset-metrics`** for complete data
2. **Set appropriate headers** for authenticated endpoints
3. **Start small** (10 users) and ramp up gradually
4. **Run longer tests** (2-5 minutes) to detect memory leaks
5. **Generate HTML reports** for detailed analysis
6. **Check failure rates** in CI to catch regressions

## Memory Leak Detection

Goose is particularly effective for detecting memory leaks:

```bash
# Run a 10-minute sustained load test
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  -u50 -r10 -t10m \
  --header "X-API-Key: test123"

# Monitor memory usage in another terminal
watch -n 1 'ps aux | grep pet_store | grep -v grep'
```

## Resources

- **Goose Documentation**: https://book.goose.rs/
- **BRRTRouter Load Testing Guide**: `docs/LOAD_TESTING.md`
- **Example Test**: `examples/api_load_test.rs`
- **GitHub Actions**: `.github/workflows/ci.yml` (goose-load-test job)

## Comparison with Performance Tests

| Test Type | Tool | Duration | Coverage | Purpose |
|-----------|------|----------|----------|---------|
| **Unit** | `cargo test` | Seconds | Individual functions | Correctness |
| **Integration** | `cargo test` | Seconds | Request/response | API contracts |
| **Load (wrk)** | `wrk` | 60s | Single endpoint | Throughput |
| **Load (Goose)** | `goose` | 2-10m | All endpoints | Memory leaks, full coverage |
| **Profiling** | `perf` | 70s | Single endpoint | CPU profiling |

**Goose fills the gap** between integration tests and profiling, providing:
- ✅ Full API coverage from OpenAPI spec
- ✅ Sustained load for memory leak detection
- ✅ Per-endpoint metrics
- ✅ Authenticated endpoint testing
- ✅ CI/CD integration with automatic failure detection

