# The Staging Area Pattern - Comprehensive Documentation

## What Is It?

The **staging area pattern** is used in both **Tilt** and **curl integration tests** to enable fast, local compilation while still using Docker containers.

## The Pattern

```
1. Build locally:  cargo zigbuild → target/x86_64-unknown-linux-musl/release/pet_store
2. Stage locally:  cp → build_artifacts/pet_store  
3. Docker copies:  COPY build_artifacts/pet_store → /pet_store
```

## Why We Need It

### The .dockerignore Problem

Our `.dockerignore` file contains:
```
target/*                    ← Blocks ALL of target/ (for performance)
!build_artifacts/pet_store  ← But explicitly allows this file
```

**Why block `target/*`?**
- The `target/` directory contains GB of build artifacts
- Sending it to Docker's build context is SLOW (minutes to copy)
- We don't want Docker re-compiling Rust (that's slow too)

**The consequence:**
- Docker can't access `target/x86_64-unknown-linux-musl/release/pet_store`
- Even though the file exists on the host!
- Trying to `COPY target/.../pet_store` in dockerfiles/Dockerfile fails with "not found"

### The Solution: Staging Area

Copy the binary to `build_artifacts/` which IS allowed by `.dockerignore`:

```bash
# Build cross-compiled binary
cargo zigbuild --release --target x86_64-unknown-linux-musl -p pet_store

# Stage it where Docker can see it
mkdir -p build_artifacts
cp target/x86_64-unknown-linux-musl/release/pet_store build_artifacts/

# Docker can now copy it
docker build -f dockerfiles/Dockerfile.test -t brrtrouter-petstore:e2e .
```

## Where It's Used

### 1. Tilt (Local Development)

**File:** `Tiltfile` lines 56-101

```python
local_resource(
    'build-petstore',
    '''
    cargo zigbuild --release --target x86_64-unknown-linux-musl -p pet_store &&
    mkdir -p build_artifacts &&
    cp target/x86_64-unknown-linux-musl/release/pet_store build_artifacts/
    ''',
    deps=['examples/pet_store/src/', 'examples/pet_store/Cargo.toml'],
)
```

**File:** `dockerfiles/Dockerfile.dev`
```dockerfile
COPY build_artifacts/pet_store /pet_store
```

### 2. Curl Integration Tests

**File:** `tests/curl_harness.rs` lines 155-189 (STEP 4)

```rust
// Copy to staging area (same as Tilt workflow)
eprintln!("[4/5] Copying to staging area...");
std::fs::create_dir_all("build_artifacts")?;
std::fs::copy(
    "target/x86_64-unknown-linux-musl/release/pet_store",
    "build_artifacts/pet_store"
)?;
```

**File:** `dockerfiles/Dockerfile.test`
```dockerfile
COPY build_artifacts/pet_store /pet_store
```

## Complete Flow Diagrams

### Tilt Workflow
```
┌─────────────────────────────────────────────────────────────────┐
│ 1. Developer changes src/server/service.rs                      │
└────────────────────┬────────────────────────────────────────────┘
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│ 2. Tilt detects change → triggers 'build-petstore' resource     │
└────────────────────┬────────────────────────────────────────────┘
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│ 3. cargo zigbuild --target x86_64-unknown-linux-musl           │
│    Output: target/x86_64-unknown-linux-musl/release/pet_store  │
│    Time: 10-30s (incremental compilation)                       │
└────────────────────┬────────────────────────────────────────────┘
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│ 4. cp target/.../pet_store build_artifacts/                    │
│    (Staging step - critical for .dockerignore!)                 │
└────────────────────┬────────────────────────────────────────────┘
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│ 5. docker build -f dockerfiles/Dockerfile.dev                    │
│    COPY build_artifacts/pet_store /pet_store                   │
│    Time: <1s (just file copy, no compilation)                   │
└────────────────────┬────────────────────────────────────────────┘
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│ 6. Docker push to local registry                                │
└────────────────────┬────────────────────────────────────────────┘
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│ 7. Kubernetes pod restarts with new image                       │
│    Developer can test changes in ~20-40 seconds!                │
└─────────────────────────────────────────────────────────────────┘
```

