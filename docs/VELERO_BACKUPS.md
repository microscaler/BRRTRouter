# Velero Backups with MinIO

## Overview

Professional-grade Kubernetes backup solution using Velero with MinIO storage.

**Key Features:**
- ✅ Automated daily backups
- ✅ Off-cluster storage (survives KIND deletion)
- ✅ Point-in-time recovery
- ✅ Backup observability data, configs, and secrets
- ✅ Production-ready backup strategy

## Architecture

```
KIND Cluster (brrtrouter-dev namespace)
    ↓
Velero (backup controller)
    ↓
MinIO (S3-compatible storage)
    ↓
Docker Volume (brrtrouter-minio-data)
    ↓
Host Filesystem (persistent)
```

**MinIO runs OUTSIDE the KIND cluster**, ensuring backups survive even if you delete the entire cluster.

## Quick Start

### Setup (One Time)

```bash
# Install Velero and start MinIO
chmod +x scripts/setup-velero.sh
./scripts/setup-velero.sh
```

This will:
1. Start MinIO in Docker (outside KIND)
2. Install Velero CLI (if needed)
3. Deploy Velero into KIND cluster
4. Configure S3 backup location (MinIO)
5. Create daily backup schedule
6. Take initial backup

### Verify Installation

```bash
# Check Velero is running
kubectl get pods -n velero

# List backups
velero backup get

# Check MinIO console
open http://localhost:9001
# Login: minioadmin / minioadmin123
```

## Usage

### Manual Backups

```bash
# Create backup now
velero backup create brrtrouter-manual \
    --include-namespaces brrtrouter-dev \
    --wait

# Backup specific resources
velero backup create grafana-dashboards \
    --include-namespaces brrtrouter-dev \
    --include-resources persistentvolumeclaims,persistentvolumes \
    --selector app=grafana

# Backup with description
velero backup create before-upgrade \
    --include-namespaces brrtrouter-dev \
    --labels purpose=upgrade \
    --wait
```

### List Backups

```bash
# All backups
velero backup get

# Detailed info
velero backup describe brrtrouter-initial

# Show logs
velero backup logs brrtrouter-initial

# Export backup details
velero backup describe brrtrouter-initial --details > backup-report.txt
```

### Restore from Backup

```bash
# List available backups
velero backup get

# Restore entire namespace
velero restore create --from-backup brrtrouter-daily-20231210 --wait

# Restore specific resources
velero restore create grafana-restore \
    --from-backup brrtrouter-daily-20231210 \
    --include-resources persistentvolumeclaims \
    --selector app=grafana

# Restore to different namespace
velero restore create test-restore \
    --from-backup brrtrouter-daily-20231210 \
    --namespace-mappings brrtrouter-dev:brrtrouter-test
```

### Delete Backups

```bash
# Delete specific backup
velero backup delete brrtrouter-manual

# Delete old backups (older than 30 days)
velero backup get | awk '/brrtrouter-daily/ && $5 ~ /30d|[0-9][0-9]d/ {print $1}' | xargs -I {} velero backup delete {}

# Delete all backups (DANGEROUS!)
velero backup delete --all --confirm
```

## Automated Schedules

### Daily Backup (Pre-configured)

```yaml
# .velero/backup-schedule.yaml
apiVersion: velero.io/v1
kind: Schedule
metadata:
  name: brrtrouter-daily
spec:
  schedule: "0 2 * * *"  # 2 AM daily
  template:
    includedNamespaces:
    - brrtrouter-dev
    ttl: 720h0m0s  # Keep for 30 days
```

### Additional Schedules

```bash
# Hourly backups (keep for 24 hours)
velero schedule create brrtrouter-hourly \
    --schedule="0 * * * *" \
    --include-namespaces brrtrouter-dev \
    --ttl 24h

# Weekly backups (keep for 90 days)
velero schedule create brrtrouter-weekly \
    --schedule="0 3 * * 0" \
    --include-namespaces brrtrouter-dev \
    --ttl 2160h

# Before each Tilt deployment
# (Manual - trigger before major changes)
velero backup create pre-deploy-$(date +%Y%m%d-%H%M%S) \
    --include-namespaces brrtrouter-dev \
    --labels purpose=pre-deploy
```

### Manage Schedules

```bash
# List schedules
velero schedule get

# Describe schedule
velero schedule describe brrtrouter-daily

# Pause schedule
velero schedule pause brrtrouter-daily

# Resume schedule
velero schedule unpause brrtrouter-daily

# Delete schedule
velero schedule delete brrtrouter-hourly
```

## Disaster Recovery Scenarios

### Scenario 1: Accidentally Deleted Grafana Dashboard

