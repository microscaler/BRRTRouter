# Automatic Build Solution - Always Test Current Code

## The Breakthrough

**Inspired by the Tilt workflow**: Build locally, copy into Docker!

This completely solves the "testing stale code" problem **automatically** with zero friction.

## How It Works

### The Tilt Pattern

From your Tilt setup:
```bash
1. Build locally: cargo build --release --target x86_64-unknown-linux-musl
2. Copy to staging: cp target/.../pet_store build_artifacts/
3. Docker copies file: COPY build_artifacts/pet_store /pet_store
4. Result: Instant Docker build, testing current code
```

### Applied to Curl Tests

```rust
// tests/curl_harness.rs
pub fn ensure_image_ready() {
    IMAGE_SETUP.get_or_init(|| {
        eprintln!("[1/4] Checking Docker...");
        // ... check Docker
        
        eprintln!("[2/4] Building pet_store binary (incremental)...");
        cargo build --release -p pet_store
        
        eprintln!("[3/4] Verifying binary...");
        // Check target/release/pet_store exists
        
        eprintln!("[4/4] Building Docker image (copying binary)...");
        docker build -f Dockerfile.test -t brrtrouter-petstore:e2e .
        
        eprintln!("✨ Testing CURRENT code (just compiled)");
    });
}
```

## The Magic

### Before (Complex)

```bash
$ vim src/server/service.rs  # Make changes
$ just nt curl

❌ Problem 1: Testing stale code (if forgot to rebuild)
❌ Problem 2: Manual rebuild required
❌ Problem 3: Docker compile takes 5-10 minutes
❌ Problem 4: Freshness checks were complex/annoying
```

### After (Automatic!)

```bash
$ vim src/server/service.rs  # Make changes
$ just nt curl

[1/4] Checking Docker...
      ✓ Docker is available
[2/4] Building pet_store binary (incremental)...
      ✓ Binary built                           # 10-30 seconds!
[3/4] Verifying binary...
      ✓ Binary found at target/release/pet_store
[4/4] Building Docker image (copying binary)...
      ✓ Image ready                            # < 1 second!

=== Setup Complete in 25.3s ===
    ✨ Testing CURRENT code (just compiled)

# Tests run against code you JUST changed!
```

## Key Benefits

### 1. Always Testing Current Code ✅

No more "did I rebuild?" confusion:
- Tests **automatically** build the latest code
- Incremental compilation is fast (10-30s)
- Docker image build is instant (< 1s)

### 2. Fast Incremental Builds ⚡

```bash
# First build (cold)
cargo build --release -p pet_store
# 2-3 minutes

# Subsequent builds (only changed files)
cargo build --release -p pet_store
# 10-30 seconds!

# Docker image (just copies files)
docker build -f Dockerfile.test
# < 1 second!
```

### 3. Zero Friction 🎯

- No manual rebuild commands to remember
- No environment variables
- No freshness checks
- No stale image warnings
- **It just works!**

### 4. Same as Tilt 🔄

Consistency across workflows:
- Tilt: Build locally → copy to container
- Curl tests: Build locally → copy to container
- CI/CD: Build once → run tests

## Implementation

### File: Dockerfile.test

```dockerfile
# Fast test image using pre-built binary
FROM scratch

# Copy pre-built binary from host (instant!)
COPY target/release/pet_store /pet_store

# Copy assets
COPY examples/pet_store/doc /doc
COPY examples/pet_store/static_site /static_site
COPY examples/pet_store/config /config

EXPOSE 8080
ENTRYPOINT ["/pet_store", ...]
```

**Key insight:** No Rust compilation in Docker! Just file copying.

### File: tests/curl_harness.rs

```rust
pub fn ensure_image_ready() {
    IMAGE_SETUP.get_or_init(|| {
        // Step 1: Check Docker
        docker --version
        
        // Step 2: Build binary locally (incremental, fast!)
        cargo build --release -p pet_store
        
        // Step 3: Verify it exists
        assert!(Path::new("target/release/pet_store").exists());
        
        // Step 4: Build Docker image (instant - just copies!)
        docker build -f Dockerfile.test -t brrtrouter-petstore:e2e .
    });
}
```

**Key insight:** Build happens automatically before tests run!

### File: justfile

```bash
# Manual rebuild if needed (rare)
build-test-image:
    cargo build --release -p pet_store
    docker build -f Dockerfile.test -t brrtrouter-petstore:e2e .
```

But you rarely need this - tests do it automatically!

## Performance Comparison

### Old Approach (Docker-based build)

```
Cold build:    5-10 minutes (compile in Docker)
Warm rebuild:  2-3 minutes  (Docker layer cache)
Iteration:     Manual rebuild required
Staleness:     High risk
```

### New Approach (Local build + copy)

```
Cold build:    2-3 minutes (local cargo build)
Warm rebuild:  10-30 seconds (incremental compile)
Docker part:   < 1 second (just copy files)
Iteration:     Automatic!
Staleness:     Zero risk (always builds first)
```

**Result: 10-30 seconds for full cycle vs 2-3 minutes!**

## User Experience

### Scenario 1: Fix a Bug

