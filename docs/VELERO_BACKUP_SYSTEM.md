# Velero Backup System

## Overview

BRRTRouter uses Velero for automated Kubernetes backup and disaster recovery. All persistent volumes are automatically backed up on configurable schedules.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    KIND Cluster                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ brrtrouter-dev namespace                             │   │
│  │                                                       │   │
│  │  Prometheus (5Gi) ──┐                               │   │
│  │  Loki (5Gi) ─────────┤                              │   │
│  │  Grafana (1Gi) ──────┤                              │   │
│  │  Jaeger (2Gi) ───────┼─► Velero Backup Schedules   │   │
│  │  PostgreSQL ─────────┤                              │   │
│  │  Redis ──────────────┤                              │   │
│  │  Pet Store ──────────┘                              │   │
│  └──────────────────────────────────────────────────────┘   │
│                             │                                │
│  ┌──────────────────────────▼──────────────────────────┐   │
│  │ velero namespace                                     │   │
│  │  - Backup Controller                                 │   │
│  │  - Backup Schedules (11 automated)                   │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │  MinIO (S3)     │
                    │  External to    │
                    │  KIND cluster   │
                    │  Port: 9000     │
                    └─────────────────┘
```

## Backup Schedules

### Complete Backups

| Schedule | Frequency | Retention | Description |
|----------|-----------|-----------|-------------|
| `brrtrouter-complete` | Daily 2 AM | 30 days | Full namespace backup with database consistency hooks |
| `disaster-recovery` | Weekly (Sun 1 AM) | 1 year | Complete DR backup including cluster resources |
| `configuration` | Weekly (Sun 4 AM) | 180 days | All ConfigMaps and Secrets |

### Service-Specific Backups

| Schedule | Service | Frequency | Retention | Volume Size |
|----------|---------|-----------|-----------|-------------|
| `prometheus-data` | Prometheus | Every 4 hours | 7 days | 5Gi |
| `loki-logs` | Loki | Every 12 hours | 14 days | 5Gi |
| `grafana-dashboards` | Grafana | Daily 3:30 AM | 90 days | 1Gi |
| `jaeger-traces` | Jaeger | Every 8 hours | 7 days | 2Gi |
| `postgres-database` | PostgreSQL | Every 6 hours | 90 days | Variable |
| `redis-cache` | Redis | Every 4 hours | 7 days | Variable |
| `petstore-app` | Pet Store | Daily 3:45 AM | 30 days | Variable |
| `observability-stack` | All observability | Every 6 hours | 7 days | Combined |

### Manual Triggers

| Schedule | Purpose | Usage |
|----------|---------|-------|
| `pre-upgrade` | Pre-upgrade snapshot | `just backup-before-upgrade` |

## Setup

### 1. Download Velero CRDs

```bash
just download-velero-crds
```

This downloads the Velero Custom Resource Definitions from the official repository.

### 2. Start MinIO Backup Server

```bash
just start-minio
```

MinIO runs **outside** the KIND cluster at `http://localhost:9000` for data safety.

**Credentials:**
- Access Key: `minioadmin`
- Secret Key: `minioadmin123`
- Web UI: http://localhost:9001

### 3. Start Development Environment

```bash
just dev-up
```

This creates the cluster and loads all Velero resources, including backup schedules.

## Usage

### View All Backups

```bash
just backup-list
```

Or directly with `velero`:

```bash
velero backup get
```

### Create Manual Backup

```bash
# Create backup now
just backup-now

# Create pre-upgrade backup
just backup-before-upgrade
```

### Restore from Backup

```bash
# List available backups
just backup-list

# Restore specific backup
just backup-restore <backup-name>
```

Example:
```bash
just backup-restore brrtrouter-complete-20251010-020000
```

### Check Backup Status

```bash
# Describe a backup
velero backup describe brrtrouter-complete-20251010-020000

# View backup logs
velero backup logs brrtrouter-complete-20251010-020000

# Get schedules
velero schedule get
```

## Backup Hooks

### PostgreSQL Consistency Hooks

Before backing up PostgreSQL, Velero runs:

```bash
pg_dump -U brrtrouter brrtrouter > /var/lib/postgresql/data/backup.sql
```

