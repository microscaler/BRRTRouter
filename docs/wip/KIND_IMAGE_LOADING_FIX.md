# Kind Image Loading Fix

## 🐛 The Problem

```
Failed to pull image "brrtrouter-petstore:tilt-a03bd1fd8b5f703b": 
pull access denied, repository does not exist or may require authorization
```

### Root Cause

1. Docker image built **locally** on host machine
2. Kind cluster tries to pull image from **Docker Hub**
3. Image doesn't exist on Docker Hub
4. Kind can't access the local Docker image

## ✅ The Solution

### Load Local Images into Kind

```bash
kind load docker-image <image-name> --name <cluster-name>
```

This copies the local Docker image into the kind cluster's containerd cache.

## 🔧 Implementation

### Updated custom_build

```python
custom_build(
    'brrtrouter-petstore',
    # Build AND load into kind in one command
    'docker build -t $EXPECTED_REF -f dockerfiles/Dockerfile.dev . && kind load docker-image $EXPECTED_REF --name brrtrouter-dev',
    deps=[...],
    skips_local_docker=True,  # We're using kind, not local docker
    disable_push=True,         # Don't push to registry
    live_update=[...],
)
```

### Dependency Enforcement

```python
# Separate resource to enforce build order
local_resource(
    'ensure-builds-complete',
    'echo "✅ UI and binary builds complete"',
    resource_deps=[
        'build-sample-ui',
        'build-petstore',
    ],
)

k8s_resource(
    'petstore',
    resource_deps=[
        'ensure-builds-complete',  # Waits for builds
        ...
    ]
)
```

## 🔍 How It Works

```
1. build-sample-ui completes
   build-petstore completes
        ↓
2. ensure-builds-complete runs
   (just an echo, completes instantly)
        ↓
3. custom_build triggered
   (deps changed: static_site/, pet_store binary)
        ↓
4. docker build -t brrtrouter-petstore:tilt-abc123...
   (builds image locally)
        ↓
5. kind load docker-image brrtrouter-petstore:tilt-abc123 --name brrtrouter-dev
   (copies image into kind's containerd)
        ↓
6. petstore deployment pulls image
   (finds it in kind's local cache)
        ↓
7. ✅ Pod starts successfully
```

## 📊 Key Changes

### Before (Broken)

```python
# Trying to use separate local_resource + custom_build
local_resource('docker-build-petstore', ...)
custom_build('brrtrouter-petstore', 'docker tag ...', ...)

# ❌ Image not in kind cluster
# ❌ K8s tries to pull from Docker Hub
# ❌ ImagePullBackOff
```

### After (Fixed)

```python
# Single custom_build with kind load
custom_build(
    'brrtrouter-petstore',
    'docker build ... && kind load docker-image ...',
    ...
)

# ✅ Image built locally
# ✅ Image loaded into kind
# ✅ Pod starts successfully
```

## 🎯 Why `skips_local_docker=True`

```python
skips_local_docker=True,  # Important!
```

This tells Tilt:
- Don't use local Docker daemon for K8s
- Image is in the kind cluster
- Don't try to push to a registry

## 🔄 Live Updates

Live updates still work because:

1. Files sync directly to pod filesystem
2. No image rebuild needed
3. Process reloads via `kill -HUP 1`

```python
live_update=[
    sync('./build_artifacts/pet_store', '/app/pet_store'),
    sync('./examples/pet_store/static_site/', '/app/static_site/'),
    run('kill -HUP 1', trigger=['./build_artifacts/pet_store']),
]
```

## 📋 Verification

### Check Image in Kind

```bash
# List images in kind cluster
docker exec -it brrtrouter-dev-control-plane crictl images | grep brrtrouter

# Should see:
# brrtrouter-petstore  tilt-abc123...  ...
```

### Check Pod Status

```bash
kubectl get pods -n brrtrouter-dev

# Should see:
# NAME                        READY   STATUS    RESTARTS   AGE
# petstore-xxx-yyy            1/1     Running   0          30s
```

### Check Pod Events

```bash
kubectl describe pod -n brrtrouter-dev -l app=petstore

# Should see:
# Events:
#   Type    Reason     Age   From     Message
#   ----    ------     ----  ----     -------
#   Normal  Pulled     30s   kubelet  Successfully pulled image "brrtrouter-petstore:tilt-..."
#   Normal  Created    30s   kubelet  Created container petstore
#   Normal  Started    30s   kubelet  Started container petstore
```

## 🚀 Complete Build Flow

```
┌─────────────────────────────────────────┐
│ 1. Local Builds (Parallel)             │
│    - build-sample-ui                    │
│    - build-brrtrouter → gen → build     │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│ 2. ensure-builds-complete               │
│    (enforces dependencies)              │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│ 3. custom_build (when deps change)      │
│    docker build -t $REF ...             │
│    kind load docker-image $REF          │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│ 4. K8s Deployment                       │
│    Pulls image from kind's cache        │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│ 5. ✅ Pod Running                       │
│    http://localhost:8080                │
└─────────────────────────────────────────┘
```

## 💡 Alternative Approaches (Not Used)

### Option 1: Docker Registry

```python
# Run local registry
# Push images to localhost:5000
# Configure kind to use registry
```
**Cons**: More complex, another service to manage

### Option 2: Build Inside Kind

```python
# Build directly in kind node
docker exec kind-control-plane docker build ...
```
**Cons**: Slower, loses caching benefits

### Option 3: Pre-load on Cluster Creation

```bash
# Load image once when creating cluster
kind create cluster --name brrtrouter-dev
kind load docker-image brrtrouter-petstore:latest
```
**Cons**: Doesn't work for incremental builds

## 🎯 Our Approach (Best for Tilt)

✅ **`kind load docker-image` in `custom_build`**

**Pros:**
- Simple integration with Tilt
- Automatic on every build
- Works with Tilt's caching
- Fast incremental updates
- Live updates still work

## 📚 References

- [Kind Documentation: Loading Images](https://kind.sigs.k8s.io/docs/user/quick-start/#loading-an-image-into-your-cluster)
- [Tilt Documentation: custom_build](https://docs.tilt.dev/custom_build.html)
- [Tilt + Kind Integration](https://docs.tilt.dev/choosing_clusters.html#kind)

---

**Status**: ✅ Images Load into Kind Automatically  
**Method**: custom_build + kind load  
**Result**: No more ImagePullBackOff  
**Date**: October 9, 2025