### Curl Test Workflow
```
┌─────────────────────────────────────────────────────────────────┐
│ 1. Developer runs: just nt                                      │
└────────────────────┬────────────────────────────────────────────┘
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│ 2. First curl test calls curl_harness::base_url()              │
│    Triggers ensure_image_ready() (singleton - runs once)        │
└────────────────────┬────────────────────────────────────────────┘
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│ 3. cargo zigbuild --target x86_64-unknown-linux-musl -p pet    │
│    Output: target/x86_64-unknown-linux-musl/release/pet_store  │
│    Time: 10-30s (incremental compilation)                       │
└────────────────────┬────────────────────────────────────────────┘
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│ 4. std::fs::copy(target/.../pet_store, build_artifacts/)       │
│    (Staging step - critical for .dockerignore!)                 │
└────────────────────┬────────────────────────────────────────────┘
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│ 5. docker build -f dockerfiles/Dockerfile.test -t brrtrouter-petstore:e2e  │
│    COPY build_artifacts/pet_store /pet_store                   │
│    Time: <1s (just file copy, no compilation)                   │
└────────────────────┬────────────────────────────────────────────┘
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│ 6. Start Docker container, run all 6 curl tests                │
│    Container cleanup on exit (atexit + SIGINT handlers)         │
│    All tests run against CURRENT code!                          │
└─────────────────────────────────────────────────────────────────┘
```

## Benefits

| Aspect | Without Staging (Compile in Docker) | With Staging (Local Build) |
|--------|--------------------------------------|----------------------------|
| **First build** | 5-10 minutes | 2-3 minutes |
| **Rebuild after change** | 2-3 minutes | 10-30 seconds |
| **Docker image build** | Part of above | <1 second |
| **Cargo cache** | Lost between builds | Preserved on host |
| **Staleness risk** | High (manual rebuild) | Zero (auto-builds) |
| **Cross-compile** | Complex in Docker | Simple with zig |

## Critical Rules for Future Contributors/AI

### ✅ DO

- **DO** use the staging area pattern for local development
- **DO** copy binaries to `build_artifacts/` before Docker build
- **DO** use `cargo zigbuild` for cross-compilation
- **DO** reference this pattern when adding new services

### ❌ DO NOT

- **DO NOT** try to `COPY target/...` in dockerfiles/Dockerfiles (blocked by .dockerignore)
- **DO NOT** modify `.dockerignore` to allow `target/*` (kills performance)
- **DO NOT** compile in Docker (too slow for iteration)
- **DO NOT** remove the staging step (breaks Docker builds)
- **DO NOT** forget cross-compilation target (macOS → Linux)

## Troubleshooting

### Error: "not found" when building Docker image

```
ERROR: failed to compute cache key: 
"/target/x86_64-unknown-linux-musl/release/pet_store": not found
```

**Cause:** Trying to copy from `target/` which is blocked by `.dockerignore`

**Solution:** Add staging step:
```bash
mkdir -p build_artifacts
cp target/x86_64-unknown-linux-musl/release/pet_store build_artifacts/
```

### Error: "exec format error" in container

```
exec /pet_store: exec format error
```

**Cause:** Binary compiled for wrong architecture (e.g., ARM64 macOS instead of x86_64 Linux)

**Solution:** Use cross-compilation:
```bash
cargo zigbuild --release --target x86_64-unknown-linux-musl -p pet_store
```

### Docker build is slow (minutes)

**Cause:** Building Rust code inside Docker

**Solution:** Build locally first:
1. `cargo zigbuild` outside Docker
2. Stage to `build_artifacts/`
3. Docker just copies (< 1s)

## Files to Review

| File | Purpose | Lines |
|------|---------|-------|
| `tests/curl_harness.rs` | Curl test auto-build | 112-213 (STEPS 2-5) |
| `dockerfiles/Dockerfile.test` | Test image | 1-68 (full file) |
| `Tiltfile` | Tilt workflow | 56-101 |
| `dockerfiles/Dockerfile.dev` | Tilt image | Check COPY line |
| `.dockerignore` | Performance rules | Lines 3-4 |
| `justfile` | Manual build | `build-test-image` recipe |

## History & Evolution

1. **Original:** Compiled in Docker (slow, 5-10 min)
2. **First attempt:** Pre-build requirement (forgot to rebuild → stale code)
3. **Freshness checks:** Complex timestamp logic (too much friction)
4. **Auto-build attempt:** Forgot about .dockerignore (build failed)
5. **Final solution:** Staging area pattern (fast, automatic, works!)

## Summary

The staging area pattern is a **critical architectural decision** that enables:
- ⚡ Fast iteration (10-30s vs 5-10min)
- ✅ Always current code
- 🔄 Consistency between Tilt and tests
- 🎯 Zero friction for developers

**For AI/future contributors:** This pattern is intentional, battle-tested, and documented in multiple places. Do not remove or "optimize" it without understanding the `.dockerignore` constraints!

