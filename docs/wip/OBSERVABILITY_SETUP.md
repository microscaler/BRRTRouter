# Complete Observability Stack Setup

## 🎯 Current Status

### ❌ What's Missing
- **No structured logging** - Only `println!` to stdout
- **No tracing** - Can't see request flows
- **No log aggregation** - Logs stuck in stdout
- **TooManyHeaders spam** - No way to filter/reduce noise
- **Pod crashes** - No visibility into why

### ✅ What We Have (But Not Connected)
- Prometheus (metrics) - running but not scraped
- Grafana - running but no data sources
- Jaeger - running but no traces
- OTEL Collector - running but not receiving data

## 🔧 What Needs To Be Done

### 1. Add Tracing Dependencies to Pet Store

The generated `pet_store/Cargo.toml` needs:

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
opentelemetry = { version = "0.21", features = ["trace"] }
opentelemetry-otlp = { version = "0.14", features = ["grpc-tonic"] }
opentelemetry_sdk = { version = "0.21", features = ["rt-tokio"] }
tracing-opentelemetry = "0.22"
```

### 2. Initialize Tracing in main.rs

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn init_tracing() -> anyhow::Result<()> {
    // JSON formatter for structured logs
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true);

    // Environment filter (RUST_LOG)
    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
        .or_else(|_| tracing_subscriber::EnvFilter::try_new("info"))
        .unwrap();

    // OpenTelemetry exporter to OTEL Collector
    let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://otel-collector:4317".to_string());
    
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(otlp_endpoint)
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)?;

    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // Combine layers
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(telemetry_layer)
        .init();

    Ok(())
}

fn main() -> io::Result<()> {
    // Initialize tracing FIRST
    init_tracing().expect("Failed to initialize tracing");
    
    tracing::info!("Starting pet_store application");
    
    // ... rest of main
}
```

### 3. Replace println! with tracing

```rust
// Before
println!("[startup] spec_path={}", spec_path.display());

// After  
tracing::info!(spec_path = %spec_path.display(), "Loaded OpenAPI spec");

// Before
println!("🚀 pet_store example server listening on {addr}");

// After
tracing::info!(addr = %addr, "Server started successfully");
```

### 4. Add Request Tracing in AppService

In `src/server/service.rs`:

```rust
impl HttpService for AppService {
    fn call(&mut self, req: Request, res: &mut Response) -> io::Result<()> {
        // Create span for this request
        let span = tracing::info_span!(
            "http_request",
            method = %method,
            path = %path,
            status = tracing::field::Empty,
        );
        let _enter = span.enter();

        // ... existing code ...

        // Record response status
        span.record("status", res.status_code());
        
        Ok(())
    }
}
```

### 5. Filter Out TooManyHeaders Noise

```rust
// In service.rs, suppress noisy errors
fn parse_request_with_error_handling(req: Request) -> Result<ParsedRequest, String> {
    match parse_request(req) {
        Ok(parsed) => Ok(parsed),
        Err(e) if e.to_string().contains("TooManyHeaders") => {
            // These are from K8s probes, just ignore silently
            tracing::debug!("Ignored TooManyHeaders error (likely probe)");
            Err("TooManyHeaders".to_string())
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to parse HTTP request");
            Err(e.to_string())
        }
    }
}
```

## 📊 Expected Results

### Grafana Dashboards
- **Request Rate**: Requests/sec by endpoint
- **Latency**: P50, P95, P99 response times
- **Errors**: 4xx, 5xx error rates
- **Resource Usage**: CPU, memory, connections

### Jaeger Traces
- **Request Flow**: See each request through middleware → router → handler
- **Timing**: How long each step takes
- **Errors**: Where requests fail

### Prometheus Metrics
Already exposed at `/metrics`:
- `brrtrouter_http_requests_total`
- `brrtrouter_http_request_duration_seconds`
- `brrtrouter_http_requests_in_flight`

### Structured Logs
JSON logs with:
```json
{
  "timestamp": "2025-10-09T12:34:56.789Z",
  "level": "INFO",
  "target": "pet_store",
  "fields": {
    "message": "Request processed",
    "method": "GET",
    "path": "/pets",
    "status": 200,
    "duration_ms": 12
  },
  "span": {
    "name": "http_request",
    "http.method": "GET",
    "http.target": "/pets"
  }
}
```

## 🚀 Quick Fix for Now

### Reduce Stdout Noise

Add to `k8s/petstore-deployment.yaml`:

```yaml
env:
  - name: RUST_LOG
    value: "warn,pet_store=info"  # Reduce verbose logs
  - name: RUST_BACKTRACE
    value: "0"  # Disable backtraces in production
```

### View Logs Properly

```bash
# Stream logs with timestamps
kubectl logs -f -n brrtrouter-dev deployment/petstore --timestamps

# Filter specific errors
kubectl logs -n brrtrouter-dev deployment/petstore | grep -v "TooManyHeaders"

# Check if pod is crashing
kubectl describe pod -n brrtrouter-dev -l app=petstore
```

### Check Observability Stack

```bash
# Prometheus
curl http://localhost:9090/api/v1/targets
# Should show petstore as a target

# Grafana
open http://localhost:3000
# Login: admin/admin
# Add Prometheus data source: http://prometheus:9090

# Jaeger
open http://localhost:16686
# Should show services (once tracing is added)
```

## 📋 Implementation Priority

1. **Immediate** (fix crashes):
   - [ ] Reduce RUST_LOG to `warn`
   - [ ] Check pod crashloop: `kubectl describe pod`
   - [ ] Fix TooManyHeaders if it's crashing (unlikely, but check)

2. **Short-term** (proper logging):
   - [ ] Add `tracing` dependencies to templates
   - [ ] Initialize tracing in generated main.rs
   - [ ] Replace println! with tracing macros
   - [ ] Add request spans to AppService

3. **Medium-term** (full observability):
   - [ ] Connect Prometheus scraping
   - [ ] Set up Grafana dashboards
   - [ ] Verify OTEL Collector pipeline
   - [ ] Add custom business metrics

## 🔍 Troubleshooting

### Pod Keeps Crashing

```bash
# Get crash reason
kubectl logs -n brrtrouter-dev deployment/petstore --previous

# Check events
kubectl get events -n brrtrouter-dev --sort-by='.lastTimestamp'

# Check resource limits
kubectl describe pod -n brrtrouter-dev -l app=petstore | grep -A 5 "Limits"
```

### No Metrics in Prometheus

```bash
# Check if Prometheus can reach petstore
kubectl exec -n brrtrouter-dev deployment/prometheus -- wget -qO- http://petstore:8080/metrics

# Check Prometheus config
kubectl get configmap -n brrtrouter-dev prometheus-config -o yaml
```

### No Traces in Jaeger

```bash
# Check OTEL Collector
kubectl logs -n brrtrouter-dev deployment/otel-collector

# Verify endpoint
kubectl exec -n brrtrouter-dev deployment/petstore -- env | grep OTEL
```

---

**Status**: 🚧 Observability Needs Setup  
**Priority**: 🔥 High - Need visibility NOW  
**Effort**: ~2-3 hours implementation  
**Date**: October 9, 2025

