# Velero Backup System - Declarative Setup

## Overview

Velero is deployed declaratively via Kubernetes manifests and managed by Tilt, not scripts.

**Architecture:**
- MinIO runs in Docker Compose (outside KIND cluster)
- Velero deployed via K8s manifests in KIND
- Tilt manages Velero lifecycle
- Automated daily backups via CronJob

## Setup (One Time)

### Step 1: Download Velero CRDs

```bash
# Download Velero Custom Resource Definitions (one-time)
just download-velero-crds
```

This downloads `k8s/velero/crds.yaml` which defines:
- `Backup` CRD
- `Restore` CRD  
- `Schedule` CRD
- `BackupStorageLocation` CRD
- `VolumeSnapshotLocation` CRD

**Commit this file to git** so team members don't need to download it.

### Step 2: Start MinIO

```bash
# Start MinIO backup server (runs outside KIND)
just start-minio
```

MinIO will:
- Run on `localhost:9000` (API) and `localhost:9001` (Console)
- Store data in Docker volume `brrtrouter-minio-data`
- Survive KIND cluster recreation
- Auto-create `velero` bucket

### Step 3: Start Tilt

```bash
# Tilt will automatically deploy Velero
just dev-up
```

Tilt deploys:
1. Velero namespace
2. Velero CRDs
3. Velero credentials (MinIO access keys)
4. Velero deployment
5. BackupStorageLocation (points to MinIO)
6. Schedule (daily backups at 2 AM)

## Kubernetes Manifests

All Velero manifests are organized in `k8s/velero/` for easy management.

### 1. namespace.yaml
```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: velero
```

### 2. credentials.yaml
```yaml
apiVersion: v1
kind: Secret
metadata:
  name: cloud-credentials
  namespace: velero
stringData:
  cloud: |
    [default]
    aws_access_key_id = minioadmin
    aws_secret_access_key = minioadmin123
```

### 3. deployment.yaml

Contains:
- **ServiceAccount**: `velero`
- **ClusterRoleBinding**: Grants cluster-admin to Velero
- **ConfigMap**: Backup location config
- **Deployment**: Velero pod with AWS plugin
- **BackupStorageLocation**: MinIO S3 endpoint
- **VolumeSnapshotLocation**: Volume snapshot config
- **Schedule**: Daily backup at 2 AM, 30-day retention

### 4. crds.yaml

Downloaded from Velero GitHub:
- Defines all Velero Custom Resources
- Version-locked to v1.12.3
- ~7000 lines of OpenAPI schemas

### 5. docker-compose-minio.yml

MinIO storage backend runs outside KIND cluster via Docker Compose:
- Located in `k8s/velero/docker-compose-minio.yml`
- Started with `just start-minio`
- Ensures backups survive cluster deletion

## Tilt Integration

### Tiltfile Configuration

```python
# Load namespaces
k8s_yaml([
    'k8s/namespace.yaml',
    'k8s/velero/namespace.yaml',
])

# Load Velero (if CRDs exist)
if os.path.exists('k8s/velero/crds.yaml'):
    k8s_yaml('k8s/velero/crds.yaml')
    k8s_yaml([
        'k8s/velero/credentials.yaml',
        'k8s/velero/deployment.yaml',
        'k8s/velero/backups.yaml',
    ])

# Resource configuration
k8s_resource(
    'velero',
    labels=['backup'],
)
```

**Benefits:**
- ✅ Velero automatically deployed with `tilt up`
- ✅ Changes to manifests trigger reload
- ✅ Velero logs visible in Tilt UI
- ✅ Health status monitored
- ✅ No manual `velero install` needed

## Usage

### Check Velero Status

```bash
# Via Tilt UI
tilt up
# Look for "velero" resource in Tilt web UI

# Via kubectl
kubectl get pods -n velero

# Via Velero CLI
velero version
velero backup-location get
```

### Create Manual Backup

```bash
# Quick backup
just backup-now

# Or use Velero CLI directly
velero backup create my-backup \
    --include-namespaces brrtrouter-dev \
    --wait
```

### List Backups

```bash
just backup-list

# Or
velero backup get
```

### Restore from Backup

```bash
just backup-restore <backup-name>

# Or
velero restore create --from-backup <backup-name> --wait
```

## Automated Backups

### Daily Schedule

Defined in `k8s/velero/deployment.yaml` or `k8s/velero/backups.yaml`:

```yaml
apiVersion: velero.io/v1
kind: Schedule
metadata:
  name: brrtrouter-daily
  namespace: velero
spec:
  schedule: "0 2 * * *"  # 2 AM daily
  template:
    includedNamespaces:
      - brrtrouter-dev
    ttl: 720h0m0s  # 30 days
```

**To modify:**
1. Edit `k8s/velero/deployment.yaml` or `k8s/velero/backups.yaml`
2. Save
3. Tilt auto-applies changes

### Add More Schedules

Create additional schedule manifests in `k8s/velero/`:

```yaml
# k8s/velero/schedule-hourly.yaml
apiVersion: velero.io/v1
kind: Schedule
metadata:
  name: brrtrouter-hourly
  namespace: velero
spec:
  schedule: "0 * * * *"  # Every hour
  template:
    includedNamespaces:
      - brrtrouter-dev
    ttl: 24h0m0s  # Keep 24 hours
```

Add to Tiltfile:
```python
k8s_yaml('k8s/velero/schedule-hourly.yaml')
```

## MinIO Management

### Access Console

