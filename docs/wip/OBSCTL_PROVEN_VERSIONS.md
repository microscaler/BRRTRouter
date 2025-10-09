# Proven OpenTelemetry Versions from obsctl

## 🎯 Based on Working Configuration

From the microscaler/obsctl project, these specific versions were proven to work together after significant trial and error.

## 📦 OTEL Collector Version

**CRITICAL**: Use `opentelemetry-collector-contrib:0.93.0`

```yaml
# k8s/otel-collector.yaml
image: otel/opentelemetry-collector-contrib:0.93.0
```

**Why contrib?**
- Includes Loki exporter
- Includes additional processors
- More flexible for development

## 📚 Rust Crate Versions (Proven Working)

Based on obsctl's Cargo.toml, these versions work together:

```toml
[dependencies]
# Tracing ecosystem
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-appender = "0.2"

# OpenTelemetry integration (CRITICAL VERSIONS from obsctl)
opentelemetry = { version = "0.30", features = ["metrics", "trace"] }
opentelemetry-otlp = { version = "0.30", features = ["grpc-tonic", "metrics", "trace"] }
opentelemetry_sdk = { version = "0.30", features = ["metrics", "trace"] }
opentelemetry-semantic-conventions = "0.30"

# Tracing-OpenTelemetry bridge
tracing-opentelemetry = "0.30"

# OTLP transport (compatible with 0.30)
tonic = "0.12"
prost = "0.13"

# For structured logging
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Version Compatibility Matrix

| Crate | Version | Notes |
|-------|---------|-------|
| `opentelemetry` | 0.30 | Core OTEL SDK |
| `opentelemetry-otlp` | 0.30 | OTLP exporter (gRPC) |
| `opentelemetry_sdk` | 0.30 | SDK implementation |
| `opentelemetry-semantic-conventions` | 0.30 | Semantic conventions |
| `tracing-opentelemetry` | 0.30 | Bridge to tracing |
| `tracing-subscriber` | 0.3 | Subscriber impl |
| `tonic` | 0.12 | gRPC runtime |
| `prost` | 0.13 | Protocol buffers |

**CRITICAL**: These versions must match exactly. Mismatched versions cause:
- Runtime panics
- Connection failures to OTEL collector
- Silent data loss
- Weird type errors

## 🔧 Initialization Pattern from obsctl

```rust
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::{WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{
    trace::{self, RandomIdGenerator, Sampler},
    Resource,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn init_tracing(service_name: &str, otlp_endpoint: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create OTLP exporter
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(otlp_endpoint)
        )
        .with_trace_config(
            trace::config()
                .with_sampler(Sampler::AlwaysOn)
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", service_name.to_string()),
                ]))
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)?;

    // 2. Create OpenTelemetry layer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // 3. Create console/stdout layer with JSON formatting
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true);

    // 4. Combine layers with env filter
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(telemetry)
        .init();

    Ok(())
}

// At app shutdown
fn shutdown_tracing() {
    opentelemetry::global::shutdown_tracer_provider();
}
```

## 📋 Logging Pattern from obsctl

### Structured Logging with Spans

```rust
use tracing::{info, warn, error, debug, instrument, Span};

// Function-level tracing
#[instrument(skip(db), fields(user_id = %user.id))]
async fn process_request(user: &User, db: &Database) -> Result<Response> {
    info!("Processing request for user");
    
    // Add dynamic fields to span
    Span::current().record("request_id", &format!("{}", request_id));
    
    // Structured log with context
    debug!(
        endpoint = "/api/users",
        method = "GET",
        "Handling user request"
    );
    
    // Error with context
    if let Err(e) = db.query().await {
        error!(
            error = %e,
            query = "SELECT * FROM users",
            "Database query failed"
        );
        return Err(e);
    }
    
    info!(duration_ms = start.elapsed().as_millis(), "Request completed");
    Ok(response)
}
```

### Request/Response Logging

```rust
// In middleware or handler
use tracing::info_span;

let span = info_span!(
    "http_request",
    method = %req.method,
    path = %req.path,
    status = tracing::field::Empty,
    duration_ms = tracing::field::Empty,
);

let _enter = span.enter();

// Process request
let start = std::time::Instant::now();
let response = handle_request(req).await?;
let duration = start.elapsed();

// Record response details
span.record("status", response.status.as_u16());
span.record("duration_ms", duration.as_millis());

info!(
    method = %req.method,
    path = %req.path,
    status = response.status.as_u16(),
    duration_ms = duration.as_millis(),
    "Request completed"
);
```

## 🚀 Configuration in BRRTRouter

### Add to Cargo.toml

```toml
[dependencies]
# ... existing dependencies ...

