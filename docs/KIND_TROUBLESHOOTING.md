# KIND Cluster Troubleshooting

## Common Issue: Kubelet Health Check Timeout

### Symptoms

```
[kubelet-check] The kubelet is not healthy after 4m0.009063401s
ERROR: failed to create cluster: failed to init node with kubeadm
```

### Root Causes

1. **Docker resource limits** (most common on macOS)
2. **Conflicting existing cluster/resources**
3. **Docker daemon issues**
4. **System resource constraints**

## Solutions

### Solution 1: Clean Up and Retry (Recommended First Step)

```bash
# Stop everything
just dev-down

# Clean up Docker
docker system prune -a --volumes -f

# Remove any orphaned KIND clusters
kind delete clusters --all

# Restart Docker Desktop
# macOS: Docker Desktop → Quit Docker Desktop → Start Docker Desktop

# Try again
just dev-up
```

### Solution 2: Increase Docker Resources (macOS)

Docker Desktop → Settings → Resources:

**Minimum recommended:**
- **CPUs:** 4 cores
- **Memory:** 8 GB
- **Swap:** 2 GB
- **Disk:** 60 GB

**Optimal for BRRTRouter:**
- **CPUs:** 6-8 cores
- **Memory:** 12-16 GB
- **Swap:** 4 GB
- **Disk:** 100 GB

After changing, click "Apply & Restart"

### Solution 3: Simplify KIND Configuration

If you're still having issues, temporarily use a minimal KIND config:

```bash
# Backup current config
cp k8s/cluster/kind-config.yaml k8s/cluster/kind-config.yaml.bak

# Create minimal config
cat > kind-config-minimal.yaml <<'EOF'
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
name: brrtrouter-dev
nodes:
  - role: control-plane
    extraPortMappings:
      - containerPort: 30900
        hostPort: 9090
      - containerPort: 30300
        hostPort: 3000
      - containerPort: 30090
        hostPort: 8080
      - containerPort: 30686
        hostPort: 16686
EOF

# Create cluster with minimal config
kind create cluster --config kind-config-minimal.yaml --wait 60s
```

### Solution 4: Check Docker Status

```bash
# Check Docker is running
docker info

# Check Docker daemon logs (macOS)
# Docker Desktop → Troubleshoot → View logs

# Check available resources
docker system df

# Check running containers
docker ps -a
```

### Solution 5: Use Older KIND Node Image

Sometimes the latest Kubernetes version has issues. Try an older stable version:

```bash
# Edit k8s/cluster/kind-config.yaml and add:
nodes:
  - role: control-plane
    image: kindest/node:v1.29.2  # Older stable version
```

Or use KIND's default (don't specify image):

```bash
kind create cluster --name brrtrouter-dev
```

### Solution 6: Check System Resources

```bash
# macOS: Check available memory
vm_stat | grep "Pages free"

# Check disk space
df -h

# Check CPU load
top -l 1 | grep "CPU usage"
```

## Step-by-Step Debugging

### 1. Clean Slate

```bash
# Kill all Docker containers
docker kill $(docker ps -q) 2>/dev/null || true

# Remove all containers
docker rm -f $(docker ps -aq) 2>/dev/null || true

# Remove KIND networks
docker network rm kind 2>/dev/null || true

# Remove volumes (WARNING: removes all data)
docker volume prune -f

# Restart Docker Desktop
```

### 2. Test Docker

```bash
# Test basic Docker functionality
docker run --rm hello-world

# Test Docker networking
docker run --rm alpine ping -c 3 google.com
```

### 3. Create Minimal KIND Cluster

```bash
# Create simplest possible cluster
kind create cluster --name test-cluster

# If successful, delete it
kind delete cluster --name test-cluster

# Then try full BRRTRouter setup
just dev-up
```

### 4. Check Logs

```bash
# If cluster creation fails, check logs
docker logs brrtrouter-dev-control-plane

# Check Docker events
docker events --since 10m
```

## Known Issues

### macOS Apple Silicon (M1/M2/M3)

**Issue:** KIND node image might not be compatible

**Solution:** Use explicit AMD64 images or wait for ARM64 support

```yaml
# In k8s/cluster/kind-config.yaml
nodes:
  - role: control-plane
    image: kindest/node:v1.29.2@sha256:... # Use specific ARM64 hash
```

### Docker Desktop File Sharing

**Issue:** KIND needs access to mounted volumes

**Solution:** 
1. Docker Desktop → Settings → Resources → File Sharing
2. Add `/var/lib/docker/volumes` if not present
3. Apply & Restart

### VPN/Proxy Issues

**Issue:** VPN or corporate proxy blocking Docker networking

**Solution:**
```bash
# Disable VPN temporarily
# Or configure Docker proxy settings
# Docker Desktop → Settings → Resources → Proxies
```

## Quick Fixes Reference

| Issue | Command | Time |
|-------|---------|------|
| Stale cluster | `kind delete clusters --all` | 5s |
| Docker cache | `docker system prune -a -f` | 30s |
| Restart Docker | Docker Desktop → Restart | 60s |
| Minimal cluster | `kind create cluster --name test` | 2m |
| Clean volumes | `docker volume prune -f` | 10s |

## Still Not Working?

### Option 1: Use Docker Compose (Fallback)

If KIND continues to fail, consider running services individually:

```bash
# Run PostgreSQL in Docker
docker run -d --name postgres -p 5432:5432 \
  -e POSTGRES_USER=brrtrouter \
  -e POSTGRES_PASSWORD=dev_password \
  -e POSTGRES_DB=brrtrouter \
  postgres:15

# Run Redis in Docker
docker run -d --name redis -p 6379:6379 redis:7
```

### Option 2: Run Services Natively

```bash
# Run without Kubernetes
cargo run -p pet_store -- --spec doc/openapi.yaml --port 8080
```

### Option 3: Check System Compatibility

```bash
# macOS version
sw_vers

# Docker version
docker --version

# KIND version
kind --version

# Kubernetes version
kubectl version --client
```

**Minimum requirements:**
- macOS: 12.0+ (Monterey)
- Docker Desktop: 4.20+
- KIND: 0.20+
- kubectl: 1.27+

## Success Indicators

When KIND cluster starts successfully, you should see:

```
✓ Ensuring node image (kindest/node:v1.33.1)
✓ Preparing nodes
✓ Writing configuration
✓ Starting control-plane
✓ Installing CNI
✓ Installing StorageClass
✓ Waiting ≤ 60s for control-plane = Ready
```

Then:
```bash
# Verify cluster is running
kubectl get nodes

# Should show:
# NAME                           STATUS   ROLES           AGE   VERSION
# brrtrouter-dev-control-plane   Ready    control-plane   1m    v1.33.1
```

## Prevention

### Regular Maintenance

```bash
# Weekly cleanup
docker system prune -f
docker volume prune -f

# Before starting work
docker ps -a  # Check for orphaned containers
kind get clusters  # Check for orphaned clusters
```

### Resource Monitoring

```bash
# Monitor Docker resource usage
docker stats

# Check disk usage
docker system df
```

## Contact

If none of these solutions work, please file an issue with:

1. Output of `docker info`
2. Output of `kind version`
3. macOS version (`sw_vers`)
4. Complete error log
5. Output of `docker logs brrtrouter-dev-control-plane` (if container exists)

## Alternative: Use Existing Kubernetes

If you have an existing Kubernetes cluster (minikube, k3s, etc.), you can adapt the `k8s/` manifests to use it instead of KIND.