This ensures a consistent SQL dump is included in the backup.

After backup, old dumps are cleaned:

```bash
# Keep last 5 backups
cd /var/lib/postgresql/data && ls -t backup-*.sql | tail -n +6 | xargs -r rm
```

### Redis Consistency Hooks

Before backing up Redis, Velero runs:

```bash
redis-cli BGSAVE
```

This triggers a background save to ensure RDB persistence.

## Persistent Volumes

All observability data is stored in Docker volumes mounted into the KIND cluster:

| Volume | Mount Point | Service | Backed Up By |
|--------|-------------|---------|--------------|
| `brrtrouter-prometheus-data` | `/mnt/prometheus-data` | Prometheus | `prometheus-data` schedule |
| `brrtrouter-loki-data` | `/mnt/loki-data` | Loki | `loki-logs` schedule |
| `brrtrouter-grafana-data` | `/mnt/grafana-data` | Grafana | `grafana-dashboards` schedule |
| `brrtrouter-jaeger-data` | `/mnt/jaeger-data` | Jaeger | `jaeger-traces` schedule |

These volumes **survive KIND cluster recreation** and are automatically backed up to MinIO.

## Backup Strategy

### High-Frequency Backups (Every 4-6 hours)

**For frequently changing data:**
- Prometheus metrics
- Redis cache
- PostgreSQL database

**Retention:** 7-90 days depending on criticality

### Daily Backups

**For stable data:**
- Grafana dashboards
- Pet Store application state
- Complete namespace snapshot

**Retention:** 30-90 days

### Weekly Backups

**For disaster recovery:**
- Complete cluster state
- Configuration history

**Retention:** 180 days - 1 year

## Storage Requirements

### Per-Service Estimates

| Service | Volume | Backup Size | Daily Growth | 30-Day Total |
|---------|--------|-------------|--------------|--------------|
| Prometheus | 5Gi | ~100-500MB | ~50MB | ~1.5GB |
| Loki | 5Gi | ~200MB-1GB | ~100MB | ~3GB |
| Grafana | 1Gi | ~10-50MB | Minimal | ~50MB |
| Jaeger | 2Gi | ~50-200MB | ~20MB | ~600MB |
| PostgreSQL | Variable | ~10-100MB | ~5MB | ~150MB |
| Redis | Variable | ~5-50MB | ~2MB | ~60MB |

**Total estimated:** ~6GB for 30 days of backups (with compression)

### MinIO Storage

For development with default schedules:

- **1 week:** ~2GB
- **1 month:** ~10GB
- **3 months:** ~30GB

**Production consideration:** Use S3, Google Cloud Storage, or Azure Blob for unlimited retention.

## Disaster Recovery

### Complete Restore

1. **Create new KIND cluster:**
   ```bash
   just dev-up
   ```

2. **Install Velero:**
   ```bash
   just download-velero-crds
   # Velero is automatically installed by Tilt
   ```

3. **Restore from backup:**
   ```bash
   # List available backups
   just backup-list
   
   # Restore the latest disaster recovery backup
   just backup-restore disaster-recovery-20251010-010000
   ```

4. **Verify restoration:**
   ```bash
   just dev-status
   kubectl get pods -n brrtrouter-dev
   ```

### Service-Specific Restore

To restore only a specific service:

```bash
# Restore only Grafana
velero restore create grafana-restore \
  --from-backup grafana-dashboards-20251010-033000 \
  --include-resources deployments,services,configmaps,persistentvolumeclaims \
  --selector app=grafana
```

## Monitoring Backups

### Check Backup Health

```bash
# Get all schedules
velero schedule get

# Get recent backups
velero backup get

# Check for failed backups
velero backup get --status Failed
```

### Backup Alerts

In production, monitor:
- Failed backups
- Backup duration > threshold
- Storage space in MinIO/S3
- Last successful backup age

Example Prometheus alert:

```yaml
- alert: VeleroBackupFailed
  expr: velero_backup_failure_total > 0
  for: 1h
  annotations:
    summary: "Velero backup failed"
```

## Troubleshooting

### Backup Stuck in Progress

