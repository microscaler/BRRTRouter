# Final Proven OpenTelemetry Versions ✅

## 🎯 Exact Versions from obsctl

These are the **exact** versions from microscaler/obsctl that work with `otel-collector-contrib:0.93.0`:

```toml
# OpenTelemetry observability (PROVEN VERSIONS from obsctl)
# CRITICAL: These versions work with otel-collector-contrib:0.93.0
# DO NOT UPDATE without testing against OTEL collector!
tracing = "0.1"
opentelemetry = { version = "0.30", features = ["metrics", "trace"] }
opentelemetry-otlp = { version = "0.30", features = ["grpc-tonic", "metrics", "trace"] }
opentelemetry_sdk = { version = "0.30", features = ["metrics", "trace"] }
opentelemetry-semantic-conventions = "0.30"
tracing-opentelemetry = "0.31"  # ← Note: 0.31, not 0.30!
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-appender = "0.2"

# gRPC dependencies for OTLP (compatible with opentelemetry 0.30)
tonic = "0.12"
prost = "0.13"
tokio = { version = "1.45.1", features = ["rt-multi-thread", "macros"] }
```

## 📊 Version Matrix

| Crate | Version | Critical? |
|-------|---------|-----------|
| `opentelemetry` | 0.30 | ✅ YES |
| `opentelemetry-otlp` | 0.30 | ✅ YES |
| `opentelemetry_sdk` | 0.30 | ✅ YES |
| `opentelemetry-semantic-conventions` | 0.30 | ✅ YES |
| `tracing` | 0.1 | ⚠️ Standard |
| `tracing-opentelemetry` | **0.31** | ✅ YES (not 0.30!) |
| `tracing-subscriber` | 0.3 | ⚠️ Standard |
| `tracing-appender` | 0.2 | ⚠️ Standard |
| `tonic` | 0.12 | ✅ YES |
| `prost` | 0.13 | ✅ YES |
| `tokio` | 1.45.1 | ⚠️ Flexible |

## ⚠️ Critical Notes

### 1. tracing-opentelemetry is 0.31

**This is intentional!**
- `opentelemetry = 0.30`
- `tracing-opentelemetry = 0.31` ← One version higher!

This is because `tracing-opentelemetry` 0.31 is compatible with `opentelemetry` 0.30.

### 2. Features Matter

```toml
# ❌ WRONG - Missing features
opentelemetry = "0.30"

# ✅ CORRECT - With features
opentelemetry = { version = "0.30", features = ["metrics", "trace"] }
```

### 3. tonic and prost versions

These MUST match the opentelemetry-otlp requirements:
- `tonic = "0.12"` (not 0.10, not 0.13)
- `prost = "0.13"` (not 0.12, not 0.14)

## 🚀 OTEL Collector Version

**MUST USE:**
```yaml
image: otel/opentelemetry-collector-contrib:0.93.0
```

**Why contrib?**
- Includes `loki` exporter
- Includes `prometheus` exporter
- More complete than base image

## 📝 Full Stack

```
┌─────────────────────────────────────┐
│ Application (BRRTRouter)            │
│ - tracing-subscriber (0.3)          │
│ - tracing-opentelemetry (0.31)      │
│ - opentelemetry (0.30)              │
│ - opentelemetry-otlp (0.30)         │
└──────────────┬──────────────────────┘
               │ OTLP/gRPC (tonic 0.12)
               ▼
┌─────────────────────────────────────┐
│ OTEL Collector (0.93.0-contrib)     │
│ - Receives: traces, metrics, logs   │
│ - Processes: batch, memory_limiter  │
│ - Exports: Jaeger, Prometheus, Loki │
└──────────────┬──────────────────────┘
               │
      ┌────────┼────────┐
      ▼        ▼        ▼
   Jaeger  Prometheus  Loki
      └────────┼────────┘
               ▼
            Grafana
```

## 🧪 Test After Update

```bash
# 1. Update dependencies
cargo update

# 2. Build
cargo build --release

# 3. Check for version conflicts
cargo tree | grep opentelemetry
# Should see all 0.30 (except tracing-opentelemetry 0.31)

# 4. Check gRPC dependencies
cargo tree | grep tonic
# Should see 0.12

# 5. Run tests
cargo test

# 6. Deploy and test
tilt down
tilt up

# 7. Verify OTLP connection
kubectl logs -n brrtrouter-dev deployment/otel-collector | grep "OTLP"
```

## 🔍 Debugging Version Issues

### If you see "trait bound" errors:

```
error[E0277]: the trait bound `...` is not satisfied
```

**Likely cause:** Version mismatch between:
- `opentelemetry` and `opentelemetry-otlp`
- `opentelemetry` and `tracing-opentelemetry`
- `tonic` version incompatible with `opentelemetry-otlp`

**Fix:** Use exact versions above.

### If OTLP connection fails:

```bash
# Check OTEL collector logs
kubectl logs -n brrtrouter-dev deployment/otel-collector

# Should see:
# "OTLP gRPC server started on 0.0.0.0:4317"

# Check app can reach collector
kubectl exec -n brrtrouter-dev deployment/petstore -- nc -zv otel-collector 4317
```

### If no traces appear in Jaeger:

1. Check app is sending:
   ```bash
   # Look for span exports in app logs
   kubectl logs -n brrtrouter-dev deployment/petstore | grep -i "span"
   ```

2. Check OTEL collector is receiving:
   ```bash
   kubectl logs -n brrtrouter-dev deployment/otel-collector | grep -i "received"
   ```

3. Check Jaeger is receiving:
   ```bash
   kubectl logs -n brrtrouter-dev deployment/jaeger | grep -i "span"
   ```

## 💾 Save This Configuration

**DO NOT change these versions unless:**
1. You're prepared to spend hours debugging
2. You've tested with a new OTEL Collector version
3. You've verified all compatibility matrices
4. You have a full day to dedicate to it

**These versions took significant time to get working in obsctl. Trust the proven configuration!**

---

**Source**: microscaler/obsctl (production-tested)  
**Status**: ✅ Final, proven configuration  
**OTEL Collector**: 0.93.0 (contrib)  
**Key difference**: tracing-opentelemetry is 0.31, not 0.30  
**Date**: October 9, 2025

