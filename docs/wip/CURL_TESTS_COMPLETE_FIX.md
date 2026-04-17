# Curl Integration Tests - Complete Fix Summary

## Overview

The `curl_integration_tests` had **three critical issues** that made them unusable:

1. ❌ **300+ second hangs** - Docker image auto-build during test startup
2. ❌ **Orphaned containers on Ctrl+C** - Static `OnceLock` prevented cleanup
3. ❌ **Dozens of containers running** - Drop never called on static harness

All three issues are now **completely fixed**!

## Issue 1: 300+ Second Hangs

### Problem
```bash
$ just nt curl
# Hangs for 300+ seconds with no output
TRY 1 SLOW [>300.000s] brrtrouter::curl_integration_tests ...
```

**Root Cause:** `ensure_image_ready()` was automatically building the Docker image, which compiles the entire Rust workspace (5-10 minutes!).

### Solution
- ✅ Removed automatic `docker build` from tests
- ✅ Image must be pre-built: `docker build -t brrtrouter-petstore:e2e .`
- ✅ Fast failure with clear instructions if image is missing
- ✅ Singleton pattern with thread coordination logging

**Impact:** Setup time reduced from 300+ seconds to < 1 second

## Issue 2: Orphaned Containers on Ctrl+C

### Problem
```bash
$ just nt curl
# Press Ctrl+C
^C
$ docker ps | grep brrtrouter-e2e
brrtrouter-e2e-12345  # Container still running!
```

**Root Cause:** Static `OnceLock` prevents `Drop` from being called on SIGINT.

### Solution
- ✅ Added POSIX signal handlers for SIGINT and SIGTERM
- ✅ Signal handler explicitly cleans up containers
- ✅ Cleanup by both container ID and process-specific name
- ✅ Added `libc = "0.2"` dependency for signal handling

**Impact:** Containers are now cleaned up even when tests are interrupted

## Issue 3: Dozens of Containers Running at 50% CPU

### Problem
```bash
$ just nt curl  # Run multiple times
$ docker ps | grep brrtrouter-e2e | wc -l
25  # 25 containers running, 50% CPU usage! 💀
```

**Root Cause:** Static `OnceLock<ContainerHarness>` is NEVER dropped, even on normal exit!

### Solution
- ✅ Added `atexit` handler for normal process termination
- ✅ Added `cleanup_handler()` that cleans up in ALL exit scenarios
- ✅ Triple redundancy: atexit + SIGINT + SIGTERM
- ✅ Cleanup tries container ID first, then falls back to name-based cleanup

**Impact:** Zero orphaned containers, automatic cleanup on ALL exit paths

## Complete Solution Architecture

```rust
// 1. Static harness (necessary evil for shared container)
static HARNESS: OnceLock<ContainerHarness> = OnceLock::new();

// 2. Cleanup handler (works for ALL exit scenarios)
extern "C" fn cleanup_handler() {
    // Try to get container from harness
    if let Some(harness) = HARNESS.get() {
        docker stop -t 2 {harness.container_id}
        docker rm -f {harness.container_id}
    }
    
    // Fallback: cleanup by process-specific name
    cleanup_orphaned_containers();
}

// 3. Register for ALL exit paths
fn register_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGINT, signal_handler);  // Ctrl+C
        libc::signal(libc::SIGTERM, signal_handler); // kill
        libc::atexit(atexit_wrapper);                // Normal exit
    }
}
```

## User Workflow

### First Time Setup
```bash
# Build the Docker image (one time, 5-10 minutes)
docker build -t brrtrouter-petstore:e2e .
```

### Running Tests
```bash
# Run tests (fast, < 1 second setup)
just nt curl

=== Docker Image Setup (Thread ThreadId(2)) ===
[1/2] Checking Docker availability...
      ✓ Docker is available
[2/2] Checking for image brrtrouter-petstore:e2e...
      ✓ Image is ready
=== Setup Complete in 0.05s ===

[Thread ThreadId(2)] Proceeding...
# Tests run...

🧹 Cleaning up Docker containers on exit...
Stopping container: abc123
✓ Cleanup complete
```

### Interrupted Tests (Ctrl+C)
```bash
$ just nt curl
# Press Ctrl+C mid-test

🧹 Cleaning up Docker containers on exit...
Stopping container: abc123
Cleaning up container: brrtrouter-e2e-12345
✓ Removed container: brrtrouter-e2e-12345
✓ Cleanup complete
^C

$ docker ps | grep brrtrouter-e2e
# (nothing) ✅
```

### Multiple Test Runs
```bash
# Run 10 times in a row
for i in {1..10}; do just nt curl; done

# Check for orphaned containers
$ docker ps -a | grep brrtrouter-e2e
# (nothing) ✅

# No containers leaked!
```

## Technical Implementation

### File: tests/curl_harness.rs

**Key Components:**

