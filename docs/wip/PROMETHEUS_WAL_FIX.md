# Prometheus WAL Corruption Fix

## Issue
Prometheus encountered a WAL (Write-Ahead Log) corruption error on startup:
```
ts=2025-10-20T07:22:49.764Z caller=db.go:898 level=warn component=tsdb msg="Encountered WAL read error, attempting repair" err="backfill checkpoint: corruption in segment 00000000 at 275530: decode tombstones: invalid size"
```

## Root Cause
WAL corruption can occur due to:
1. Unclean shutdown of Prometheus
2. Storage issues or disk full conditions
3. Pod eviction during write operations
4. Network storage latency issues

## Resolution
Prometheus has built-in WAL repair capabilities. The fix was applied automatically:

1. **Detection**: Prometheus detected the corruption on startup
2. **Repair Process**:
   - Identified corrupted segment (segment 0 at offset 275530)
   - Deleted all segments newer than the corrupted one
   - Rewrote the corrupted segment
3. **Success**: `Successfully repaired WAL` message confirmed the fix

### Manual Steps Taken
```bash
# Deleted the pod to trigger a fresh start
kubectl delete pod -n brrtrouter-dev -l app=prometheus

# Verified the new pod is healthy
kubectl get pods -n brrtrouter-dev | grep prometheus

# Checked logs for successful repair
kubectl logs -n brrtrouter-dev -l app=prometheus | grep WAL
```

## Prevention

### 1. Graceful Shutdown
Ensure Prometheus pods have adequate time to shut down:
```yaml
spec:
  terminationGracePeriodSeconds: 60
```

### 2. Persistent Storage
Use reliable persistent storage with sufficient IOPS

### 3. Resource Limits
Ensure adequate memory and CPU to prevent OOM kills:
```yaml
resources:
  requests:
    memory: "512Mi"
    cpu: "250m"
  limits:
    memory: "1Gi"
    cpu: "500m"
```

### 4. Regular Backups
Consider using Prometheus snapshots for backup:
```bash
curl -XPOST http://prometheus:9090/api/v1/admin/tsdb/snapshot
```

## Known Issue: PVC Binding

There's a PVC binding issue where:
- `prometheus-storage` is bound to `loki-pv`
- `loki-storage` is bound to `prometheus-pv`

This cross-binding doesn't affect functionality but should be fixed in the next maintenance window:
```bash
# Current state (incorrect)
prometheus-storage → loki-pv
loki-storage → prometheus-pv

# Should be
prometheus-storage → prometheus-pv
loki-storage → loki-pv
```

## Monitoring

To monitor for future WAL issues:
1. Check Prometheus metrics: `prometheus_tsdb_wal_corruptions_total`
2. Set up alerts for WAL corruption events
3. Monitor disk usage: `prometheus_tsdb_storage_blocks_bytes`

## Recovery Options

If automatic repair fails:

### Option 1: Clear WAL (data loss)
```bash
kubectl exec -n brrtrouter-dev -it <prometheus-pod> -- sh
rm -rf /prometheus/wal/*
```

### Option 2: Restore from snapshot
```bash
kubectl cp backup/prometheus-snapshot.tar.gz <prometheus-pod>:/prometheus/
kubectl exec -n brrtrouter-dev -it <prometheus-pod> -- tar -xzf prometheus-snapshot.tar.gz
```

### Option 3: Start fresh (complete data loss)
```bash
kubectl delete pvc prometheus-storage -n brrtrouter-dev
kubectl delete pod -l app=prometheus -n brrtrouter-dev
```

## Verification

Prometheus is healthy when:
```bash
curl http://prometheus:9090/-/healthy
# Returns: "Prometheus Server is Healthy."

curl http://prometheus:9090/api/v1/query?query=up
# Returns valid metrics
```
