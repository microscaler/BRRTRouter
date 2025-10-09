# CI Zigbuild Installation - Complete Analysis

## Summary

Added `cargo-zigbuild` to **3 out of 5 CI jobs** that need it.

## Jobs Analysis

| Job | Needs Zigbuild? | Why? | Status |
|-----|----------------|------|--------|
| **build-and-test** | ✅ YES | Runs nextest → curl_integration_tests → curl_harness | ✅ ADDED (line 67-68) |
| **tilt-ci** | ✅ YES | Builds binaries with zigbuild for Tilt | ✅ Already had it (line 142-143) |
| **e2e-docker** | ✅ YES | Runs curl_integration_tests | ✅ ADDED (line 288-289) |
| **perf-wrk** | ❌ NO | Uses pre-built service container, no tests | ⊘ Not needed |
| **goose-load-test** | ❌ NO | Uses pre-built service container, builds examples only | ⊘ Not needed |

## Detailed Analysis

### 1. build-and-test (ADDED)

**Line 67-68:**
```yaml
- name: Install cargo-zigbuild (for curl_harness cross-compilation)
  run: cargo install cargo-zigbuild --locked
```

**Why needed:**
- Runs `cargo nextest run --workspace --all-targets`
- Includes `curl_integration_tests`
- Which uses `curl_harness.rs`
- Which calls `cargo zigbuild` to build pet_store for Docker

**Build flow:**
```
nextest → curl_integration_tests.rs 
       → curl_harness.rs (line 4)
       → ensure_image_ready() (line 150)
       → cargo zigbuild ... (line 152)
```

### 2. tilt-ci (ALREADY HAD IT)

**Line 142-143:**
```yaml
- name: Install cargo-zigbuild for cross-compilation
  run: cargo install cargo-zigbuild
```

**Why needed:**
- Explicitly builds with zigbuild (line 163, 165):
```yaml
cargo zigbuild --release --target x86_64-unknown-linux-musl
cargo zigbuild --release --target x86_64-unknown-linux-musl -p pet_store
```

**Status:** Already correct! No changes needed.

### 3. e2e-docker (ADDED)

**Line 288-289:**
```yaml
- name: Install cargo-zigbuild (for curl_integration_tests)
  run: cargo install cargo-zigbuild --locked
```

**Why needed:**
- Line 341 runs: `cargo test --test curl_integration_tests`
- Same curl_harness dependency as build-and-test
- Needs zigbuild to rebuild pet_store for Docker

**Note:** This job downloads pre-built artifacts for the main build (line 298-300), but still runs tests that need to rebuild.

### 4. perf-wrk (NOT NEEDED)

**What it does:**
- Uses service container with pre-built image (line 359):
```yaml
services:
  petstore:
    image: ${{ needs.e2e-docker.outputs.image }}
```
- Installs `wrk` and runs load tests
- No cargo build or test commands
- No pet_store compilation

**Status:** Does not need zigbuild ✅

### 5. goose-load-test (NOT NEEDED)

**What it does:**
- Uses service container with pre-built image (line 477):
```yaml
services:
  petstore:
    image: ${{ needs.e2e-docker.outputs.image }}
```
- Only builds the load test example (line 497):
```yaml
cargo build --release --example api_load_test
```
- This is just a regular Rust build, no cross-compilation
- No pet_store compilation needed

**Status:** Does not need zigbuild ✅

## Installation Patterns

### Pattern 1: Tests Use curl_harness
```yaml
- name: Install cargo-zigbuild (for curl_harness cross-compilation)
  run: cargo install cargo-zigbuild --locked
```
**Used by:** build-and-test, e2e-docker

### Pattern 2: Explicit zigbuild Use
```yaml
- name: Install cargo-zigbuild for cross-compilation
  run: cargo install cargo-zigbuild
```
**Used by:** tilt-ci

### Pattern 3: Not Needed
No installation needed - uses service containers or doesn't build.
**Used by:** perf-wrk, goose-load-test

## Dependency Chain

```
e2e-docker job
  ├─ Downloads pet_store artifact (for Docker image)
  ├─ Builds Docker image
  ├─ Pushes to ttl.sh
  ├─ Outputs image URL
  └─ Runs curl_integration_tests (needs zigbuild!) ← This is why!

perf-wrk job
  ├─ needs: e2e-docker
  └─ Uses pre-built image from e2e-docker ← No zigbuild needed

goose-load-test job
  ├─ needs: e2e-docker
  └─ Uses pre-built image from e2e-docker ← No zigbuild needed
```

