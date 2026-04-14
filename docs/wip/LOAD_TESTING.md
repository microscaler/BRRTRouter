# Load Testing BRRTRouter with Goose

Guide to load testing BRRTRouter applications using [Goose 0.18.1](https://docs.rs/goose/0.18.1/goose/) - a Rust-native load testing framework inspired by Locust.

## Overview

**Using Goose 0.18.1** with the latest API improvements including:
- Killswitch mechanism for programmatic termination
- Enhanced coordinated omission metrics
- Context7 support for AI assistants
- Standardized logging format

Goose is perfect for testing:
- **Static file serving** (HTML, CSS, JS, JSON)
- **MiniJinja template rendering** performance
- **API endpoints** with authentication
- **Built-in endpoints** (/health, /metrics)
- **Complete user flows**

## Quick Start

### 1. Create Load Test Binary

```bash
# Create a new binary for load testing
cargo new --bin brrtrouter-loadtest
cd brrtrouter-loadtest
```

### 2. Add Dependencies

```toml
# Cargo.toml
[dependencies]
goose = "0.18.1"  # Latest stable version
tokio = { version = "1", features = ["full"] }
```

### 3. Create Load Test

```rust
// src/main.rs
use goose::prelude::*;

async fn load_css(user: &mut GooseUser) -> TransactionResult {
    user.get("css/styles.css").await?;
    Ok(())
}

async fn load_js(user: &mut GooseUser) -> TransactionResult {
    user.get("js/app.js").await?;
    Ok(())
}

async fn api_health(user: &mut GooseUser) -> TransactionResult {
    user.get("health").await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), GooseError> {
    GooseAttack::initialize()?
        .register_scenario(
            scenario!("Static Files")
                .register_transaction(transaction!(load_css))
                .register_transaction(transaction!(load_js))
        )
        .register_scenario(
            scenario!("Health Check")
                .register_transaction(transaction!(api_health))
        )
        .execute()
        .await?;
    
    Ok(())
}
```

### 4. Run Load Test

```bash
# Start your BRRTRouter service first
cd /path/to/brrtrouter/examples/pet_store
cargo run -- --spec doc/openapi.yaml --port 8080

# In another terminal, run load test
cd brrtrouter-loadtest
cargo run -- --host http://localhost:8080 --users 10 --run-time 30s
```

## Example Commands

```bash
# Basic load test: 10 users for 30 seconds
cargo run -- --host http://localhost:8080 --users 10 --run-time 30s

# Include startup metrics (recommended for accurate results)
cargo run -- --host http://localhost:8080 \
  --users 10 \
  --run-time 30s \
  --no-reset-metrics

# Stress test: 100 users ramping up at 10/second for 5 minutes
cargo run -- --host http://localhost:8080 \
  --users 100 \
  --hatch-rate 10 \
  --run-time 5m

# Generate HTML report with complete metrics
cargo run -- --host http://localhost:8080 \
  --users 50 \
  --run-time 2m \
  --no-reset-metrics \
  --report-file report.html

# Full test with multiple report formats
cargo run --release -- --host http://localhost:8080 \
  -u20 -r5 -t1m \
  --no-reset-metrics \
  --report-file report.html \
  --report-file metrics.json
```

### Key Flags

According to the [Goose metrics documentation](https://book.goose.rs/getting-started/metrics.html):

- `-u`, `--users`: Number of concurrent users (e.g., `-u10`)
- `-r`, `--hatch-rate`: Users to spawn per second (e.g., `-r3`)
- `-t`, `--run-time`: Duration to run (e.g., `-t1m`, `-t30s`)
- `--no-reset-metrics`: Include startup metrics (recommended for complete data)
- `--report-file`: Generate report (extension determines format)
- `--host`: Target host URL

## Test Scenarios

### Static Files

```rust
async fn load_static_files(user: &mut GooseUser) -> TransactionResult {
    user.get("css/styles.css").await?;
    user.get("js/app.js").await?;
    user.get("api-info.json").await?;
    Ok(())
}
```

### Authenticated API Calls

```rust
async fn api_with_auth(user: &mut GooseUser) -> TransactionResult {
    // Note: Goose 0.18 uses a different API for headers
    // Check the Goose documentation for current syntax
    user.get("pets").await?;
    Ok(())
}
```

### Full User Flow

```rust
async fn complete_flow(user: &mut GooseUser) -> TransactionResult {
    // 1. Load landing page
    user.get("/").await?;
    
    // 2. Load assets
    user.get("css/styles.css").await?;
    user.get("js/app.js").await?;
    
    // 3. API calls
    user.get("health").await?;
    
    Ok(())
}
```

## BRRTRouter Example

See `tests/goose_load_tests_simple.rs` for a working example that tests:
- CSS file serving
- JavaScript file serving
- JSON file serving  
- Health check endpoint

## Metrics & Reports

Goose provides comprehensive metrics in multiple formats:

### ASCII Metrics (Perfect for CI/CD)

Goose outputs detailed ASCII metrics to stdout, ideal for GitHub Actions and CI pipelines:

```
=== PER SCENARIO METRICS ===
Name                     |  # users |  # times run | scenarios/s | iterations
1: Static Files          |        5 |           20 |        0.33 |       4.00
Aggregated               |        5 |           20 |        0.33 |       4.00

=== PER TRANSACTION METRICS ===
Name                     |   # times run |        # fails |  trans/s |  fail/s
1: Load CSS              |            20 |         0 (0%) |     0.33 |    0.00
2: Load JS               |            20 |         0 (0%) |     0.33 |    0.00
3: Load JSON             |            20 |         0 (0%) |     0.33 |    0.00
Aggregated               |            60 |         0 (0%) |     1.00 |    0.00
```

Metrics include:
- **Per-scenario stats**: users, runs, scenarios/s, iterations
- **Per-transaction stats**: runs, failures, trans/s, fail/s
- **Response times**: avg, min, max, median
- **Status codes**: per-request breakdown
- **Overview**: load test phases and timing

### Report Formats

Generate reports in multiple formats using `--report-file`:

```bash
# HTML report (with interactive charts)
cargo run -- --report-file report.html

# Markdown report
cargo run -- --report-file report.md

# JSON report (machine-readable)
cargo run -- --report-file metrics.json

# Multiple formats at once
cargo run -- --report-file report.html --report-file metrics.json
```

### HTML Reports

HTML reports include interactive [eCharts](https://echarts.apache.org/) visualizations:
- **Request graphs**: visualize all requests over time
- **Response time graphs**: per-request response times
- **Status code tables**: comprehensive status breakdown
- **Transaction metrics**: logical grouping of requests
- **Scenario metrics**: high-level user flow stats
- **User graphs**: ramp-up/down visualization

**Note:** HTML reports require CDN access to load eCharts library.

### GitHub Actions Integration

**BRRTRouter includes a full Goose load test in CI** (`.github/workflows/ci.yml`):

The `goose-load-test` job runs automatically on every PR and push, testing:
- ✅ **All OpenAPI endpoints** (GET /pets, /users, etc.) with authentication
- ✅ **Built-in endpoints** (/health, /metrics)
- ✅ **Static files** (/openapi.yaml, /docs, CSS, JS)
- ✅ **Memory leak detection** via sustained 2-minute load test
- ✅ **20 concurrent users** ramping up at 5/second

**Artifacts uploaded:**
- `goose-ascii-metrics` - Full ASCII metrics + summary (7 days)
- `goose-html-report` - Interactive HTML report (7 days)
- `goose-json-report` - Machine-readable JSON (7 days)

**Example GitHub Actions configuration:**

```yaml
goose-load-test:
  runs-on: ubuntu-latest
  services:
    petstore:
      image: your-service-image
      ports:
        - 8080:8080
  steps:
    - name: Checkout code
      uses: actions/checkout@v4

    - name: Set up Rust
      uses: dtolnay/rust-toolchain@stable

    - name: Build Goose load test
      run: cargo build --release --example api_load_test

    - name: Run comprehensive load test
      run: |
        cargo run --release --example api_load_test -- \
          --host http://localhost:8080 \
          --users 20 \
          --hatch-rate 5 \
          --run-time 2m \
          --no-reset-metrics \
          --report-file goose-report.html \
          --report-file goose-report.json \
          | tee goose-metrics.txt

    - name: Check for failures
      run: |
        if grep -q "Aggregated.*0 (0%)" goose-metrics.txt; then
          echo "✅ No failures detected"
        else
          echo "❌ Load test had failures"
          exit 1
        fi

    - name: Upload ASCII metrics
      uses: actions/upload-artifact@v4
      with:
        name: goose-ascii-metrics
        path: goose-metrics.txt

    - name: Upload HTML report
      uses: actions/upload-artifact@v4
      with:
        name: goose-html-report
        path: goose-report.html
```

**Key advantages over wrk:**
- ✅ Tests **authenticated endpoints** (wrk only tests /health)
- ✅ Validates **all OpenAPI routes** from spec
- ✅ Better **memory leak detection** via longer sustained load
- ✅ **Per-endpoint metrics** (wrk aggregates everything)
- ✅ **Automatic failure detection** in CI

## Best Practices

1. **Start Small**: Begin with 5-10 users to verify functionality
2. **Ramp Gradually**: Use `--hatch-rate` to avoid overwhelming the server
3. **Test Realistic Flows**: Combine static + API requests like real users
4. **Monitor Server**: Watch CPU, memory, connections during tests
5. **Generate Reports**: Use `--report-file` for detailed HTML reports

## Integration with BRRTRouter

BRRTRouter's built-in features work great with Goose:

### Health Check Monitoring
```rust
async fn verify_health(user: &mut GooseUser) -> TransactionResult {
    let goose = user.get("health").await?;
    let response = goose.response?;
    assert_eq!(response.status(), 200);
    Ok(())
}
```

### Metrics Collection
```rust
async fn check_metrics(user: &mut GooseUser) -> TransactionResult {
    user.get("metrics").await?;
    Ok(())
}
```

## Resources

- [Goose Documentation](https://docs.rs/goose/latest/goose/)
- [The Goose Book](https://book.goose.rs/)
- [Goose GitHub](https://github.com/tag1consulting/goose)
- [BRRTRouter Static Files Guide](./STATIC_FILES_AND_TEMPLATES.md)
- [Example Test](../tests/goose_load_tests_simple.rs)

## Troubleshooting

### Connection Refused
- Ensure BRRTRouter service is running
- Check the correct host and port
- Verify firewall settings

### Too Many Failures
- Reduce user count or hatch rate
- Check server capacity
- Review BRRTRouter logs

### Slow Performance
- Check if template rendering is the bottleneck
- Monitor static file caching
- Review middleware overhead

## Next Steps

1. Create standalone load test binary
2. Define realistic user scenarios
3. Start with low user count
4. Gradually increase load
5. Monitor and optimize

For production load testing, consider:
- Running from multiple machines
- Testing from different geographic locations
- Simulating different user types
- Testing failure scenarios
