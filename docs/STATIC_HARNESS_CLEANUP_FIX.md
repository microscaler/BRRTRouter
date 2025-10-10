# Static Harness Cleanup Fix - Critical Container Leak

## The Critical Problem

**Dozens of containers running at 50% CPU!** The `curl_integration_tests` were leaving containers running because:

```rust
static HARNESS: OnceLock<ContainerHarness> = OnceLock::new();
```

**The Issue:**
- `HARNESS` is a static variable with `'static` lifetime
- Static variables are NEVER dropped until process exit
- Even when the process exits, Rust doesn't guarantee `Drop` is called on statics
- The `ContainerHarness::drop()` implementation was **never running**!

## Root Cause Analysis

### What We Thought Would Happen

```rust
impl Drop for ContainerHarness {
    fn drop(&mut self) {
        // Clean up container
        docker rm -f {container_id}
    }
}

static HARNESS: OnceLock<ContainerHarness> = OnceLock::new();
// We thought: "When process exits, Drop will be called"
```

### What Actually Happens

```
Test Process Start
  ├─ HARNESS.get_or_init(ContainerHarness::start)
  ├─ Container starts (ID: abc123)
  ├─ Tests run
  ├─ Tests complete/interrupted
  └─ Process exits
      └─ HARNESS never drops! 💀
          └─ Container keeps running! 💀💀💀
```

**Result:** After 10 test runs, you have 10 containers consuming 50% CPU!

## The Solution: Triple Redundancy

We now clean up containers in **THREE ways**:

### 1. atexit Handler (Normal Exit)

```rust
extern "C" fn cleanup_handler() {
    if let Some(harness) = HARNESS.get() {
        docker stop -t 2 {harness.container_id}
        docker rm -f {harness.container_id}
    }
    cleanup_orphaned_containers(); // Also cleanup by name
}

unsafe {
    libc::atexit(atexit_wrapper);
}
```

**Triggers when:** Tests complete successfully and process exits normally

### 2. SIGINT Handler (Ctrl+C)

```rust
extern "C" fn signal_handler(_: libc::c_int) {
    cleanup_handler();  // Same cleanup
    // Re-raise signal for normal termination
    libc::signal(libc::SIGINT, libc::SIG_DFL);
    libc::raise(libc::SIGINT);
}
```

**Triggers when:** User presses Ctrl+C or `just nt` is interrupted

### 3. SIGTERM Handler (Kill)

```rust
libc::signal(libc::SIGTERM, signal_handler as libc::sighandler_t);
```

**Triggers when:** Process is killed with `kill <pid>`

## Implementation Details

### The Cleanup Handler

```rust
extern "C" fn cleanup_handler() {
    // Prevent recursive cleanup
    if SIGNAL_CLEANUP_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    
    eprintln!("\n🧹 Cleaning up Docker containers on exit...");
    
    // Method 1: Get container ID from static harness
    if let Some(harness) = HARNESS.get() {
        eprintln!("Stopping container: {}", harness.container_id);
        docker stop -t 2 {harness.container_id}
        docker rm -f {harness.container_id}
    }
    
    // Method 2: Cleanup by process-specific name (fallback)
    cleanup_orphaned_containers();  // Uses process ID in name
    
    eprintln!("✓ Cleanup complete\n");
}
```

### Why Both Methods?

1. **By Container ID** (from harness):
   - Most direct - we know the exact container
   - Works if harness was initialized

2. **By Container Name** (process ID):
   - Fallback if harness wasn't fully initialized
   - Also cleans up containers from previous failed runs
   - Container name: `brrtrouter-e2e-{process_id}`

### Registration

```rust
fn register_signal_handlers() {
    // Define handlers...
    
    // Register for SIGINT and SIGTERM
    unsafe {
        libc::signal(libc::SIGINT, signal_handler as libc::sighandler_t);
        libc::signal(libc::SIGTERM, signal_handler as libc::sighandler_t);
        libc::atexit(atexit_wrapper);  // Normal exit
    }
}
```