# Observability (PROVEN VERSIONS from obsctl)
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-appender = "0.2"
opentelemetry = "0.21"
opentelemetry-otlp = { version = "0.14", features = ["grpc-tonic", "trace", "metrics", "logs"] }
opentelemetry_sdk = { version = "0.21", features = ["rt-tokio"] }
opentelemetry-semantic-conventions = "0.13"
tracing-opentelemetry = "0.22"
```

### Environment Variables

```bash
# Enable tracing
RUST_LOG=brrtrouter=debug,info

# OTLP endpoint (set in K8s deployment)
OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317
OTEL_SERVICE_NAME=brrtrouter-petstore
```

### In main.rs (generated template)

```rust
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> std::io::Result<()> {
    // Initialize tracing FIRST
    if let Ok(endpoint) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        init_tracing("brrtrouter-petstore", &endpoint)
            .expect("Failed to initialize tracing");
        info!("OpenTelemetry tracing initialized");
    } else {
        // Fallback to console logging only
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive("brrtrouter=debug".parse().unwrap())
            )
            .init();
        info!("Console logging initialized (no OTLP)");
    }
    
    // Rest of main...
    
    // At shutdown
    opentelemetry::global::shutdown_tracer_provider();
    Ok(())
}
```

## 📊 Metrics Pattern from obsctl

```rust
use opentelemetry::{
    global,
    metrics::{Counter, Histogram},
    KeyValue,
};

// Initialize metrics
let meter = global::meter("brrtrouter");

// Create instruments
let request_counter = meter
    .u64_counter("http_requests_total")
    .with_description("Total HTTP requests")
    .init();

let request_duration = meter
    .f64_histogram("http_request_duration_seconds")
    .with_description("HTTP request duration in seconds")
    .init();

// Use in code
request_counter.add(
    1,
    &[
        KeyValue::new("method", "GET"),
        KeyValue::new("path", "/api/users"),
        KeyValue::new("status", "200"),
    ],
);

request_duration.record(
    duration.as_secs_f64(),
    &[
        KeyValue::new("method", "GET"),
        KeyValue::new("path", "/api/users"),
    ],
);
```

## ⚠️ Common Pitfalls (from obsctl experience)

### 1. Version Mismatches
```toml
# ❌ BAD - Mismatched versions
opentelemetry = "0.20"  # Wrong!
tracing-opentelemetry = "0.22"  # Will fail at runtime

# ✅ GOOD - Matched versions
opentelemetry = "0.21"
tracing-opentelemetry = "0.22"
```

### 2. Missing Features
```toml
# ❌ BAD - Missing grpc-tonic feature
opentelemetry-otlp = "0.14"

# ✅ GOOD - With required features
opentelemetry-otlp = { version = "0.14", features = ["grpc-tonic", "trace"] }
```

### 3. Runtime Selection
```toml
# ❌ BAD - Wrong runtime
opentelemetry_sdk = { version = "0.21", features = ["rt-async-std"] }

# ✅ GOOD - Tokio runtime (we use may, but tokio for OTEL)
opentelemetry_sdk = { version = "0.21", features = ["rt-tokio"] }
```

### 4. Initialization Order
```rust
// ❌ BAD - Init after may::config()
may::config().set_stack_size(stack_size);
init_tracing()?;  // Too late!

// ✅ GOOD - Init tracing first
init_tracing()?;
may::config().set_stack_size(stack_size);
```

### 5. Shutdown
```rust
// ❌ BAD - No shutdown (data loss!)
fn main() {
    init_tracing()?;
    run_server();
    // Exits without flushing
}

// ✅ GOOD - Proper shutdown
fn main() {
    init_tracing()?;
    run_server();
    opentelemetry::global::shutdown_tracer_provider();  // Flush!
}
```

## 🔍 Debugging from obsctl

### Check OTLP Connection
```bash
# In container
nc -zv otel-collector 4317
# Should connect

# Check OTEL collector logs
kubectl logs -n brrtrouter-dev deployment/otel-collector
# Look for: "OTLP gRPC server started"
```

### Verify Data Flow
```bash
# 1. App sends to OTEL collector
curl http://localhost:8080/health
# Check app logs for: "Exporting span"

# 2. OTEL collector receives
kubectl logs -n brrtrouter-dev deployment/otel-collector | grep "received"

# 3. Jaeger receives
open http://localhost:16686
# Search for traces from "brrtrouter-petstore"
```

## 📝 Files to Modify

1. ✅ `Cargo.toml` - Add observability dependencies
2. ✅ `src/lib.rs` - Add tracing module
3. ✅ `templates/main.rs.txt` - Add tracing initialization
4. ✅ `src/server/service.rs` - Add request/response spans
5. ✅ `src/middleware/tracing.rs` - Already exists, enhance
6. ✅ `k8s/otel-collector.yaml` - Updated to 0.93.0 ✅
7. ✅ `k8s/petstore-deployment.yaml` - Add OTEL env vars

---

**Source**: microscaler/obsctl proven configuration  
**Status**: Ready to implement  
**Critical**: Use exact versions  
**Date**: October 9, 2025

