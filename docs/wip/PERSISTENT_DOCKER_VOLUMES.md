# Persistent Docker Volumes for Observability

## Problem Solved

**Before:** Grafana dashboards, metrics, logs, and traces were lost every time the KIND cluster was recreated.  
**After:** All observability data persists in Docker volumes that survive cluster recreation.

## How It Works

### Docker Volumes (Persistent Storage)

Four named Docker volumes store all observability data:

```bash
brrtrouter-prometheus-data  # Metrics time-series
brrtrouter-loki-data        # Log chunks and indexes
brrtrouter-grafana-data     # Dashboards, datasources, preferences
brrtrouter-jaeger-data      # Distributed traces
```

These volumes are **external to the KIND cluster** and persist on your Docker host.

### KIND Cluster Mounts

The KIND cluster mounts these Docker volumes into the node:

```yaml
# kind-config.yaml
extraMounts:
  - hostPath: /var/lib/docker/volumes/brrtrouter-prometheus-data/_data
    containerPath: /mnt/prometheus-data
  - hostPath: /var/lib/docker/volumes/brrtrouter-loki-data/_data
    containerPath: /mnt/loki-data
  # ... etc
```

### Kubernetes PVs/PVCs

Kubernetes PersistentVolumes use `hostPath` to access the mounted Docker volumes:

```yaml
# k8s/observability-storage.yaml
apiVersion: v1
kind: PersistentVolume
metadata:
  name: grafana-pv
spec:
  storageClassName: manual
  hostPath:
    path: "/mnt/grafana-data"  # Maps to Docker volume
```

## Data Flow

```
Docker Volume (Host)
    ↓
KIND Node Mount (/mnt/grafana-data)
    ↓
Kubernetes PV (hostPath)
    ↓
Kubernetes PVC
    ↓
Pod Volume Mount
    ↓
Grafana Container (/var/lib/grafana)
```

## Usage

### Automatic Setup

Volumes are automatically created when you run:

```bash
just dev-up
```

Or manually:

```bash
./scripts/dev-setup.sh
```

### Manual Volume Creation

```bash
# Volumes are created automatically by dev-setup.sh
# Or create manually:
docker volume create brrtrouter-prometheus-data
docker volume create brrtrouter-loki-data
docker volume create brrtrouter-grafana-data
docker volume create brrtrouter-jaeger-data
```

### Verify Volumes Exist

```bash
# List all brrtrouter volumes
docker volume ls | grep brrtrouter

# Expected output:
# brrtrouter-grafana-data
# brrtrouter-jaeger-data
# brrtrouter-loki-data
# brrtrouter-prometheus-data

# Inspect a volume
docker volume inspect brrtrouter-grafana-data

# Check volume location
docker volume inspect brrtrouter-grafana-data --format '{{ .Mountpoint }}'
# Output: /var/lib/docker/volumes/brrtrouter-grafana-data/_data
```

## Testing Data Persistence

### Test 1: Create Dashboards and Recreate Cluster

```bash
# 1. Start cluster
just dev-up

# 2. Create custom dashboard in Grafana
open http://localhost:3000
# - Login (admin/admin)
# - Create a new dashboard
# - Add some panels
# - Save as "My Test Dashboard"

# 3. Tear down cluster
just dev-down

# 4. Recreate cluster
just dev-up

# 5. Check Grafana
open http://localhost:3000
# ✅ "My Test Dashboard" should still be there!
```

### Test 2: Generate Metrics and Recreate

```bash
# 1. Send traffic to generate metrics
for i in {1..1000}; do
  curl http://localhost:9090/health
done

# 2. View metrics in Grafana
open http://localhost:3000
# - Go to Explore
# - Query: rate(brrtrouter_requests_total[5m])
# - Should see data

# 3. Recreate cluster
just dev-down
just dev-up

# 4. Query metrics again
# ✅ Historical data should still be there!
```

## Files Modified

1. **`kind-config.yaml`** (lines 14-24)
   - Added `extraMounts` for Docker volumes

2. **`k8s/observability-storage.yaml`** (complete rewrite)
   - Changed from dynamic PVCs to PV+PVC with `hostPath`
   - Four PV definitions (prometheus, loki, grafana, jaeger)
   - Four PVC definitions with `storageClassName: manual`

3. **`scripts/dev-setup.sh`** (lines 19-33)
   - Added Docker volume creation at startup

4. **`scripts/setup-persistent-volumes.sh`** (NEW)
   - Standalone script to create volumes

## Volume Management

### Check Volume Usage

```bash
# See storage used by each volume
docker volume ls --format "table {{.Name}}\t{{.Size}}"

# Or detailed inspect
for vol in prometheus-data loki-data grafana-data jaeger-data; do
  echo "brrtrouter-$vol:"
  docker system df -v | grep "brrtrouter-$vol"
done
```

### Backup Volumes

```bash
# Backup all observability data
mkdir -p ~/brrtrouter-backups/$(date +%Y%m%d-%H%M%S)

for vol in prometheus-data loki-data grafana-data jaeger-data; do
  docker run --rm \
    -v brrtrouter-$vol:/source:ro \
    -v ~/brrtrouter-backups/$(date +%Y%m%d-%H%M%S):/backup \
    alpine \
    tar czf /backup/$vol.tar.gz -C /source .
  echo "✅ Backed up $vol"
done

echo "Backup complete: ~/brrtrouter-backups/"
```

