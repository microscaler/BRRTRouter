# Tilt Dependency DAG Analysis

## 🔍 Critical Issue Found

**Problem**: `custom_build` and `ensure-builds-complete` are NOT properly connected!

The `custom_build` for Docker image:
- Has `deps` (file dependencies) ✅
- Does NOT have `resource_deps` ❌
- Runs whenever files change, NOT when builds complete

The `ensure-builds-complete`:
- Depends on `build-sample-ui` and `build-petstore` ✅
- But nothing depends on IT except `petstore` k8s resource ❌
- Does NOT block the `custom_build` ❌

## 📊 Current Execution Flow (BROKEN)

```
START
  │
  ├─> build-sample-ui (parallel) ────┐
  │                                   │
  ├─> build-brrtrouter (parallel)    │
  │         │                         │
  │         └─> gen-petstore          │
  │               │                   │
  │               └─> build-petstore  │
  │                     │             │
  │                     └─────────────┼─> ensure-builds-complete
  │                                   │   (just echoes, does nothing)
  │                                   │
  ├─> custom_build ← ❌ RACE CONDITION! ❌
  │   (docker build + kind load)      │
  │   Triggered by file deps:         │
  │   - build_artifacts/pet_store     │
  │   - examples/pet_store/static_site│
  │                                   │
  │   ⚠️  Can run BEFORE              │
  │       build-sample-ui completes!  │
  │                                   │
  └─> petstore k8s deployment         │
      (waits for ensure-builds-complete)
```

## 🐛 The Race Condition

1. Tilt starts `build-sample-ui` (parallel)
2. Tilt starts `build-brrtrouter` (parallel)
3. **`custom_build` watches files** (`static_site/`, `pet_store` binary)
4. `build-petstore` completes → `build_artifacts/pet_store` updated
5. **`custom_build` triggers IMMEDIATELY** (file changed!)
6. Docker build runs: `COPY ./examples/pet_store/static_site`
7. **BUT `build-sample-ui` might still be running!**
8. Docker copies OLD or INCOMPLETE `static_site/` files
9. Image is built with stale UI
10. `ensure-builds-complete` finishes (does nothing)
11. `petstore` deployment uses the broken image

## ✅ What Should Happen

```
START
  │
  ├─> build-sample-ui (parallel)
  │         │
  │         └─> examples/pet_store/static_site/ updated
  │
  └─> build-brrtrouter (parallel)
            │
            └─> gen-petstore
                  │
                  └─> build-petstore
                        │
                        └─> build_artifacts/pet_store updated
                              │
                              ├─> build-sample-ui DONE ✅
                              ├─> build-petstore DONE ✅
                              │
                              └─> WAIT POINT ⏸️
                                    │
                                    └─> custom_build (docker)
                                          │
                                          └─> petstore k8s
```

## 🔧 The Fix

### Problem: `custom_build` can't have `resource_deps`

Tilt's `custom_build` doesn't support `resource_deps` directly. We need a different approach.

### Solution: Use Explicit Triggers

**Option 1: Make custom_build manual, trigger from ensure-builds-complete**

```python
# Build completes, explicitly trigger docker build
local_resource(
    'ensure-builds-complete',
    'echo "✅ Triggering docker build" && tilt trigger brrtrouter-petstore',
    resource_deps=['build-sample-ui', 'build-petstore'],
    labels=['build'],
)

custom_build(
    'brrtrouter-petstore',
    'docker build ... && kind load ...',
    deps=[...],
    trigger_mode=TRIGGER_MODE_MANUAL,  # Only trigger explicitly
    ...
)
```

**Option 2: Use docker_build with proper ordering** (Better)

