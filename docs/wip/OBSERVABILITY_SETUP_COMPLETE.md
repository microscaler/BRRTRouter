# Observability Stack Setup Complete

## What Was Fixed

The Grafana dashboards weren't showing up because:

1. ❌ **Wrong JSON structure** - Dashboard JSON was wrapped in `{"dashboard": {...}}` instead of just `{...}`
2. ❌ **Volume mounts incorrect** - Dashboards weren't properly mounted as individual files
3. ❌ **Missing unified dashboard** - The comprehensive observability dashboard wasn't added to Tilt

## What's Configured Now

### ✅ Data Sources

All three datasources are configured in Grafana:

1. **Prometheus** (default) - Metrics from pet store
   - URL: `http://prometheus:9090`
   - Scraping: `petstore:8080/metrics`

2. **Loki** - Logs from all pods
   - URL: `http://loki:3100`
   - Collection: Promtail DaemonSet

3. **Jaeger** - Distributed traces
   - URL: `http://jaeger:16686`
   - OTLP export configured

### ✅ Dashboards

Two dashboards are now available:

1. **BRRTRouter Pet Store - Quick View**
   - Request rate graph
   - Response latency (p50, p95)
   - Quick overview

2. **BRRTRouter - Unified Observability** ⭐
   - Request rate & latency
   - Active requests & total requests
   - Error rate with thresholds
   - **Live application logs** (last 100 lines)
   - HTTP status codes pie chart
   - Top endpoints by request count
   - Request duration heatmap
   - Memory & CPU usage

## How to Access

### Grafana

```bash
# Access Grafana
open http://localhost:3000

# Login (auto-login enabled)
# Username: admin
# Password: admin
```

**To see dashboards:**
1. Click "Dashboards" in left sidebar (four squares icon)
2. You'll see both dashboards listed
3. Click on "BRRTRouter - Unified Observability" for the full view

### Direct Links

```bash
# Grafana
http://localhost:3000

# Prometheus
http://localhost:8080

# Jaeger
http://localhost:16686

# Pet Store (generate traffic)
http://localhost:9090/health
http://localhost:9090/pets
http://localhost:9090/metrics
```

## Generate Traffic to See Data

The dashboards need data! Generate some traffic:

```bash
# Health checks
for i in {1..100}; do curl http://localhost:9090/health; done

# API requests
for i in {1..50}; do curl http://localhost:9090/pets; done
for i in {1..50}; do curl http://localhost:9090/users; done

# With API key
for i in {1..50}; do curl -H "X-API-Key: test123" http://localhost:9090/pets; done

# Mixed endpoints
for i in {1..20}; do
  curl http://localhost:9090/health
  curl http://localhost:9090/metrics  
  curl -H "X-API-Key: test123" http://localhost:9090/pets
  curl -H "X-API-Key: test123" http://localhost:9090/users
  sleep 0.1
done
```

## Verify Everything is Working

```bash
# Quick verification
just dev-observability-verify
```

**Expected output:**
```
🔍 Verifying Observability Stack

1. Checking pod status...
   ✓ prometheus: Running
   ✓ grafana: Running
   ✓ loki: Running
   ✓ promtail: Running
   ✓ jaeger: Running
   ✓ otel-collector: Running
   ✓ petstore: Running

2. Checking Prometheus metrics...
   ✓ Prometheus has petstore target configured
   ✓ Prometheus is collecting petstore metrics

3. Checking Loki logs...
   ✓ Loki is ready
   ✓ Promtail is running (1 instance(s))

4. Checking Grafana datasources...
   ✓ Prometheus datasource configured
   ✓ Loki datasource configured
   ✓ Jaeger datasource configured

✅ Observability Stack Check Complete
```

## Using the Dashboards

### Metrics (Prometheus)

In Grafana:
1. Go to "Explore" (compass icon)
2. Select "Prometheus" datasource
3. Try these queries:

```promql
# Request rate
rate(brrtrouter_requests_total[1m])

# Latency p95
histogram_quantile(0.95, rate(brrtrouter_request_duration_seconds_bucket[1m]))

# Active requests
brrtrouter_active_requests

# Error rate
rate(brrtrouter_requests_total{status=~"5.."}[1m])
```

### Logs (Loki)

In Grafana:
1. Go to "Explore"
2. Select "Loki" datasource
3. Try these queries:

