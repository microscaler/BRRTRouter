# Persistent Observability Storage

## Overview

All observability components now use PersistentVolumeClaims (PVCs) to preserve data between pod restarts, redeployments, and cluster recreations.

## What Was Added

### Storage PVCs

**File: `k8s/observability-storage.yaml`**

Four PersistentVolumeClaims for observability data:

1. **prometheus-storage** (5Gi)
   - Stores metrics time-series data
   - Retains request rates, latencies, error rates
   - Path: `/prometheus`

2. **loki-storage** (5Gi)
   - Stores log chunks and indexes
   - Retains all application logs
   - Path: `/loki`

3. **grafana-storage** (1Gi)
   - Stores dashboards, datasources, user preferences
   - Retains custom dashboard configurations
   - Path: `/var/lib/grafana`

4. **jaeger-storage** (2Gi)
   - Stores distributed traces using Badger DB
   - Retains span data for request tracing
   - Paths: `/badger/data` and `/badger/key`

### Changes to Observability Components

#### Prometheus (`k8s/prometheus.yaml`)
```yaml
volumes:
  - name: storage
    persistentVolumeClaim:
      claimName: prometheus-storage
```

**Before**: Used `emptyDir: {}` - data lost on pod restart  
**After**: Uses PVC - data persists across restarts

#### Loki (`k8s/loki.yaml`)
```yaml
volumes:
  - name: storage
    persistentVolumeClaim:
      claimName: loki-storage
```

**Before**: Used `emptyDir: {}` - logs lost on pod restart  
**After**: Uses PVC - logs persist across restarts

#### Grafana (`k8s/grafana.yaml`)
```yaml
volumes:
  - name: storage
    persistentVolumeClaim:
      claimName: grafana-storage
```

**Before**: Used `emptyDir: {}` - dashboards lost on pod restart  
**After**: Uses PVC - dashboards and settings persist

#### Jaeger (`k8s/jaeger.yaml`)
```yaml
env:
  - name: SPAN_STORAGE_TYPE
    value: "badger"
  - name: BADGER_EPHEMERAL
    value: "false"
  - name: BADGER_DIRECTORY_VALUE
    value: "/badger/data"
  - name: BADGER_DIRECTORY_KEY
    value: "/badger/key"
volumeMounts:
  - name: storage
    mountPath: /badger
volumes:
  - name: storage
    persistentVolumeClaim:
      claimName: jaeger-storage
```

**Before**: Used memory storage - traces lost on pod restart  
**After**: Uses Badger DB with PVC - traces persist across restarts

### Tiltfile Updates

```python
# Load observability stack storage (PVCs first)
k8s_yaml('k8s/observability-storage.yaml')

# Load observability stack
k8s_yaml([
    'k8s/prometheus.yaml',
    'k8s/loki.yaml',
    'k8s/promtail.yaml',
    'k8s/grafana.yaml',
    'k8s/grafana-dashboard-unified.yaml',
    'k8s/jaeger.yaml',
    'k8s/otel-collector.yaml',
])
```

PVCs are loaded first to ensure they exist before pods try to claim them.

## Benefits

### 1. **Data Persistence**
- ✅ Metrics survive pod restarts
- ✅ Logs survive pod restarts
- ✅ Traces survive pod restarts
- ✅ Grafana dashboards survive pod restarts

### 2. **Historical Analysis**
- ✅ Review metrics from hours/days ago
- ✅ Correlate logs with past incidents
- ✅ Analyze trace patterns over time
- ✅ Track performance trends

### 3. **Development Workflow**
- ✅ Keep test data between code changes
- ✅ Compare before/after metrics
- ✅ Debug issues with historical context
- ✅ Build up meaningful datasets

### 4. **Production-Like**
- ✅ Mirrors production observability setup
- ✅ Tests realistic storage patterns
- ✅ Validates retention policies
- ✅ Ensures backup/restore works

## Storage Sizes

| Component   | Size | Justification |
|-------------|------|---------------|
| Prometheus  | 5Gi  | Metrics for ~7 days with standard scrape intervals |
| Loki        | 5Gi  | Logs for ~3-5 days depending on verbosity |
| Grafana     | 1Gi  | Dashboards, datasources, user prefs (small) |
| Jaeger      | 2Gi  | Traces for ~2-3 days with moderate traffic |

