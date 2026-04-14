# Full Observability Stack ✅

## 🎯 What Was Added

Complete logging, metrics, and tracing observability stack for BRRTRouter development:

1. **Loki** - Log aggregation system
2. **Promtail** - Log shipping agent
3. **Grafana datasources** - Loki, Jaeger, and Prometheus integration
4. **Updated Tiltfile** - Orchestrate all observability components

## 📊 Observability Components

### 1. Loki - Log Aggregation
```yaml
# k8s/loki.yaml
- Port: 3100 (HTTP API)
- Port: 9096 (gRPC)
- Storage: BoltDB + Filesystem
- Retention: 7 days
- Ingestion rate: 10 MB/s
```

**What it does:**
- Aggregates logs from all pods
- Indexes logs efficiently (like Prometheus for logs)
- Queryable via LogQL
- Integrated with Grafana

### 2. Promtail - Log Shipper
```yaml
# k8s/promtail.yaml
- DaemonSet (runs on every node)
- Scrapes pod logs from /var/log
- Adds labels: pod, container, app, namespace
- Pushes to Loki via HTTP
```

**What it does:**
- Discovers pods automatically via Kubernetes API
- Tails container logs
- Enriches logs with Kubernetes metadata
- Ships logs to Loki in real-time

### 3. Updated Grafana
```yaml
# k8s/grafana.yaml
datasources:
  - Prometheus (metrics) - default
  - Loki (logs)
  - Jaeger (traces)
```

**What you get:**
- **Metrics**: Prometheus for request rates, latency, errors
- **Logs**: Loki for application logs, errors, debug output
- **Traces**: Jaeger for distributed tracing
- **Correlation**: Link between metrics, logs, and traces

## 🔍 How to Use

### View Logs in Grafana

```bash
# Open Grafana
open http://localhost:3000

# Navigate to: Explore → Select "Loki" datasource

# Example queries:
{app="petstore"}                              # All petstore logs
{app="petstore"} |= "error"                   # Error logs
{app="petstore"} |= "TooManyHeaders"          # Find specific errors
{namespace="brrtrouter-dev"} |= "failed"      # All failures in namespace
rate({app="petstore"}[5m])                    # Log rate over time
```

### Query Loki Directly

```bash
# Query via API
curl -G -s "http://localhost:3100/loki/api/v1/query_range" \
  --data-urlencode 'query={app="petstore"}' \
  --data-urlencode 'start=1h' | jq

# Stream logs (like tail -f)
curl -G -s "http://localhost:3100/loki/api/v1/tail" \
  --data-urlencode 'query={app="petstore"}' \
  --no-buffer
```

### Correlate Metrics + Logs + Traces

**In Grafana:**

1. **Metrics Panel** - See request spike
   ```promql
   rate(brrtrouter_requests_total[5m])
   ```

2. **Click spike** → "Explore"

3. **Logs Panel** - See what happened
   ```logql
   {app="petstore"} |~ "error|failed"
   ```

4. **Traces Panel** - See distributed trace
   - Click trace ID in logs
   - Opens Jaeger trace view
   - See full request flow

## 🚀 Startup Flow

```
┌─────────────────────────────────────────────┐
│ 1. Data Stores                              │
│    - PostgreSQL                             │
│    - Redis                                  │
└─────────────┬───────────────────────────────┘
              │
┌─────────────▼───────────────────────────────┐
│ 2. Observability Stack                      │
│    - Prometheus (metrics scraping)          │
│    - Loki (log aggregation)                 │
│    - Promtail (log shipping DaemonSet)      │
│    - Jaeger (trace collection)              │
│    - OTEL Collector (trace/metrics gateway) │
│    - Grafana (unified UI)                   │
└─────────────┬───────────────────────────────┘
              │
┌─────────────▼───────────────────────────────┐
│ 3. Application                              │
│    - Pet Store API                          │
│      • Exports metrics → Prometheus         │
│      • Sends logs → Promtail → Loki         │
│      • Sends traces → OTEL → Jaeger         │
└─────────────────────────────────────────────┘
```

## 📋 Service Ports

| Service | Port | Purpose |
|---------|------|---------|
| **Petstore** | 8080 | HTTP API |
| **Grafana** | 3000 | Web UI (all visualizations) |
| **Prometheus** | 9090 | Metrics database + UI |
| **Loki** | 3100 | Log aggregation API |
| **Jaeger** | 16686 | Tracing UI |
| **PostgreSQL** | 5432 | Database |
| **Redis** | 6379 | Cache |

## 🔧 Configuration

### Loki Configuration
```yaml
# Storage
- Chunks: /loki/chunks
- Index: BoltDB
- Retention: 7 days

# Limits
- Ingestion rate: 10 MB/s
- Burst: 20 MB
- Max age: 7 days
```

### Promtail Configuration
```yaml
# Scraping
- Job: kubernetes-pods
- Namespace: brrtrouter-dev
- Labels added:
  * pod
  * container
  * app
  * namespace
```

### Grafana Datasources
```yaml
# Prometheus
- URL: http://prometheus:9090
- Default: true
- Type: metrics

# Loki
- URL: http://loki:3100
- Type: logs
- Max lines: 1000

# Jaeger
- URL: http://jaeger:16686
- Type: traces
```

