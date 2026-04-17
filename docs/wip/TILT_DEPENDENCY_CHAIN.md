# Tilt Dependency Chain

## 🔗 Complete Build Order

```
┌─────────────────────────────────────────────────────────────┐
│ PHASE 1: PARALLEL LOCAL BUILDS                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────────┐          ┌──────────────────┐        │
│  │ build-sample-ui  │          │ build-brrtrouter │        │
│  │ (SolidJS+Tail)   │          │ (Rust lib)       │        │
│  │ ~2-3s            │          │ ~10-30s          │        │
│  └────────┬─────────┘          └────────┬─────────┘        │
│           │                             │                  │
│           │ Outputs:                    │ Outputs:         │
│           │ examples/pet_store/         │ target/x86_64.../│
│           │   static_site/              │   libbrrtrouter  │
│           │                             │                  │
└───────────┼─────────────────────────────┼──────────────────┘
            │                             │
            │                             ▼
            │                    ┌──────────────────┐
            │                    │  gen-petstore    │
            │                    │  (OpenAPI gen)   │
            │                    │  ~2-5s           │
            │                    └────────┬─────────┘
            │                             │
            │                             │ Outputs:
            │                             │ examples/pet_store/src/
            │                             │
            │                             ▼
            │                    ┌──────────────────┐
            │                    │ build-petstore   │
            │                    │ (Rust binary)    │
            │                    │ ~10-20s          │
            │                    └────────┬─────────┘
            │                             │
            │                             │ Outputs:
            │                             │ build_artifacts/pet_store
            │                             │
            ▼                             ▼
┌─────────────────────────────────────────────────────────────┐
│ PHASE 2: DOCKER IMAGE BUILD                                │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│         ┌──────────────────────────────┐                   │
│         │   docker_build               │                   │
│         │   (brrtrouter-petstore)      │                   │
│         │   ~5-10s                     │                   │
│         └──────────────┬───────────────┘                   │
│                        │                                    │
│     Copies into image: │                                    │
│     - build_artifacts/pet_store                            │
│     - examples/pet_store/config/                           │
│     - examples/pet_store/doc/                              │
│     - examples/pet_store/static_site/  ← From build-sample-ui│
│                        │                                    │
└────────────────────────┼────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│ PHASE 3: KUBERNETES RESOURCES                              │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐     ┌──────────────┐                     │
│  │  postgres    │     │    redis     │                     │
│  └──────┬───────┘     └──────┬───────┘                     │
│         │                    │                             │
│         └──────────┬─────────┘                             │
│                    │                                        │
│                    ▼                                        │
│         ┌──────────────────┐                               │
│         │   prometheus     │                               │
│         └────────┬─────────┘                               │
│                  │                                          │
│         ┌────────┴────────┐                                │
│         │                 │                                │
│         ▼                 ▼                                │
│  ┌───────────┐     ┌───────────┐                          │
│  │  grafana  │     │  jaeger   │                          │
│  └───────────┘     └─────┬─────┘                          │
│                           │                                │
│                           ▼                                │
│                  ┌──────────────────┐                      │
│                  │  otel-collector  │                      │
│                  └────────┬─────────┘                      │
│                           │                                │
│  ┌────────────────────────┴─────────────┐                 │
│  │    All deps ready                    │                 │
│  └────────────────────┬─────────────────┘                 │
│                       │                                    │
│                       ▼                                    │
│              ┌──────────────────┐                          │
│              │    petstore      │                          │
│              │  (deployment)    │                          │
│              └──────────────────┘                          │
│                                                             │
│              http://localhost:8080                         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## 📋 Resource Dependencies

### Local Resources (Build Steps)

```python
build-sample-ui:
  deps: [sample-ui/src/, *.config.js]
  resource_deps: []
  parallel: True
  outputs: examples/pet_store/static_site/

build-brrtrouter:
  deps: [src/, Cargo.toml]
  resource_deps: []
  parallel: True
  outputs: target/x86_64-unknown-linux-musl/release/libbrrtrouter.rlib

gen-petstore:
  deps: [examples/openapi.yaml, templates/]
  resource_deps: [build-brrtrouter]
  parallel: False
  outputs: examples/pet_store/src/

build-petstore:
  deps: [examples/pet_store/src/, examples/pet_store/Cargo.toml]
  resource_deps: [gen-petstore]
  parallel: False
  outputs: build_artifacts/pet_store
```

### Docker Image

```python
docker_build('brrtrouter-petstore'):
  only: [
    build_artifacts/pet_store,           ← from build-petstore
    examples/pet_store/config/,
    examples/pet_store/doc/,
    examples/pet_store/static_site/,     ← from build-sample-ui
  ]
  # Implicitly depends on build-sample-ui and build-petstore
  # via k8s_resource dependencies
```

### Kubernetes Resources

```python
petstore:
  resource_deps: [
    build-sample-ui,    # ← CRITICAL: UI must be built first
    build-petstore,     # ← CRITICAL: Binary must be built first
    postgres,
    redis,
    prometheus,
    otel-collector,
  ]
```

## 🔍 Why This Order Matters

### ❌ Without Proper Dependencies

```
build-petstore finishes
    ↓