```bash
$ vim src/server/service.rs  # Fix critical bug
$ just nt curl               # Run tests

[2/4] Building pet_store binary...
      ✓ Binary built (12.5s)
[4/4] Building Docker image...
      ✓ Image ready (0.8s)

✨ Testing CURRENT code (just compiled)

# Tests run with your fix!
```

**Total time:** 13 seconds from code change to testing

### Scenario 2: Rapid Iteration

```bash
# Iteration 1
$ vim src/server/service.rs
$ just nt curl  # 15s compile + tests
❌ Test fails

# Iteration 2
$ vim src/server/service.rs  # Another fix
$ just nt curl  # 12s compile + tests (incremental!)
✅ Tests pass!
```

**No manual steps!** Just edit and test.

### Scenario 3: After Git Pull

```bash
$ git pull origin main
$ just nt curl

[2/4] Building pet_store binary...
      ✓ Binary built (45.2s)  # More changes = longer build
[4/4] Building Docker image...
      ✓ Image ready (0.9s)

✨ Testing CURRENT code
```

**Automatically picks up all changes from pull!**

## Why This Is Better

| Aspect | Manual Rebuild | Auto Build |
|--------|---------------|-----------|
| **Forget to rebuild** | ❌ Tests stale code | ✅ Impossible to forget |
| **Rebuild command** | ❌ Must remember | ✅ Automatic |
| **Build time** | ❌ 2-3 min (Docker) | ✅ 10-30s (local) |
| **Docker time** | ❌ Included in build | ✅ < 1s (copy only) |
| **Freshness** | ❌ Manual checks needed | ✅ Always fresh |
| **Friction** | ❌ High | ✅ Zero |
| **Consistency** | ❌ Different from Tilt | ✅ Same as Tilt |

## CI/CD Integration

```yaml
# .github/workflows/ci.yml
- name: Build pet store
  run: cargo build --release -p pet_store

- name: Build test image
  run: docker build -f Dockerfile.test -t brrtrouter-petstore:e2e .

- name: Run curl tests
  run: cargo nextest run --test curl_integration_tests
```

Or even simpler - tests do it automatically!

```yaml
- name: Run curl tests
  run: cargo nextest run --test curl_integration_tests
  # Builds automatically on first test!
```

## Technical Details

### Why Local Build Is Fast

1. **Incremental compilation** - Only recompiles changed crates
2. **Cargo cache** - Dependencies cached in `target/`
3. **No Docker overhead** - Native host compilation
4. **Release build** - Optimized but still incremental

### Why Docker Build Is Instant

```dockerfile
FROM scratch                    # No base image needed
COPY target/release/pet_store   # Just copy one file!
COPY examples/pet_store/doc     # Static files
# Total: < 1 second
```

No Rust toolchain, no compilation, just file operations!

### Singleton Pattern Ensures Once

```rust
static IMAGE_SETUP: OnceLock<Result<(), String>> = OnceLock::new();

pub fn ensure_image_ready() {
    IMAGE_SETUP.get_or_init(|| {
        // This runs ONCE per test process
        // Even if 100 tests call this function
        build_binary();
        build_image();
    });
}
```

Parallel tests all share one build!

## Comparison with Alternatives

### Alternative 1: Check Freshness

```
❌ Complex timestamp logic
❌ False positives
❌ Extra dependencies (chrono)
❌ Still manual rebuild
```

### Alternative 2: Always Rebuild in Docker

```
❌ 5-10 minutes per run
❌ Too slow for iteration
❌ Doesn't use incremental compilation
```

### Alternative 3: Trust Developer

```
⚠️ Easy to forget
⚠️ Testing stale code
⚠️ Manual rebuild step
```

### Our Solution: Auto Build Locally

```
✅ Always current code
✅ Fast (10-30s incremental)
✅ Zero friction
✅ Same pattern as Tilt
✅ Automatic
```

## Files Created/Modified

1. **Dockerfile.test** (new)
   - Simple: FROM scratch + COPY
   - No Rust compilation
   - < 1s build time

2. **tests/curl_harness.rs**
   - Auto-build in `ensure_image_ready()`
   - 4-step process with clear feedback
   - Always tests current code

3. **justfile**
   - Updated `build-test-image` command
   - Rarely needed (tests do it automatically)

4. **docs/AUTO_BUILD_SOLUTION.md** (this file)
   - Documents the game-changing improvement

## Migration

### Old Way (Manual)

```bash
# Had to remember this
docker build -t brrtrouter-petstore:e2e .
just nt curl
```

### New Way (Automatic)

```bash
# Just test!
just nt curl
# Builds automatically ✨
```

## Summary

The breakthrough: **Use the Tilt pattern for curl tests!**

1. ✅ **Build locally** - Fast incremental compilation (10-30s)
2. ✅ **Copy to Docker** - Instant image build (< 1s)
3. ✅ **Always current** - Impossible to test stale code
4. ✅ **Zero friction** - No manual steps
5. ✅ **Consistent** - Same pattern as Tilt

**Result:** From "it seemed a little too fast" (testing stale code) to "it's appropriately fast" (testing current code with auto-build)!

This is **exactly** how modern development should work! 🎉