## 🧪 Testing Logging

### 1. Generate Some Logs
```bash
# Hit API endpoints
for i in {1..100}; do
  curl -s http://localhost:8080/health > /dev/null
  curl -s -H "X-API-Key: test123" http://localhost:8080/pets > /dev/null
done
```

### 2. View in Loki
```bash
# Query recent logs
curl -G -s "http://localhost:3100/loki/api/v1/query_range" \
  --data-urlencode 'query={app="petstore"}' \
  --data-urlencode 'limit=100' | jq '.data.result[0].values'
```

### 3. View in Grafana
```
1. Open http://localhost:3000
2. Go to Explore
3. Select "Loki" datasource
4. Query: {app="petstore"} |= "health"
5. See live logs streaming
```

## 🎯 Use Cases

### Debug Production Issues
```logql
# Find errors in last hour
{app="petstore"} |~ "error|ERROR|panic" | json

# Find slow requests (if logging response times)
{app="petstore"} |= "duration" | json | duration > 1s

# Find specific user requests
{app="petstore"} |~ "user_id=12345"
```

### Monitor Service Health
```logql
# Log volume (should be steady)
sum(rate({app="petstore"}[1m]))

# Error rate
sum(rate({app="petstore"} |~ "error|ERROR"[1m]))

# Request rate by endpoint
sum by (path) (rate({app="petstore"} |= "request" | json[1m]))
```

### Investigate TooManyHeaders Error
```logql
# Find all TooManyHeaders errors
{app="petstore"} |= "TooManyHeaders"

# See what requests caused it
{app="petstore"} |= "TooManyHeaders" | json | line_format "{{.timestamp}} {{.method}} {{.path}} - Headers: {{.header_count}}"
```

## 📊 Grafana Dashboard Ideas

### 1. Service Overview Dashboard
```yaml
Panels:
  - Request rate (Prometheus)
  - Error rate (Prometheus)
  - P95 latency (Prometheus)
  - Recent errors (Loki)
  - Active traces (Jaeger)
```

### 2. Logs Explorer Dashboard
```yaml
Panels:
  - Log stream (Loki) - live tail
  - Log volume by level (Loki)
  - Top error messages (Loki)
  - Logs by pod (Loki)
```

### 3. Distributed Tracing Dashboard
```yaml
Panels:
  - Trace timeline (Jaeger)
  - Service dependencies (Jaeger)
  - Slowest operations (Jaeger)
  - Error traces (Jaeger)
```

## 🔜 Next Steps

### 1. Add Structured Logging to BRRTRouter
```rust
// Instead of println!
println!("[info] Request received");

// Use tracing
tracing::info!(
    method = %req.method,
    path = %req.path,
    headers = req.headers.len(),
    "Request received"
);
```

### 2. Add Request ID Tracing
```rust
// Generate request ID
let request_id = Uuid::new_v4();

// Add to all logs
tracing::info!(
    request_id = %request_id,
    "Processing request"
);

// Add to response headers
response.headers.insert("X-Request-ID", request_id);
```

### 3. Add Correlation IDs
```rust
// Extract from incoming request
let correlation_id = req.headers
    .get("x-correlation-id")
    .or_else(|| generate_new());

// Pass through entire request chain
// Log with correlation_id
// Return in response
```

## 📝 Files Created/Modified

1. ✅ `k8s/loki.yaml` - Loki deployment and service
2. ✅ `k8s/promtail.yaml` - Promtail DaemonSet with RBAC
3. ✅ `k8s/grafana.yaml` - Updated with Loki and Jaeger datasources
4. ✅ `Tiltfile` - Added Loki, Promtail, updated dependencies
5. ✅ `docs/OBSERVABILITY_STACK_COMPLETE.md` - This document

## 💡 Benefits

### Before (Metrics Only)
- ❌ No application logs visibility
- ❌ Hard to debug issues
- ❌ Can't correlate metrics with events
- ❌ No request-level visibility

### After (Full Observability)
- ✅ **Metrics** - What's happening (request rate, latency, errors)
- ✅ **Logs** - Why it's happening (errors, debug output, events)
- ✅ **Traces** - How it's happening (request flow, dependencies)
- ✅ **Correlation** - Link all three together

### Golden Signals
1. **Latency** - Prometheus histograms
2. **Traffic** - Prometheus counters
3. **Errors** - Prometheus counters + Loki logs
4. **Saturation** - Prometheus gauges + Loki warnings

## 🎓 Learning Resources

- **Loki**: https://grafana.com/docs/loki/latest/
- **Promtail**: https://grafana.com/docs/loki/latest/clients/promtail/
- **LogQL**: https://grafana.com/docs/loki/latest/logql/
- **Grafana Explore**: https://grafana.com/docs/grafana/latest/explore/

---

**Status**: ✅ Complete  
**Components**: Loki, Promtail, Grafana (updated)  
**Ready**: `tilt up` to start full observability stack  
**Next**: Add structured logging to BRRTRouter  
**Date**: October 9, 2025