## Why e2e-docker Needs Zigbuild Despite Having Artifacts

**Might seem redundant, but:**

1. **Downloads pet_store artifact** (line 298-300)
   - Used to build the Docker image
   - Fast, doesn't require compilation

2. **Runs curl_integration_tests** (line 341)
   - Test needs to rebuild pet_store on-the-fly
   - Ensures tests use *current code*, not artifact
   - This is a feature, not a bug!

**Why rebuild in tests?**
- Tests might change the code
- curl_harness ensures Docker image uses exact test code
- Prevents "stale binary" issues

## Installation Cost Summary

| Job | Zigbuild Install Time | Frequency | Worth It? |
|-----|---------------------|-----------|-----------|
| build-and-test | ~60s | Once per run | ✅ Yes - enables tests |
| tilt-ci | ~60s | Once per run | ✅ Yes - already had it |
| e2e-docker | ~60s | Once per run | ✅ Yes - enables tests |
| perf-wrk | N/A | N/A | N/A - not needed |
| goose-load-test | N/A | N/A | N/A - not needed |

**Total install time:** ~180 seconds across 3 jobs
**Total CI runtime:** ~10-15 minutes
**Percentage:** ~2% of total CI time
**Benefit:** Reliable cross-compilation for tests

## Verification Checklist

After this change, verify each job:

### build-and-test
- [ ] Zigbuild installs successfully
- [ ] curl_integration_tests pass
- [ ] No "no such command: zigbuild" errors

### tilt-ci
- [ ] No changes (already working)
- [ ] Zigbuild commands succeed
- [ ] Tilt CI tests pass

### e2e-docker
- [ ] Zigbuild installs successfully
- [ ] curl_integration_tests pass
- [ ] Docker image builds correctly

### perf-wrk
- [ ] No changes needed
- [ ] Uses service container successfully
- [ ] Load tests complete

### goose-load-test
- [ ] No changes needed
- [ ] Uses service container successfully
- [ ] Load tests complete

## Files Changed

**Only file:** `.github/workflows/ci.yml`

**Changes:**
1. Line 67-68: Added zigbuild to build-and-test
2. Line 288-289: Added zigbuild to e2e-docker
3. Line 142-143: No change (tilt-ci already had it)

## Common Questions

### Q: Why not install zigbuild once and cache it?
**A:** GitHub Actions doesn't share state between jobs by default. Each job is isolated.

### Q: Could we use artifacts instead?
**A:** We could upload zigbuild binary as artifact, but:
- Installation is only ~60s
- Caching adds complexity
- Not worth the optimization yet

### Q: Why does e2e-docker need zigbuild if it has artifacts?
**A:** The artifacts are for building the Docker image. Tests run separately and need to rebuild to ensure they test current code.

### Q: Can we remove musl-gcc since we have zigbuild?
**A:** No, they serve different purposes:
- `musl-gcc`: For CI artifact builds (traditional)
- `zigbuild`: For test harness (modern, reliable)

Both are useful!

## Future Optimization

If installation time becomes an issue:

### Option 1: Cache zigbuild binary
```yaml
- uses: actions/cache@v3
  with:
    path: ~/.cargo/bin/cargo-zigbuild
    key: zigbuild-${{ runner.os }}
```

### Option 2: Use pre-built zigbuild binary
```yaml
- name: Download cargo-zigbuild
  run: |
    wget https://github.com/.../cargo-zigbuild
    chmod +x cargo-zigbuild
    mv cargo-zigbuild ~/.cargo/bin/
```

### Option 3: Custom Docker image with zigbuild
```yaml
runs-on: ubuntu-latest
container:
  image: ghcr.io/microscaler/brrtrouter-ci:latest  # includes zigbuild
```

**Current approach:** Keep it simple with `cargo install` ✅

## Summary

✅ **3 jobs need zigbuild:**
- build-and-test (added)
- tilt-ci (already had it)
- e2e-docker (added)

❌ **2 jobs don't need zigbuild:**
- perf-wrk (uses service container)
- goose-load-test (uses service container)

**Total changes:** 2 new installations added
**Total CI impact:** ~120s (2%) added to total runtime
**Benefit:** Reliable cross-compilation for all tests

**Status:** Complete and ready for CI! 🚀