```bash
# 1. Check current state
kubectl get pods -n brrtrouter-dev -l app=grafana

# 2. List recent backups
velero backup get | grep $(date +%Y%m%d)

# 3. Restore Grafana data only
velero restore create grafana-recovery \
    --from-backup brrtrouter-daily-20231210 \
    --include-resources persistentvolumeclaims \
    --selector app=grafana \
    --wait

# 4. Restart Grafana pod
kubectl rollout restart deployment/grafana -n brrtrouter-dev

# 5. Verify
open http://localhost:3000
```

### Scenario 2: Entire Namespace Deleted

```bash
# 1. Recreate namespace
kubectl create namespace brrtrouter-dev

# 2. Restore everything
velero restore create full-recovery \
    --from-backup brrtrouter-daily-20231210 \
    --wait

# 3. Verify all pods are running
kubectl get pods -n brrtrouter-dev

# Done! Everything restored.
```

### Scenario 3: KIND Cluster Completely Deleted

```bash
# 1. MinIO data is safe (runs outside KIND)
docker ps | grep minio
# Should show: brrtrouter-minio

# 2. Recreate KIND cluster
just dev-down
just dev-up

# 3. Reinstall Velero
./scripts/setup-velero.sh

# 4. List available backups (still there!)
velero backup get

# 5. Restore from latest backup
LATEST=$(velero backup get --output name | head -1)
velero restore create cluster-recovery --from-backup $LATEST --wait

# 6. Verify everything is back
kubectl get all -n brrtrouter-dev
open http://localhost:3000  # Grafana dashboards restored!
```

### Scenario 4: Corrupted Prometheus Data

```bash
# 1. Delete bad data
kubectl delete pvc prometheus-storage -n brrtrouter-dev

# 2. Recreate PVC
kubectl apply -f k8s/observability-storage.yaml

# 3. Restore Prometheus data
velero restore create prometheus-fix \
    --from-backup brrtrouter-daily-20231210 \
    --include-resources persistentvolumeclaims \
    --selector app=prometheus

# 4. Restart Prometheus
kubectl rollout restart deployment/prometheus -n brrtrouter-dev
```

## MinIO Management

### Access MinIO Console

```bash
# Open browser
open http://localhost:9001

# Login credentials
# User: minioadmin
# Pass: minioadmin123
```

### Browse Backups

In MinIO Console:
1. Navigate to "Buckets" → "velero"
2. Browse folders: `backups/`, `metadata/`, `restores/`
3. Download individual backup files if needed

### MinIO CLI

```bash
# Install mc (MinIO Client)
brew install minio/stable/mc  # macOS
# or
wget https://dl.min.io/client/mc/release/linux-amd64/mc && chmod +x mc && sudo mv mc /usr/local/bin/

# Configure alias
mc alias set brrtrouter http://localhost:9000 minioadmin minioadmin123

# List buckets
mc ls brrtrouter

# List backups
mc ls brrtrouter/velero/backups/

# Download backup
mc cp brrtrouter/velero/backups/brrtrouter-daily-20231210.tar.gz ./

# Upload backup from another system
mc cp backup.tar.gz brrtrouter/velero/backups/
```

### MinIO Storage Management

```bash
# Check storage usage
docker exec brrtrouter-minio du -sh /data

# Backup MinIO data (to external storage)
docker run --rm \
    -v brrtrouter-minio-data:/source:ro \
    -v ~/minio-backups:/backup \
    alpine tar czf /backup/minio-$(date +%Y%m%d).tar.gz -C /source .

# Restore MinIO data
docker run --rm \
    -v brrtrouter-minio-data:/target \
    -v ~/minio-backups:/backup:ro \
    alpine tar xzf /backup/minio-20231210.tar.gz -C /target
```

## Monitoring & Alerting

### Check Backup Status

```bash
# Recent backups status
velero backup get | head -10

# Failed backups
velero backup get | grep -i "PartiallyFailed\|Failed"

# Check Velero pod logs
kubectl logs -n velero -l name=velero --tail=100

# Backup metrics
kubectl get backups.velero.io -n velero -o json | \
    jq '.items[] | {name: .metadata.name, phase: .status.phase, errors: .status.errors}'
```

### Backup Validation

```bash
# Verify backup contents
velero backup describe brrtrouter-daily-20231210 --details

# Check what's included
velero backup describe brrtrouter-daily-20231210 --details | grep "Resource List"

# Verify in MinIO
mc ls brrtrouter/velero/backups/brrtrouter-daily-20231210/
```

### Automated Monitoring Script

```bash
#!/bin/bash
# scripts/check-backup-health.sh

LATEST_BACKUP=$(velero backup get --output name | head -1)
BACKUP_STATUS=$(velero backup get $LATEST_BACKUP -o json | jq -r '.status.phase')
BACKUP_AGE=$(velero backup get $LATEST_BACKUP -o json | jq -r '.status.completionTimestamp')

echo "Latest backup: $LATEST_BACKUP"
echo "Status: $BACKUP_STATUS"
echo "Completed: $BACKUP_AGE"

if [ "$BACKUP_STATUS" != "Completed" ]; then
    echo "❌ Latest backup failed!"
    velero backup logs $LATEST_BACKUP
    exit 1
fi

echo "✅ Backups healthy"
```