```bash
# Open MinIO web UI
open http://localhost:9001

# Login
# User: minioadmin
# Pass: minioadmin123
```

### Browse Backups

In MinIO Console:
1. Click "Buckets" → "velero"
2. Navigate folders:
   - `backups/` - Backup metadata
   - `restic/` - Volume data
   - `metadata/` - Additional metadata

### MinIO Commands

```bash
# Start MinIO
just start-minio

# Stop MinIO
just stop-minio

# Check status
docker ps | grep minio

# View logs
docker logs brrtrouter-minio

# Backup MinIO data itself
docker run --rm \
    -v brrtrouter-minio-data:/data:ro \
    -v ~/backups:/backup \
    alpine tar czf /backup/minio-$(date +%Y%m%d).tar.gz -C /data .
```

## Disaster Recovery

### Full Cluster Recreation

```bash
# 1. Ensure MinIO is running (data persists)
docker ps | grep minio
# If not: just start-minio

# 2. Delete KIND cluster
just dev-down

# 3. Recreate cluster with Tilt
just dev-up
# Velero automatically deployed

# 4. Wait for Velero to be ready
kubectl wait --for=condition=ready pod -n velero -l app=velero

# 5. List available backups
velero backup get
# ✅ All backups still there!

# 6. Restore latest backup
LATEST=$(velero backup get -o name | head -1)
velero restore create disaster-recovery --from-backup $LATEST --wait

# 7. Verify
kubectl get all -n brrtrouter-dev
```

## Troubleshooting

### Velero Pod Not Starting

```bash
# Check pod status
kubectl get pods -n velero

# Check events
kubectl describe pod -n velero -l app=velero

# Check logs
kubectl logs -n velero -l app=velero

# Common issue: CRDs not applied
kubectl get crds | grep velero
# If empty: just download-velero-crds && tilt up
```

### MinIO Connection Failed

```bash
# Verify MinIO is running
docker ps | grep minio

# Check MinIO is accessible from KIND
kubectl run -it --rm debug --image=alpine --restart=Never -- \
  wget -qO- http://host.docker.internal:9000

# If fails: restart MinIO
just stop-minio
just start-minio
```

### Backups Not Creating

```bash
# Check BackupStorageLocation
kubectl get backupstoragelocation -n velero

# Should show:
# NAME      PHASE       LAST VALIDATED   AGE
# default   Available   10s              5m

# If "Unavailable", check logs
kubectl logs -n velero -l app=velero | grep -i error

# Test connectivity
velero backup-location get
```

### Schedule Not Running

```bash
# Check schedule exists
kubectl get schedule -n velero

# Describe schedule
kubectl describe schedule brrtrouter-daily -n velero

# Check last backup time
velero schedule describe brrtrouter-daily
```

## Files Structure

```
BRRTRouter/
├── docker-compose-minio.yml       # MinIO service (outside KIND)
├── k8s/
│   ├── velero-namespace.yaml      # Velero namespace
│   ├── velero-crds.yaml           # Velero CRDs (download once)
│   ├── velero-credentials.yaml    # MinIO credentials
│   └── velero-deployment.yaml     # Velero deployment + schedule
├── scripts/
│   └── download-velero-crds.sh    # One-time CRD download
└── Tiltfile                       # Velero integration
```

## Comparison: Script vs Declarative

### Old Approach (Scripts) ❌

```bash
./scripts/setup-velero.sh  # Runs velero install CLI
# Problems:
# - Imperative, not reproducible
# - Not managed by Tilt
# - Hard to version control
# - Manual reinstall needed
```

### New Approach (Manifests) ✅

```bash
just download-velero-crds  # One-time
just dev-up                # Tilt manages everything
# Benefits:
# - Declarative, reproducible
# - Version controlled
# - Tilt auto-applies changes
# - Git-friendly
```

## Best Practices

### 1. Commit CRDs to Git

```bash
# After downloading CRDs
git add k8s/velero/crds.yaml
git commit -m "Add Velero CRDs v1.12.3"
```

This ensures all team members have the CRDs without downloading.

### 2. Keep MinIO Running

```bash
# Add to shell startup (~/.zshrc or ~/.bashrc)
alias dev-start='just start-minio && just dev-up'
```

### 3. Test Backups Monthly

```bash
# Monthly backup test
velero restore create test-$(date +%Y%m) \
    --from-backup brrtrouter-daily-latest \
    --namespace-mappings brrtrouter-dev:brrtrouter-test

# Verify
kubectl get all -n brrtrouter-test

# Cleanup
kubectl delete namespace brrtrouter-test
```

### 4. Monitor Backup Size

```bash
# Check MinIO storage
docker exec brrtrouter-minio du -sh /data/velero

# If too large, adjust retention
# Edit k8s/velero/backups.yaml:
# ttl: 168h0m0s  # 7 days instead of 30
```

## Summary

✅ **Declarative**: All Velero config in K8s manifests  
✅ **Tilt-managed**: Automatic deployment and updates  
✅ **Git-friendly**: Version controlled, reviewable  
✅ **Reproducible**: Same setup on every `tilt up`  
✅ **Off-cluster storage**: MinIO survives cluster deletion  
✅ **Automated**: Daily backups, 30-day retention  

**Setup:**
```bash
just download-velero-crds  # One-time
just start-minio           # Start backup server
just dev-up                # Tilt deploys Velero
```

**Daily use:**
```bash
just backup-now            # Manual backup
just backup-list           # See all backups
just backup-restore <name> # Restore
```

**No scripts needed!** Pure declarative Kubernetes manifests. 🎉