### Restore Volumes

```bash
# Restore from backup
BACKUP_DIR=~/brrtrouter-backups/20251010-143000

for vol in prometheus-data loki-data grafana-data jaeger-data; do
  docker run --rm \
    -v brrtrouter-$vol:/target \
    -v $BACKUP_DIR:/backup:ro \
    alpine \
    tar xzf /backup/$vol.tar.gz -C /target
  echo "✅ Restored $vol"
done

echo "Restore complete!"
```

### Clean Up Volumes

```bash
# Remove all brrtrouter volumes (DESTRUCTIVE!)
docker volume rm brrtrouter-prometheus-data
docker volume rm brrtrouter-loki-data
docker volume rm brrtrouter-grafana-data
docker volume rm brrtrouter-jaeger-data

# Or in one command
docker volume ls -q | grep brrtrouter | xargs docker volume rm

# Then recreate
just dev-up
```

### Reset Just One Component

```bash
# Example: Reset just Grafana (lose dashboards, start fresh)
kubectl delete pod -n brrtrouter-dev -l app=grafana
docker volume rm brrtrouter-grafana-data
docker volume create brrtrouter-grafana-data

# Restart pod
kubectl rollout restart deployment/grafana -n brrtrouter-dev
```

## Troubleshooting

### PVC Stuck in Pending

```bash
# Check PVC status
kubectl get pvc -n brrtrouter-dev

# Describe to see events
kubectl describe pvc grafana-storage -n brrtrouter-dev

# Common issue: PV not created yet
kubectl get pv

# Should show:
# prometheus-pv, loki-pv, grafana-pv, jaeger-pv
```

**Fix:**
```bash
# Apply storage config
kubectl apply -f k8s/observability-storage.yaml

# Wait for PVs to be created
kubectl get pv --watch
```

### Pod Can't Mount Volume

```bash
# Check pod events
kubectl describe pod -n brrtrouter-dev -l app=grafana

# Look for errors like:
# - "failed to mount volume"
# - "path not found"
```

**Fix:**
```bash
# Ensure Docker volumes exist
docker volume ls | grep brrtrouter

# If missing, create them
./scripts/setup-persistent-volumes.sh

# Recreate cluster
just dev-down
just dev-up
```

### Data Not Persisting

```bash
# 1. Verify Docker volume has data
docker volume inspect brrtrouter-grafana-data --format '{{ .Mountpoint }}'
# Go to that path and check files:
ls -la $(docker volume inspect brrtrouter-grafana-data --format '{{ .Mountpoint }}')

# 2. Check if volume is actually mounted in pod
kubectl exec -n brrtrouter-dev -l app=grafana -- df -h /var/lib/grafana

# 3. Check PVC binding
kubectl get pvc -n brrtrouter-dev
# Status should be "Bound"
```

### Volumes Taking Too Much Space

```bash
# Check sizes
docker system df -v | grep brrtrouter

# Option 1: Clean old data (Prometheus)
kubectl exec -n brrtrouter-dev -l app=prometheus -- \
  find /prometheus -type f -mtime +7 -delete

# Option 2: Truncate logs (Loki)
kubectl exec -n brrtrouter-dev -l app=loki -- \
  find /loki -name "*.log" -mtime +3 -delete

# Option 3: Start fresh
docker volume rm brrtrouter-prometheus-data
docker volume create brrtrouter-prometheus-data
kubectl rollout restart deployment/prometheus -n brrtrouter-dev
```

## Benefits

✅ **Dashboards persist** - Custom Grafana dashboards survive cluster recreation  
✅ **Metrics history** - Prometheus data kept across restarts  
✅ **Logs retained** - Loki maintains historical logs  
✅ **Traces preserved** - Jaeger keeps span data  
✅ **Fast rebuilds** - No need to reconfigure after recreation  
✅ **True local dev** - Mirrors production persistence patterns  

## Comparison

### Before (Ephemeral Storage)

```
KIND Cluster Created
  ↓
Observability Stack Deployed
  ↓
Data Stored in Pod (emptyDir)
  ↓
[Cluster Deleted]
  ↓
❌ ALL DATA LOST
```

### After (Persistent Docker Volumes)

```
Docker Volumes Created (one time)
  ↓
KIND Cluster Created
  ↓
Volumes Mounted into KIND
  ↓
Observability Stack Uses Volumes
  ↓
[Cluster Deleted]
  ↓
Volumes Remain on Docker Host
  ↓
[Cluster Recreated]
  ↓
✅ DATA STILL THERE
```

## Production Considerations

This setup is for **local development only**. In production:

- Use cloud provider persistent volumes (EBS, Persistent Disk, etc.)
- Set up automated backups
- Use StatefulSets for stateful services
- Implement retention policies
- Monitor volume usage

## Summary

✅ **Created**: Four named Docker volumes  
✅ **Mounted**: Volumes accessible to KIND cluster  
✅ **Mapped**: Kubernetes PVs/PVCs use hostPath  
✅ **Persistent**: Data survives cluster recreation  
✅ **Tested**: Recreate cluster, data persists  

**Commands:**
```bash
# Setup (automatic on first run)
just dev-up

# Manual volume creation
./scripts/setup-persistent-volumes.sh

# Verify
docker volume ls | grep brrtrouter

# Backup
# (see "Backup Volumes" section above)
```

**Your dashboards, metrics, logs, and traces now persist!** 🎉