## Integration with Justfile

Add to your `justfile`:

```makefile
# Backup commands
backup-now:
    @velero backup create brrtrouter-manual-$(date +%Y%m%d-%H%M%S) \
        --include-namespaces brrtrouter-dev \
        --wait

backup-list:
    @velero backup get

backup-restore name:
    @velero restore create restore-$(date +%Y%m%d-%H%M%S) \
        --from-backup {{name}} \
        --wait

# Backup before major operations
backup-before-upgrade:
    @velero backup create pre-upgrade-$(date +%Y%m%d-%H%M%S) \
        --include-namespaces brrtrouter-dev \
        --labels purpose=upgrade \
        --wait
```

## Troubleshooting

### Velero Pod Not Starting

```bash
# Check events
kubectl describe pod -n velero -l name=velero

# Check logs
kubectl logs -n velero -l name=velero

# Common issue: MinIO not accessible
kubectl exec -n velero -l name=velero -- curl -v http://host.docker.internal:9000
```

**Fix:**
```bash
# Restart MinIO
docker-compose -f k8s/velero/docker-compose-minio.yml restart

# Delete and reinstall Velero
velero uninstall --force
./scripts/setup-velero.sh
```

### Backup Stuck in "InProgress"

```bash
# Check backup status
velero backup describe brrtrouter-stuck

# Check Velero logs
kubectl logs -n velero -l name=velero --tail=50

# Delete stuck backup
velero backup delete brrtrouter-stuck --confirm
```

### Restore Fails with "AlreadyExists"

```bash
# Option 1: Delete existing resources first
kubectl delete namespace brrtrouter-dev
velero restore create --from-backup <name>

# Option 2: Restore to different namespace
velero restore create test-restore \
    --from-backup <name> \
    --namespace-mappings brrtrouter-dev:brrtrouter-test
```

### MinIO Connection Refused

```bash
# Check MinIO is running
docker ps | grep minio

# Check MinIO logs
docker logs brrtrouter-minio

# Restart MinIO
docker-compose -f k8s/velero/docker-compose-minio.yml restart
```

### Backup Size Too Large

```bash
# Check backup sizes
velero backup describe <name> | grep "Total bytes"

# Exclude large resources
velero backup create smaller-backup \
    --include-namespaces brrtrouter-dev \
    --exclude-resources pods,replicasets
```

## Best Practices

### 1. Regular Testing

```bash
# Monthly restore test
velero restore create test-$(date +%Y%m) \
    --from-backup brrtrouter-daily-latest \
    --namespace-mappings brrtrouter-dev:brrtrouter-test

# Verify
kubectl get all -n brrtrouter-test

# Cleanup
kubectl delete namespace brrtrouter-test
```

### 2. Retention Policy

- **Hourly**: Keep 24 hours
- **Daily**: Keep 30 days
- **Weekly**: Keep 90 days
- **Pre-upgrade**: Keep until upgrade confirmed

### 3. Backup Before Major Changes

```bash
# Before Tilt deployments
just backup-before-upgrade
tilt up

# Before config changes
velero backup create before-config-change --include-namespaces brrtrouter-dev
kubectl apply -f k8s/
```

### 4. Monitor Backup Health

```bash
# Add to crontab
0 9 * * * /path/to/scripts/check-backup-health.sh | mail -s "Backup Status" your@email.com
```

### 5. Off-site Backups

```bash
# Sync MinIO data to S3/GCS
# (Run weekly)
mc mirror brrtrouter/velero s3/my-offsite-bucket/brrtrouter-backups/
```

## Files Created

1. **`k8s/velero/docker-compose-minio.yml`** - MinIO service definition
2. **`k8s/velero/*.yaml`** - Velero Kubernetes manifests (namespace, credentials, deployment, backups)
3. **`k8s/velero/crds.yaml`** - Velero Custom Resource Definitions (downloaded on-demand)
4. **`.velero/backup-schedule.yaml`** - Daily backup schedule
5. **`docs/VELERO_BACKUPS.md`** - This documentation

## Summary

✅ **Professional backup solution** - Velero is production-ready  
✅ **Off-cluster storage** - MinIO survives KIND deletion  
✅ **Automated** - Daily backups, 30-day retention  
✅ **Point-in-time recovery** - Restore to any backup  
✅ **Tested** - Disaster recovery scenarios documented  

**Setup:**
```bash
./scripts/setup-velero.sh
```

**Daily use:**
```bash
velero backup get              # List backups
velero backup create <name>    # Manual backup
velero restore create --from-backup <name>  # Restore
```

**MinIO Console:**
http://localhost:9001 (minioadmin / minioadmin123)

**Your observability data is now professionally backed up!** 🎉