**Total**: ~13Gi for full observability stack

### Adjusting Sizes

For production or longer retention, edit `k8s/observability-storage.yaml`:

```yaml
spec:
  resources:
    requests:
      storage: 50Gi  # Increase as needed
```

Then apply:
```bash
kubectl apply -f k8s/observability-storage.yaml
# Note: Can't shrink PVCs, only grow them
```

## Verifying Persistent Storage

### Check PVC Status

```bash
# List all PVCs
kubectl get pvc -n brrtrouter-dev

# Expected output:
# NAME                  STATUS   VOLUME     CAPACITY   ACCESS MODES
# prometheus-storage    Bound    pvc-xxx    5Gi        RWO
# loki-storage          Bound    pvc-xxx    5Gi        RWO
# grafana-storage       Bound    pvc-xxx    1Gi        RWO
# jaeger-storage        Bound    pvc-xxx    2Gi        RWO
```

### Check Data Persistence

```bash
# 1. Generate some traffic and check metrics
curl http://localhost:9090/health
open http://localhost:3000  # View in Grafana

# 2. Restart a pod
kubectl rollout restart deployment/prometheus -n brrtrouter-dev
kubectl wait --for=condition=ready pod -l app=prometheus -n brrtrouter-dev --timeout=60s

# 3. Check metrics are still there
open http://localhost:3000  # Historical data should be intact
```

### Check Storage Usage

```bash
# Prometheus storage
kubectl exec -n brrtrouter-dev -l app=prometheus -- df -h /prometheus

# Loki storage
kubectl exec -n brrtrouter-dev -l app=loki -- df -h /loki

# Grafana storage
kubectl exec -n brrtrouter-dev -l app=grafana -- df -h /var/lib/grafana

# Jaeger storage
kubectl exec -n brrtrouter-dev -l app=jaeger -- df -h /badger
```

## Data Retention Policies

### Prometheus

Default retention: **15 days** (configured in `prometheus.yaml`)

```yaml
args:
  - '--config.file=/etc/prometheus/prometheus.yml'
  - '--storage.tsdb.path=/prometheus'
  - '--storage.tsdb.retention.time=15d'  # Adjust as needed
```

### Loki

Retention configured in `loki-config.yaml`:

```yaml
limits_config:
  retention_period: 168h  # 7 days
```

To change:
```bash
kubectl edit configmap loki-config -n brrtrouter-dev
# Update retention_period
kubectl rollout restart deployment/loki -n brrtrouter-dev
```

### Jaeger

Badger has built-in compaction. For manual cleanup:

```bash
# Check trace count
kubectl exec -n brrtrouter-dev -l app=jaeger -- \
  curl -s http://localhost:16686/api/services | jq '.data | length'

# Badger auto-compacts, but you can trigger cleanup by restarting
kubectl rollout restart deployment/jaeger -n brrtrouter-dev
```

## Backing Up Observability Data

### Quick Backup Script

```bash
#!/bin/bash
# backup-observability.sh

BACKUP_DIR="./observability-backups/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$BACKUP_DIR"

echo "📦 Backing up observability data to $BACKUP_DIR"

# Backup Prometheus data
kubectl exec -n brrtrouter-dev -l app=prometheus -- \
  tar czf - /prometheus > "$BACKUP_DIR/prometheus.tar.gz"
echo "✅ Prometheus backed up"

# Backup Loki data
kubectl exec -n brrtrouter-dev -l app=loki -- \
  tar czf - /loki > "$BACKUP_DIR/loki.tar.gz"
echo "✅ Loki backed up"

# Backup Grafana data
kubectl exec -n brrtrouter-dev -l app=grafana -- \
  tar czf - /var/lib/grafana > "$BACKUP_DIR/grafana.tar.gz"
echo "✅ Grafana backed up"

# Backup Jaeger data
kubectl exec -n brrtrouter-dev -l app=jaeger -- \
  tar czf - /badger > "$BACKUP_DIR/jaeger.tar.gz"
echo "✅ Jaeger backed up"

echo ""
echo "🎉 Backup complete: $BACKUP_DIR"
echo "Total size: $(du -sh $BACKUP_DIR | cut -f1)"
```

### Restoring from Backup