docker_build starts immediately
    ↓
COPIES examples/pet_store/static_site/
    ↓
⚠️  BUT build-sample-ui is still running!
    ↓
Docker image has OLD or EMPTY static files
    ↓
❌ UI doesn't appear or shows stale content
```

### ✅ With Proper Dependencies

```
build-sample-ui finishes
    ↓
examples/pet_store/static_site/ populated
    ↓
build-petstore finishes
    ↓
build_artifacts/pet_store ready
    ↓
docker_build starts
    ↓
COPIES both static_site/ AND pet_store binary
    ↓
✅ Image has LATEST UI and binary
    ↓
petstore deployment starts
    ↓
✅ Everything works!
```

## 🎯 Key Changes Made

### Before (Broken)

```python
custom_build(
    'brrtrouter-petstore',
    'docker build -t $EXPECTED_REF -f dockerfiles/Dockerfile.dev .',
    deps=[...],  # File dependencies only
    # ❌ No resource_deps
)

k8s_resource(
    'petstore',
    resource_deps=[
        'build-petstore',  # Only binary
        # ❌ Missing 'build-sample-ui'
        ...
    ]
)
```

### After (Fixed)

```python
docker_build(
    'brrtrouter-petstore',
    context='.',
    dockerfile='dockerfiles/Dockerfile.dev',
    only=[...],
    # ✅ Dependencies enforced via k8s_resource
)

k8s_resource(
    'petstore',
    resource_deps=[
        'build-sample-ui',    # ✅ UI built first
        'build-petstore',     # ✅ Binary built first
        'postgres',
        'redis',
        'prometheus',
        'otel-collector'
    ]
)
```

## 🔄 Live Update Flow

After initial build, when you edit files:

### UI Changes
```
Edit sample-ui/src/App.jsx
    ↓
Tilt detects change
    ↓
build-sample-ui rebuilds (~2-3s)
    ↓
examples/pet_store/static_site/ updated
    ↓
Tilt live_update syncs to /app/static_site/
    ↓
✅ Refresh browser, see changes
```

### Binary Changes
```
Edit src/server/service.rs
    ↓
build-brrtrouter rebuilds (~5-10s)
    ↓
gen-petstore runs (if needed)
    ↓
build-petstore rebuilds (~10-20s)
    ↓
build_artifacts/pet_store updated
    ↓
Tilt live_update syncs to /app/pet_store
    ↓
Tilt runs 'kill -HUP 1' to reload
    ↓
✅ Service reloads with new code
```

## ⏱️ Initial Build Timeline

| Step | Time | Cumulative |
|------|------|------------|
| build-sample-ui | 2-3s | 2-3s |
| build-brrtrouter | 10-30s | 10-30s (parallel) |
| gen-petstore | 2-5s | 12-35s |
| build-petstore | 10-20s | 22-55s |
| docker_build | 5-10s | 27-65s |
| k8s deploy | 10-15s | 37-80s |
| **Total** | **37-80s** | **First-time startup** |

### Incremental Builds (After First Build)

| Change | Time | Notes |
|--------|------|-------|
| UI only | 3-5s | build-sample-ui + live_update |
| Rust lib | 5-10s | Incremental compile |
| Binary | 10-20s | Incremental compile + live_update |
| Config | < 2s | live_update only |
| Static files | < 2s | live_update only |

## 🛠️ Verifying Dependencies

### Check Build Order in Tilt UI

1. Open Tilt UI (press space or visit http://localhost:10353)
2. Watch resource order:
   - `build-sample-ui` and `build-brrtrouter` start immediately (parallel)
   - `gen-petstore` waits for `build-brrtrouter`
   - `build-petstore` waits for `gen-petstore`
   - `brrtrouter-petstore` (Docker) waits for both UI and binary
   - `petstore` (K8s) waits for everything

### Check Resource Dependencies

```bash
# In Tilt UI, click any resource to see:
# - "Waiting for: <dependency>"
# - "Dependencies: <list>"
```

### Test Dependency Chain

```bash
# Clean start
tilt down
rm -rf build_artifacts/ examples/pet_store/static_site/dist/

# Watch order
tilt up

# Should see:
# 1. build-sample-ui (starting)
# 2. build-brrtrouter (starting)  <- parallel
# 3. build-sample-ui (success)
# 4. build-brrtrouter (success)
# 5. gen-petstore (starting)      <- waits for brrtrouter
# 6. gen-petstore (success)
# 7. build-petstore (starting)    <- waits for gen
# 8. build-petstore (success)
# 9. docker build (starting)      <- waits for UI + binary
# 10. petstore deploy (starting)  <- waits for everything
```

## 📚 Related Documentation

- `docs/SAMPLE_UI_BUILD_FLOW.md` - Detailed UI build pipeline
- `docs/SAMPLE_UI_TILT_INTEGRATION.md` - UI integration details
- `docs/LOCAL_DEVELOPMENT.md` - Overall dev workflow

---

**Status**: ✅ Dependencies Fixed  
**Build Order**: Correct  
**Parallel Builds**: Optimized  
**Date**: October 9, 2025