```logql
# All petstore logs
{app="petstore"}

# Error logs only
{app="petstore"} |= "error" or "ERROR"

# Specific endpoint
{app="petstore"} |= "/pets"

# JSON parsing
{app="petstore"} | json | status >= 400
```

### Traces (Jaeger)

In Grafana:
1. Go to "Explore"
2. Select "Jaeger" datasource
3. Search for service: "petstore"

Or directly in Jaeger UI:
- http://localhost:16686

## Unified Dashboard Features

The "BRRTRouter - Unified Observability" dashboard shows:

### 📊 Metrics Section (Top)
- **Request Rate**: Real-time requests per second by endpoint
- **Response Latency**: p50, p95, p99 latency percentiles
- **Active Requests**: Current concurrent requests
- **Total Requests**: Cumulative request counter
- **Error Rate**: 5xx errors with color thresholds (green/yellow/red)

### 📝 Logs Section (Middle)
- **Live Application Logs**: Last 100 log lines
- Shows timestamps, log levels, and full messages
- Auto-refreshes every 10 seconds
- Scroll to see history

### 📈 Analytics Section (Bottom)
- **HTTP Status Codes**: Pie chart of response codes
- **Top Endpoints**: Bar chart of busiest endpoints
- **Request Duration Heatmap**: Visual latency distribution
- **Memory Usage**: RSS memory over time
- **CPU Usage**: CPU utilization percentage

## Troubleshooting

### No Dashboards Showing

```bash
# Restart Grafana pod
kubectl rollout restart deployment/grafana -n brrtrouter-dev

# Wait for it to be ready
kubectl wait --for=condition=ready pod -l app=grafana -n brrtrouter-dev --timeout=60s

# Check if ConfigMaps exist
kubectl get configmap -n brrtrouter-dev | grep dashboard
# Should show:
#   grafana-dashboard-petstore
#   grafana-dashboard-unified
```

### No Metrics

```bash
# Check if Prometheus is scraping
kubectl port-forward svc/prometheus 9090:9090 -n brrtrouter-dev
open http://localhost:9090/targets
# Should show petstore target as "UP"

# Check pet store is exposing metrics
curl http://localhost:9090/metrics | grep brrtrouter
```

### No Logs

```bash
# Check Promtail is running
kubectl get pods -n brrtrouter-dev -l app=promtail

# Check Loki is receiving logs
kubectl logs -n brrtrouter-dev -l app=loki | grep "push"

# Generate some logs
curl http://localhost:9090/health
curl http://localhost:9090/pets
```

### Datasources Not Working

```bash
# Check datasource configuration
kubectl exec -n brrtrouter-dev -l app=grafana -- cat /etc/grafana/provisioning/datasources/datasources.yaml

# Test Prometheus connection from Grafana pod
kubectl exec -n brrtrouter-dev -l app=grafana -- wget -q -O- http://prometheus:9090/api/v1/targets
```

## Files Changed

1. **`k8s/grafana.yaml`** - Fixed dashboard JSON structure and volume mounts
2. **`k8s/grafana-dashboard-unified.yaml`** - NEW: Comprehensive observability dashboard
3. **`Tiltfile`** - Added unified dashboard to deployment
4. **`scripts/verify-observability.sh`** - NEW: Verification script
5. **`justfile`** - Added `dev-observability-verify` command

## Next Steps

1. **Apply the changes:**
   ```bash
   # Tilt will auto-reload, or manually:
   kubectl apply -f k8s/grafana.yaml
   kubectl apply -f k8s/grafana-dashboard-unified.yaml
   kubectl rollout restart deployment/grafana -n brrtrouter-dev
   ```

2. **Access Grafana:**
   ```bash
   open http://localhost:3000
   ```

3. **View dashboards:**
   - Click "Dashboards" → "BRRTRouter - Unified Observability"

4. **Generate traffic:**
   ```bash
   for i in {1..100}; do curl http://localhost:9090/health; sleep 0.1; done
   ```

5. **Watch the data flow in!** 📊📝📈

## Summary

✅ **Dashboards**: Fixed JSON structure and mounted correctly  
✅ **Datasources**: Prometheus, Loki, Jaeger all configured  
✅ **Metrics**: Prometheus scraping petstore  
✅ **Logs**: Promtail → Loki → Grafana  
✅ **Unified View**: Single dashboard with metrics, logs, and analytics  
✅ **Verification**: `just dev-observability-verify` command  

**Everything is ready for full observability!** 🎉