```bash
#!/bin/bash
# restore-observability.sh <backup-dir>

BACKUP_DIR="$1"

if [ -z "$BACKUP_DIR" ]; then
  echo "Usage: $0 <backup-dir>"
  exit 1
fi

echo "🔄 Restoring from $BACKUP_DIR"

# Scale down deployments
kubectl scale deployment -n brrtrouter-dev prometheus loki grafana jaeger --replicas=0
sleep 5

# Restore Prometheus
kubectl exec -n brrtrouter-dev -l app=prometheus -- \
  tar xzf - -C / < "$BACKUP_DIR/prometheus.tar.gz"
echo "✅ Prometheus restored"

# (Repeat for other components)

# Scale back up
kubectl scale deployment -n brrtrouter-dev prometheus loki grafana jaeger --replicas=1
echo "🎉 Restore complete"
```

## Cleaning Up Old Data

### When PVCs Fill Up

```bash
# Check which PVC is full
kubectl get pvc -n brrtrouter-dev

# Option 1: Increase PVC size (if supported by storage class)
kubectl edit pvc prometheus-storage -n brrtrouter-dev
# Change storage: 5Gi to storage: 10Gi

# Option 2: Manually clean old data
kubectl exec -n brrtrouter-dev -l app=prometheus -- \
  find /prometheus -type f -mtime +7 -delete

# Option 3: Reduce retention period (see "Data Retention Policies" above)
```

## Production Considerations

### Storage Classes

For production, use a real storage class (not `standard`):

```yaml
spec:
  storageClassName: fast-ssd  # or gp3, premium-rwo, etc.
  accessModes:
    - ReadWriteOnce
```

### Backup Strategy

1. **Automated Backups**: Use Velero, Kasten K10, or cloud-native backup tools
2. **Snapshot Schedule**: Daily snapshots of PVCs
3. **Retention**: Keep 7 daily, 4 weekly, 12 monthly backups
4. **Test Restores**: Verify backups work quarterly

### Monitoring Storage

Add alerts for:
- PVC usage > 80%
- PVC usage > 90%
- PVC full

```promql
# Prometheus alert
(kubelet_volume_stats_used_bytes / kubelet_volume_stats_capacity_bytes) > 0.8
```

## Troubleshooting

### PVC Stuck in Pending

```bash
kubectl describe pvc prometheus-storage -n brrtrouter-dev

# Common issues:
# 1. Storage class doesn't exist
# 2. No available persistent volumes
# 3. Access mode not supported

# For KIND, ensure storage provisioner is running:
kubectl get storageclass
kubectl get pv
```

### Pod Can't Mount PVC

```bash
kubectl describe pod -n brrtrouter-dev -l app=prometheus

# Common issues:
# 1. PVC in different namespace
# 2. Access mode conflict (two pods, RWO PVC)
# 3. PVC not bound yet

# Check events:
kubectl get events -n brrtrouter-dev --sort-by='.lastTimestamp'
```

### Data Not Persisting

```bash
# Verify PVC is actually bound
kubectl get pvc -n brrtrouter-dev prometheus-storage

# Check mount point in pod
kubectl exec -n brrtrouter-dev -l app=prometheus -- mount | grep prometheus

# Check if data is being written
kubectl exec -n brrtrouter-dev -l app=prometheus -- ls -lah /prometheus
```

## Summary

✅ **All observability data now persists across restarts**  
✅ **Prometheus**: 5Gi for metrics (15d retention)  
✅ **Loki**: 5Gi for logs (7d retention)  
✅ **Grafana**: 1Gi for dashboards  
✅ **Jaeger**: 2Gi for traces (Badger DB)  

**Total storage**: ~13Gi for full observability stack

**Files modified:**
- `k8s/observability-storage.yaml` (NEW)
- `k8s/prometheus.yaml` (PVC mount)
- `k8s/loki.yaml` (PVC mount)
- `k8s/grafana.yaml` (PVC mount)
- `k8s/jaeger.yaml` (Badger + PVC mount)
- `Tiltfile` (load PVCs first)

**To apply:**
```bash
# Tilt will auto-reload, or manually:
kubectl apply -f k8s/observability-storage.yaml
kubectl rollout restart deployment -n brrtrouter-dev prometheus loki grafana jaeger
```

🎉 **Your observability data is now safe!**