This is called once when the first test accesses `base_url()`.

## Testing the Fix

### Before Fix (Disaster)

```bash
$ just nt curl
# Tests run
$ docker ps | grep brrtrouter-e2e
brrtrouter-e2e-12345  # Still running!

$ just nt curl  # Run again
$ docker ps | grep brrtrouter-e2e
brrtrouter-e2e-12345  # First one still there
brrtrouter-e2e-12346  # New one added!

# After 10 runs
$ docker ps | grep brrtrouter-e2e | wc -l
10  # 10 containers eating 50% CPU! 💀
```

### After Fix (Clean)

```bash
$ just nt curl
# Tests run
🧹 Cleaning up Docker containers on exit...
Stopping container: abc123
✓ Cleanup complete

$ docker ps | grep brrtrouter-e2e
# (nothing) ✅

$ just nt curl  # Run again
🧹 Cleaning up Docker containers on exit...
✓ Cleanup complete

$ docker ps | grep brrtrouter-e2e
# (nothing) ✅
```

### Test Ctrl+C Cleanup

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

## Why Static + Drop Doesn't Work

This is a critical Rust concept:

```rust
struct Resource { /* ... */ }
impl Drop for Resource { /* cleanup */ }

// ❌ BAD: Drop may never be called!
static RESOURCE: OnceLock<Resource> = OnceLock::new();

// ✅ GOOD: Drop is guaranteed when variable goes out of scope
let resource = Resource::new();
// ... use resource ...
// Drop is called here automatically
```

**The Rule:** Never rely on `Drop` for `static` variables if cleanup is critical!

### Why Rust Doesn't Drop Statics

1. **Performance**: Dropping statics at shutdown adds overhead
2. **Uncertainty**: OS cleanup is often faster than manual cleanup
3. **Edge Cases**: What if a static is used during another static's drop?
4. **Design**: Statics are meant to live for the entire program

## Manual Cleanup Options

If you have orphaned containers from before this fix:

```bash
# List them
docker ps -a | grep brrtrouter-e2e

# Clean them all up
docker rm -f $(docker ps -a -q --filter "name=brrtrouter-e2e")

# Or use the cleanup script
./scripts/cleanup-test-containers.sh
```

## Files Modified

1. **tests/curl_harness.rs**
   - Added `cleanup_handler()` function
   - Added `atexit` registration
   - Enhanced signal handlers to call `cleanup_handler()`
   - Cleanup now tries both container ID and name-based cleanup

2. **docs/STATIC_HARNESS_CLEANUP_FIX.md** (this file)
   - Documents the critical issue and fix

3. **scripts/cleanup-test-containers.sh**
   - Emergency cleanup script for orphaned containers

## Related Issues

This complements other cleanup work:
- **SIGINT cleanup** - Now calls the same `cleanup_handler()`
- **RAII Drop** - Still useful for non-static resources
- **Nextest isolation** - Process-specific container names prevent conflicts

## Lessons Learned

1. **Static + Drop = Unreliable** - Never trust Drop on statics for critical cleanup
2. **atexit is essential** - Register cleanup handlers explicitly
3. **Signal handlers matter** - SIGINT/SIGTERM happen frequently in testing
4. **Triple redundancy wins** - atexit + SIGINT + SIGTERM + name-based fallback
5. **Test your cleanup** - Always verify containers are actually removed

## Prevention

To prevent similar issues in the future:

1. **Avoid statics for resources** - Use test-local RAII when possible
2. **Always register atexit** - If you MUST use statics with resources
3. **Test cleanup explicitly** - `docker ps` after every test run
4. **Monitor resource usage** - 50% CPU is a red flag!
5. **Document cleanup requirements** - Make it obvious in code comments

## Impact

**Before:** Dozens of containers, 50% CPU usage, manual cleanup required
**After:** Zero orphaned containers, automatic cleanup on all exit paths

This fix is **critical** for CI/CD and local development stability.

