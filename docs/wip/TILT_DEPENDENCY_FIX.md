# Tilt Dependency Chain - FIXED

## 🔍 The Problem

### What Was Wrong

```python
# BEFORE - BROKEN
docker_build(
    'brrtrouter-petstore',
    ...
    # ❌ NO resource_deps parameter exists for docker_build!
    # ❌ Tilt starts building Docker image immediately
    # ❌ Race condition: Docker build vs build-sample-ui
)

k8s_resource(
    'petstore',
    resource_deps=[
        'build-sample-ui',  # ❌ This doesn't block Docker build!
        'build-petstore',   # ❌ This doesn't block Docker build!
        ...
    ]
)
```

### Why It Failed

1. `docker_build()` doesn't support `resource_deps`
2. `docker_build()` starts as soon as the K8s YAML is loaded
3. `build-sample-ui` runs in parallel with Docker build
4. Docker's `COPY ./examples/pet_store/static_site` happens BEFORE `yarn build:copy` completes
5. Result: **Empty or stale UI files in the container**

## ✅ The Solution

### Split Docker Build into Two Steps

```python
# STEP 1: Build Docker image as a local_resource (with proper dependencies)
local_resource(
    'docker-build-petstore',
    'docker build -t brrtrouter-petstore:tilt-local -f dockerfiles/Dockerfile.dev .',
    deps=[...],
    resource_deps=[
        'build-sample-ui',  # ✅ BLOCKS until UI is built
        'build-petstore',   # ✅ BLOCKS until binary is built
    ],
    allow_parallel=False,
)

# STEP 2: Tell Tilt about the image (just tag it)
custom_build(
    'brrtrouter-petstore',
    'docker tag brrtrouter-petstore:tilt-local $EXPECTED_REF',
    ...,
    tag='tilt-local',
)

# STEP 3: K8s deployment depends on docker build completing
k8s_resource(
    'petstore',
    resource_deps=[
        'docker-build-petstore',  # ✅ Image is ready
        ...
    ]
)
```

## 📊 New Build Flow

```
┌────────────────────────────────────────────────────────────┐
│ PHASE 1: PARALLEL LOCAL BUILDS                            │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  ┌─────────────────┐        ┌──────────────────┐          │
│  │ build-sample-ui │        │ build-brrtrouter │          │
│  │ (2-3s)          │        │ (10-30s)         │          │
│  └────────┬────────┘        └────────┬─────────┘          │
│           │                          │                    │
│           │                          ▼                    │
│           │                 ┌──────────────────┐          │
│           │                 │  gen-petstore    │          │
│           │                 │  (2-5s)          │          │
│           │                 └────────┬─────────┘          │
│           │                          │                    │
│           │                          ▼                    │
│           │                 ┌──────────────────┐          │
│           │                 │ build-petstore   │          │
│           │                 │ (10-20s)         │          │
│           │                 └────────┬─────────┘          │
│           │                          │                    │
└───────────┼──────────────────────────┼────────────────────┘
            │                          │
            │ ✅ examples/pet_store/   │ ✅ build_artifacts/
            │    static_site/ READY    │    pet_store READY
            │                          │
            └────────────┬─────────────┘
                         │
                         │ ⚠️  BOTH MUST COMPLETE FIRST
                         │
                         ▼
┌────────────────────────────────────────────────────────────┐
│ PHASE 2: DOCKER BUILD (Now a local_resource)              │
├────────────────────────────────────────────────────────────┤
│                                                            │
│              ┌────────────────────────┐                    │
│              │ docker-build-petstore  │                    │
│              │ (local_resource)       │                    │
│              │ (5-10s)                │                    │
│              └───────────┬────────────┘                    │
│                          │                                 │
│   dockerfiles/Dockerfile.dev COPY:   │                                 │
│   - build_artifacts/pet_store ✅                           │
│   - examples/pet_store/config/ ✅                          │
│   - examples/pet_store/doc/ ✅                             │
│   - examples/pet_store/static_site/ ✅ (UI files present!) │
│                          │                                 │
│   Output:                │                                 │
│   brrtrouter-petstore:tilt-local                          │
│                          │                                 │
└──────────────────────────┼─────────────────────────────────┘
                           │
                           ▼
┌────────────────────────────────────────────────────────────┐
│ PHASE 3: IMAGE TAGGING (custom_build)                     │
├────────────────────────────────────────────────────────────┤
│                                                            │
│         docker tag brrtrouter-petstore:tilt-local          │
│                    $EXPECTED_REF                           │
│                                                            │
│   (Tilt's K8s integration sees this image)                 │
│                          │                                 │
└──────────────────────────┼─────────────────────────────────┘
                           │
                           ▼
┌────────────────────────────────────────────────────────────┐
│ PHASE 4: KUBERNETES DEPLOYMENT                            │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  postgres, redis → prometheus → grafana, jaeger →         │
│  otel-collector → petstore (with correct image!)          │
│                                                            │
│              http://localhost:8080 ✅                      │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

## 🎯 Key Changes

### 1. Docker Build is Now a `local_resource`

**Before:**
```python
docker_build('brrtrouter-petstore', ...)
# Starts immediately, no blocking
```

**After:**
```python
local_resource(
    'docker-build-petstore',
    'docker build ...',
    resource_deps=['build-sample-ui', 'build-petstore']
)
# ✅ BLOCKS until dependencies complete
```

### 2. Use `custom_build` for Image Tagging

```python
custom_build(
    'brrtrouter-petstore',
    'docker tag brrtrouter-petstore:tilt-local $EXPECTED_REF',
    tag='tilt-local',
    disable_push=True,
)
```

This tells Tilt's K8s integration about the image without triggering a rebuild.

### 3. K8s Deployment Depends on Docker Build

**Before:**
```python
k8s_resource(
    'petstore',
    resource_deps=['build-sample-ui', 'build-petstore', ...]  # ❌ Doesn't help
)
```

**After:**
```python
k8s_resource(
    'petstore',
    resource_deps=['docker-build-petstore', ...]  # ✅ Image is ready
)
```

## 📋 Complete Dependency Tree

```
build-sample-ui (parallel)
    └─> examples/pet_store/static_site/