```python
# Step 1: Ensure builds complete
local_resource(
    'wait-for-builds',
    'echo "Builds complete"',
    resource_deps=['build-sample-ui', 'build-petstore'],
    labels=['build'],
)

# Step 2: Build Docker image (depends on builds)
local_resource(
    'docker-build-image',
    'docker build -t brrtrouter-petstore:dev -f Dockerfile.dev .',
    deps=[
        './build_artifacts/pet_store',
        './examples/pet_store/static_site',
    ],
    resource_deps=['wait-for-builds'],  # BLOCKS until builds done
    labels=['build'],
)

# Step 3: Load into kind (depends on docker build)
local_resource(
    'kind-load-image',
    'kind load docker-image brrtrouter-petstore:dev --name brrtrouter-dev',
    resource_deps=['docker-build-image'],
    labels=['build'],
)

# Step 4: Tag for k8s
custom_build(
    'brrtrouter-petstore',
    'docker tag brrtrouter-petstore:dev $EXPECTED_REF',
    deps=[],  # No file deps, only triggered by rebuild
    tag='dev',
    disable_push=True,
    live_update=[...],
)

# Step 5: K8s deployment
k8s_resource(
    'petstore',
    resource_deps=['kind-load-image', ...],  # Waits for image
)
```

## 📝 Current Dependencies (As Configured)

### Local Resources
```
build-sample-ui:
  deps: [sample-ui/src/, ...]
  resource_deps: []
  outputs: examples/pet_store/static_site/

build-brrtrouter:
  deps: [src/, Cargo.toml]
  resource_deps: []
  outputs: target/.../libbrrtrouter.rlib

gen-petstore:
  deps: [examples/openapi.yaml, templates/]
  resource_deps: [build-brrtrouter]
  outputs: examples/pet_store/src/

build-petstore:
  deps: [examples/pet_store/src/]
  resource_deps: [gen-petstore]
  outputs: build_artifacts/pet_store

ensure-builds-complete:
  resource_deps: [build-sample-ui, build-petstore]
  outputs: (nothing, just echo)
```

### Custom Build (THE PROBLEM)
```
custom_build('brrtrouter-petstore'):
  deps: [build_artifacts/pet_store, static_site/, ...]
  resource_deps: NONE ❌
  triggers: IMMEDIATELY when deps change ❌
```

### K8s Resources
```
petstore:
  resource_deps: [ensure-builds-complete, postgres, ...]
```

## 🎯 Root Cause

**`custom_build` triggers on file changes, ignoring build dependencies.**

When `build_artifacts/pet_store` is updated, `custom_build` runs immediately, even if `build-sample-ui` is still running and hasn't finished copying files to `static_site/`.

## ✅ Recommended Fix

Use separate `local_resource` steps with explicit dependencies:

```python
# 1. Ensure all builds complete
local_resource(
    'wait-for-builds',
    'echo "All builds complete"',
    resource_deps=['build-sample-ui', 'build-petstore'],
)

# 2. Build Docker image (AFTER builds)
local_resource(
    'docker-build',
    'docker build -t brrtrouter-petstore:tilt -f Dockerfile.dev .',
    deps=['./build_artifacts/pet_store', './examples/pet_store/static_site'],
    resource_deps=['wait-for-builds'],  # ← KEY FIX
)

# 3. Load into kind
local_resource(
    'kind-load',
    'kind load docker-image brrtrouter-petstore:tilt --name brrtrouter-dev',
    resource_deps=['docker-build'],
)

# 4. Simple custom_build for k8s
custom_build(
    'brrtrouter-petstore',
    'docker tag brrtrouter-petstore:tilt $EXPECTED_REF',
    deps=[],
    tag='tilt',
    disable_push=True,
    live_update=[...],
)

# 5. K8s waits for image
k8s_resource(
    'petstore',
    resource_deps=['kind-load', ...],
)
```

## 🔬 How to Verify

After fix:
```bash
# Watch Tilt UI - should see this order:
1. build-sample-ui (running)
2. build-petstore (running)
3. wait-for-builds (waiting...)
4. ✅ build-sample-ui (done)
5. ✅ build-petstore (done)
6. ✅ wait-for-builds (done)
7. docker-build (starting...)  ← Only NOW
8. kind-load (after docker)
9. petstore (deploying)
```

---

**Status**: 🐛 Race Condition Identified  
**Impact**: Static files copied before build completes  
**Fix**: Explicit build ordering with local_resource  
**Priority**: 🔥 Critical

