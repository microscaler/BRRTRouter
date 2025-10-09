# KIND Local Registry Fix

## Problem

Tilt was failing to deploy with:

```
Failed to pull image "localhost:5001/brrtrouter-petstore:tilt-8f2a723b9c191316"
dial tcp [::1]:5001: connect: connection refused
Error: ImagePullBackOff
```

**Root Cause:** We were missing critical pieces of the [KIND local registry setup](https://kind.sigs.k8s.io/docs/user/local-registry/) according to the official documentation.

## What Was Missing

According to the KIND docs, there are 5 steps to set up a local registry. We were missing:

1. ❌ **Step 2: containerd config patch in kind-config.yaml**
2. ✅ Step 3: Registry config on nodes (we had this)
3. ✅ Step 4: Connect registry to kind network (we had this)
4. ✅ Step 5: ConfigMap documentation (we had this)

### The Critical Missing Piece

**File:** `kind-config.yaml`

We were missing the containerd configuration that tells KIND to use `/etc/containerd/certs.d` for registry configuration:

```yaml
containerdConfigPatches:
  - |-
    [plugins."io.containerd.grpc.v1.cri".registry]
      config_path = "/etc/containerd/certs.d"
```

**Without this patch:**
- KIND nodes ignore `/etc/containerd/certs.d/localhost:5001/hosts.toml`
- Pods can't resolve `localhost:5001` to the registry
- Images fail to pull

**With this patch:**
- KIND nodes read `/etc/containerd/certs.d/` configurations
- `localhost:5001` is aliased to `kind-registry:5000`
- Pods can pull images successfully

## The Fix

### 1. Updated kind-config.yaml

**File:** `kind-config.yaml` (lines 6-11)

```yaml
# Enable local registry support
# See: https://kind.sigs.k8s.io/docs/user/local-registry/
containerdConfigPatches:
  - |-
    [plugins."io.containerd.grpc.v1.cri".registry]
      config_path = "/etc/containerd/certs.d"
```

### 2. Verified dev-setup.sh

The script already correctly implements steps 3-5:

```bash
# Step 3: Add registry config to nodes
REGISTRY_DIR="/etc/containerd/certs.d/localhost:5001"
for node in $(kind get nodes --name "brrtrouter-dev"); do
    docker exec "${node}" mkdir -p "${REGISTRY_DIR}"
    cat <<EOF | docker exec -i "${node}" cp /dev/stdin "${REGISTRY_DIR}/hosts.toml"
[host."http://kind-registry:5000"]
EOF
done

# Step 4: Connect registry to kind network
docker network connect "kind" "kind-registry"

# Step 5: Document the registry
kubectl apply -f - <<EOF
apiVersion: v1
kind: ConfigMap
metadata:
  name: local-registry-hosting
  namespace: kube-public
data:
  localRegistryHosting.v1: |
    host: "localhost:5001"
    help: "https://kind.sigs.k8s.io/docs/user/local-registry/"
EOF
```

### 3. Created Verification Script

**File:** `scripts/verify-registry.sh`

Comprehensive checks for:
- ✅ Registry container running
- ✅ Registry accessible from host
- ✅ Registry on kind network
- ✅ Containerd configuration on nodes
- ✅ Registry accessible from inside cluster

**Usage:**
```bash
just dev-registry-verify
```

## How It Works

### The Network Flow

```
Host Machine (localhost:5001)
    ↓ 
Docker (port mapping 127.0.0.1:5001 → container:5000)
    ↓
Registry Container (kind-registry:5000)
    ↓ (on 'kind' network)
KIND Node (containerd with hosts.toml)
    ↓ (resolves localhost:5001 → kind-registry:5000)
Pod pulls image "localhost:5001/brrtrouter-petstore:tilt"
```

### Key Insight from KIND Docs

> "localhost" resolves to loopback addresses that are network-namespace local.
> In other words: localhost in the container is not localhost on the host.
> 
> We want a consistent name that works from both ends, so we tell containerd to
> alias localhost:5001 to the registry container when pulling images.

**Why `localhost:5001` in pod YAML works:**

1. Pod tries to pull `localhost:5001/brrtrouter-petstore:tilt`
2. Containerd reads `/etc/containerd/certs.d/localhost:5001/hosts.toml`
3. Config says: "localhost:5001 → http://kind-registry:5000"
4. Containerd connects to `kind-registry:5000` on the kind network
5. Image pulls successfully!

## Complete Setup Process

### For New Clusters

```bash
# This now includes everything
just dev-up
```

**What happens:**
1. ✅ Starts/restarts registry (transparent)
2. ✅ Creates cluster with containerd patch
3. ✅ Configures registry on nodes
4. ✅ Connects registry to network
5. ✅ Documents registry in ConfigMap
6. ✅ Starts Tilt

### For Existing Clusters

If you have an existing cluster **without** the containerd patch:

```bash
# Recreate cluster with proper configuration
just dev-down
just dev-up
```

**Why recreate?**
- The `containerdConfigPatches` must be applied at cluster creation time
- Can't be added to an existing cluster
- Quick operation (~30 seconds)

## Verification

### Quick Check

```bash
# Verify everything is set up correctly
just dev-registry-verify
```

**Expected output:**
```
🔍 Verifying Local Registry Setup

1. Checking registry container...
   ✓ Registry container is running

2. Checking registry from host...
   ✓ Registry accessible at localhost:5001
      {"repositories":["brrtrouter-petstore"]}

3. Checking registry network connection...
   ✓ Registry is connected to 'kind' network
      Registry IP on kind network: 172.18.0.2

4. Checking KIND cluster...
   ✓ KIND cluster 'brrtrouter-dev' exists

5. Checking containerd configuration on nodes...
   Checking node: brrtrouter-dev-control-plane
      ✓ Config directory exists
      ✓ hosts.toml exists
      Content:
         [host."http://kind-registry:5000"]

6. Testing registry access from inside cluster...
   Using node: brrtrouter-dev-control-plane
   ✓ Can reach registry from node via name resolution
      {"repositories":["brrtrouter-petstore"]}

✅ Registry setup is complete and functional!
```

### Manual Checks

```bash
# 1. Check containerd config in node
kubectl get nodes
NODE_NAME=$(kubectl get nodes -o name | head -1 | cut -d/ -f2)
docker exec "${NODE_NAME}" cat /etc/containerd/certs.d/localhost:5001/hosts.toml
# Should show: [host."http://kind-registry:5000"]

# 2. Check ConfigMap
kubectl get configmap local-registry-hosting -n kube-public -o yaml

# 3. Test image pull from inside cluster
kubectl run test-pull --image=localhost:5001/brrtrouter-petstore:tilt --rm -it
```

## Troubleshooting

### Image Still Won't Pull

**Symptom:**
```
Error: ImagePullBackOff
dial tcp [::1]:5001: connect: connection refused
```

**Solutions:**

1. **Verify containerd patch is applied:**
   ```bash
   # Recreate cluster if patch is missing
   just dev-down
   just dev-up
   ```

2. **Check hosts.toml exists:**
   ```bash
   just dev-registry-verify
   # Look at section 5 output
   ```

3. **Verify registry is on kind network:**
   ```bash
   docker inspect kind-registry | grep -A 10 Networks
   # Should show both 'bridge' and 'kind'
   ```

### Registry Not Accessible

**Symptom:**
```
Cannot reach registry from inside node
```

**Solutions:**

1. **Restart registry and reconnect:**
   ```bash
   just dev-registry
   ```

2. **Check registry is running:**
   ```bash
   docker ps --filter "name=kind-registry"
   ```

3. **Test from host:**
   ```bash
   curl http://localhost:5001/v2/_catalog
   ```

## Files Changed

### Modified Files

1. **`kind-config.yaml`** (lines 6-11)
   - Added `containerdConfigPatches` for containerd registry config

2. **`scripts/dev-setup.sh`** (line 169)
   - Cleaned up node configuration logging

3. **`justfile`** (lines 213-215)
   - Added `dev-registry-verify` command

### New Files

1. **`scripts/verify-registry.sh`**
   - Comprehensive registry verification

2. **`docs/KIND_REGISTRY_FIX.md`**
   - This documentation

## References

- [KIND Local Registry Documentation](https://kind.sigs.k8s.io/docs/user/local-registry/)
- [Containerd Registry Configuration](https://github.com/containerd/containerd/blob/main/docs/hosts.md)
- [KEP-1755: Communicating a Local Registry](https://github.com/kubernetes/enhancements/tree/master/keps/sig-cluster-lifecycle/generic/1755-communicating-a-local-registry)

## Key Takeaways

1. **containerdConfigPatches is critical** - Without it, none of the registry configuration works
2. **Must recreate cluster** - This patch can't be added to existing clusters
3. **localhost:5001 works everywhere** - Host, Tilt, and pods all use the same address
4. **kind-registry:5000 is internal** - Only used by containerd, not in YAML
5. **Verification is essential** - Use `just dev-registry-verify` to confirm setup

## Migration Guide

### For Developers with Existing Clusters

```bash
# 1. Save any important work
git stash

# 2. Tear down old cluster
just dev-down

# 3. Pull latest changes (includes kind-config.yaml update)
git pull

# 4. Create new cluster with proper configuration
just dev-up

# 5. Verify it works
just dev-registry-verify

# 6. Restore work
git stash pop
```

**Time required:** ~2 minutes

## Summary

**Problem:** KIND pods couldn't pull from localhost:5001 registry

**Root Cause:** Missing `containerdConfigPatches` in kind-config.yaml

**Solution:** 
- ✅ Added containerd config patch
- ✅ Created verification script  
- ✅ Documented complete setup

**Result:** Pods can now pull images from localhost:5001 successfully! 🎉

**Next Steps:**
1. Recreate your cluster: `just dev-down && just dev-up`
2. Verify setup: `just dev-registry-verify`
3. Start developing: Images will pull correctly!