build-brrtrouter (parallel)
    └─> gen-petstore
        └─> build-petstore
            └─> build_artifacts/pet_store

docker-build-petstore
    ├─ depends on: build-sample-ui ✅
    ├─ depends on: build-petstore ✅
    └─> brrtrouter-petstore:tilt-local

custom_build (brrtrouter-petstore)
    └─ tags: brrtrouter-petstore:tilt-local

postgres (parallel)
redis (parallel)
    └─> prometheus
        ├─> grafana
        └─> jaeger
            └─> otel-collector
                └─> petstore
                    └─ depends on: docker-build-petstore ✅
```

## ⏱️ Build Timeline

| Step | Time | Cumulative | Notes |
|------|------|------------|-------|
| build-sample-ui | 2-3s | 2-3s | Parallel |
| build-brrtrouter | 10-30s | 10-30s | Parallel |
| gen-petstore | 2-5s | 12-35s | Sequential |
| build-petstore | 10-20s | 22-55s | Sequential |
| **⏸️  WAIT POINT** | **0s** | **22-55s** | **Both builds done** |
| docker-build-petstore | 5-10s | 27-65s | Now starts |
| K8s deploy | 10-15s | 37-80s | Final step |

## 🔍 Verification

### Check Build Order

```bash
tilt up

# Watch the order in Tilt UI:
# 1. build-sample-ui (green) ✅
# 2. build-petstore (green) ✅
# 3. docker-build-petstore (starting...) ← Only NOW
# 4. petstore (waiting...) ← Depends on docker build
```

### Verify Files in Image

```bash
# After docker-build-petstore completes
docker run --rm brrtrouter-petstore:tilt-local ls -la /app/static_site/

# Should see:
# index.html
# assets/
#   index-[hash].js
#   index-[hash].css
```

### Test the UI

```bash
# After petstore pod is running
curl http://localhost:8080/

# Should return the SolidJS app HTML with:
# - Tailwind CSS link
# - SolidJS bundle script
# - Proper asset hashes
```

## 🐛 Troubleshooting

### Issue: "Cannot find image brrtrouter-petstore:tilt-local"

**Cause**: Docker build failed or didn't tag correctly  
**Fix**: Check `docker-build-petstore` logs in Tilt UI

### Issue: Still seeing empty static_site/

**Cause**: `build-sample-ui` is failing  
**Fix**: 
```bash
cd sample-ui
yarn install
yarn build:copy
```

### Issue: UI loads but is blank/old

**Cause**: Tilt live_update might not be syncing  
**Fix**: Force rebuild:
```bash
tilt down
rm -rf examples/pet_store/static_site/*
tilt up
```

## 📚 Related Files

- `Tiltfile` - Main configuration (UPDATED)
- `dockerfiles/Dockerfile.dev` - Docker image definition
- `sample-ui/scripts/copy-to-petstore.js` - UI copy script
- `k8s/petstore-deployment.yaml` - K8s deployment (uses image)

---

**Status**: ✅ Dependencies Correctly Enforced  
**Method**: local_resource + custom_build  
**Guarantee**: UI always built before Docker image  
**Date**: October 9, 2025