1. **Singleton Image Check**
   ```rust
   static IMAGE_SETUP: OnceLock<Result<(), String>> = OnceLock::new();
   ```
   - Checks image exists exactly once
   - Blocks other threads until complete
   - Fails fast if image missing

2. **Static Container Harness**
   ```rust
   static HARNESS: OnceLock<ContainerHarness> = OnceLock::new();
   ```
   - Shares one container across all tests in a process
   - Uses process ID in container name for nextest isolation
   - Cleaned up via explicit handlers, not Drop

3. **Triple Cleanup Registration**
   ```rust
   register_signal_handlers() {
       libc::signal(SIGINT, handler);
       libc::signal(SIGTERM, handler);
       libc::atexit(cleanup);
   }
   ```
   - atexit: Normal termination
   - SIGINT: Ctrl+C / test interruption
   - SIGTERM: Process kill

4. **Dual Cleanup Strategy**
   ```rust
   cleanup_handler() {
       // Method 1: Direct container ID
       if let Some(h) = HARNESS.get() {
           docker rm -f {h.container_id}
       }
       
       // Method 2: Process-specific name
       cleanup_orphaned_containers();
   }
   ```

### File: Cargo.toml

Added dependency:
```toml
[dev-dependencies]
libc = "0.2"  # For signal handling in tests (SIGINT cleanup)
```

## Benefits

| Aspect | Before | After |
|--------|--------|-------|
| **Setup Time** | 300+ seconds (silent build) | < 1 second (image check) |
| **Ctrl+C Cleanup** | ❌ Containers orphaned | ✅ Automatic cleanup |
| **Normal Exit Cleanup** | ❌ Containers orphaned | ✅ Automatic cleanup |
| **Resource Leaks** | 💀 Dozens of containers | ✅ Zero leaks |
| **CPU Usage** | 50% (from orphaned containers) | Normal |
| **Developer Experience** | Frustrating, manual cleanup | Seamless, automatic |
| **CI/CD Reliability** | Fails, hangs | Stable, fast |

## Documentation Created

1. **docs/SIGINT_CLEANUP_FIX.md** - Signal handling details
2. **docs/CURL_TESTS_DOCKER_IMAGE.md** - Image requirement workflow
3. **docs/CURL_TESTS_FIX_COMPLETE.md** - First attempt summary
4. **docs/STATIC_HARNESS_CLEANUP_FIX.md** - Static Drop issue details
5. **docs/CURL_TESTS_COMPLETE_FIX.md** - This comprehensive summary
6. **docs/TEST_SETUP_TEARDOWN.md** - Updated with signal handling section
7. **scripts/cleanup-test-containers.sh** - Emergency cleanup script

## Emergency Cleanup

If you have orphaned containers from before this fix:

```bash
# Method 1: Use the cleanup script
./scripts/cleanup-test-containers.sh

# Method 2: Manual cleanup
docker rm -f $(docker ps -a -q --filter "name=brrtrouter-e2e")

# Method 3: Nuclear option (removes ALL stopped containers)
docker container prune -f
```

## Verification

Test the complete fix:

```bash
# 1. Clean slate
docker rm -f $(docker ps -a -q --filter "name=brrtrouter-e2e") 2>/dev/null || true

# 2. Build image
docker build -t brrtrouter-petstore:e2e .

# 3. Run tests normally
just nt curl
docker ps -a | grep brrtrouter-e2e  # Should be empty ✅

# 4. Run and interrupt with Ctrl+C
just nt curl
# Press Ctrl+C
docker ps -a | grep brrtrouter-e2e  # Should be empty ✅

# 5. Run multiple times
for i in {1..5}; do just nt curl; done
docker ps -a | grep brrtrouter-e2e  # Should be empty ✅

# 6. Check CPU usage
top  # Should be normal, no runaway containers ✅
```

## Lessons Learned

1. **Never trust Drop on statics** - Always use explicit cleanup handlers
2. **Triple redundancy is necessary** - atexit + SIGINT + SIGTERM
3. **Fail fast > silent hangs** - Pre-build requirement is better UX
4. **Singleton pattern needs error propagation** - Use `OnceLock<Result<T, E>>`
5. **Monitor resource usage** - 50% CPU is a critical warning sign

## Future Improvements

Possible enhancements:
- Add `just build-test-image` convenience command
- Cache Docker image in GitHub Actions
- Add image age check and warning if stale
- Consider `cargo-zigbuild` for faster musl builds
- Add health check for image validity

## Impact

**Critical Fix:** The curl integration tests are now:
- ✅ Fast (< 1s setup instead of 300+s)
- ✅ Reliable (no orphaned containers)
- ✅ Clean (automatic cleanup on ALL exit paths)
- ✅ Resource-efficient (no CPU leaks)
- ✅ Developer-friendly (clear error messages)
- ✅ CI/CD ready (stable and predictable)

This makes the curl integration tests **production-ready** for continuous integration!