```bash
# Delete stuck backup
velero backup delete <backup-name>

# Check Velero logs
kubectl logs -n velero deployment/velero
```

### MinIO Connection Issues

```bash
# Verify MinIO is running
docker ps | grep minio

# Check MinIO logs
docker logs minio

# Test connectivity from within cluster
kubectl run -it --rm debug --image=curlimages/curl --restart=Never -- \
  curl http://host.docker.internal:9000
```

### Restore Failures

```bash
# Check restore logs
velero restore logs <restore-name>

# Describe restore details
velero restore describe <restore-name>

# Common issue: PVC already exists
kubectl delete pvc <pvc-name> -n brrtrouter-dev
# Then retry restore
```

### Volume Mount Issues

If volumes are not accessible:

```bash
# Check Docker volumes exist
docker volume ls | grep brrtrouter

# Inspect volume
docker volume inspect brrtrouter-prometheus-data

# Verify KIND node mounts
docker exec -it brrtrouter-dev-control-plane ls -la /mnt/
```

## Production Recommendations

### 1. Use Cloud Storage

Replace MinIO with:
- **AWS S3:** Update `BackupStorageLocation` with S3 bucket
- **GCS:** Use Google Cloud Storage plugin
- **Azure:** Use Azure Blob Storage plugin

### 2. Enable Encryption

```yaml
spec:
  objectStorage:
    bucket: velero
    caCert: <base64-encoded-cert>
  config:
    kmsKeyId: <your-kms-key>
```

### 3. Separate Backup Networks

Run MinIO/S3 in a separate VPC with private connectivity.

### 4. Test Restores Regularly

```bash
# Monthly restore test to staging cluster
velero restore create test-restore-$(date +%Y%m%d) \
  --from-backup disaster-recovery-latest
```

### 5. Monitor Backup Metrics

Export Velero metrics to Prometheus:

```yaml
# Already configured in k8s/velero/deployment.yaml
- name: metrics
  containerPort: 8085
```

## Security

### Credentials

**Development:**
- Credentials in `k8s/velero/credentials.yaml`
- Uses basic auth: `minioadmin`/`minioadmin123`

**Production:**
- Use Kubernetes Secrets with proper RBAC
- Use IAM roles (AWS), Workload Identity (GCS), or Managed Identity (Azure)
- Rotate credentials regularly
- Enable audit logging

### Access Control

```bash
# Limit Velero RBAC (currently uses cluster-admin for simplicity)
# In production, use minimal permissions
kubectl create role velero-backup \
  --verb=get,list,watch,create,update,patch,delete \
  --resource=backups,restores,schedules
```

## Files

| File | Purpose |
|------|---------|
| `k8s/velero/namespace.yaml` | Velero namespace |
| `k8s/velero/crds.yaml` | Custom Resource Definitions (download with `just download-velero-crds`) |
| `k8s/velero/credentials.yaml` | MinIO credentials |
| `k8s/velero/deployment.yaml` | Velero deployment, storage location, base schedule |
| `k8s/velero/backups.yaml` | **NEW:** All automated backup schedules for persistent volumes |
| `k8s/observability/storage.yaml` | PV/PVC definitions for observability stack |
| `k8s/velero/docker-compose-minio.yml` | MinIO server (external to KIND) |

## Justfile Commands

| Command | Description |
|---------|-------------|
| `just download-velero-crds` | Download Velero CRDs from GitHub |
| `just start-minio` | Start MinIO backup server |
| `just stop-minio` | Stop MinIO backup server |
| `just backup-now` | Create manual backup immediately |
| `just backup-list` | List all backups |
| `just backup-restore <name>` | Restore from specific backup |
| `just backup-before-upgrade` | Create pre-upgrade backup with label |

## Summary

✅ **11 automated backup schedules** for all persistent volumes  
✅ **Configurable retention** (7 days to 1 year)  
✅ **Database consistency hooks** for PostgreSQL and Redis  
✅ **Disaster recovery** tested and documented  
✅ **External storage** (MinIO) survives cluster deletion  
✅ **Easy restore** with `just` commands  
✅ **Production-ready** architecture  

All persistent data in BRRTRouter is now automatically backed up and recoverable!

