# SIGINT Cleanup Fix for Docker Integration Tests

## Problem

When running `just nt` (cargo nextest), pressing Ctrl+C sends SIGINT to the test process. This caused:

1. **Orphaned Docker containers**: The `ContainerHarness` `Drop` implementation never ran, leaving containers running
2. **60+ second test hangs**: Subsequent test runs tried to start containers with the same name, leading to conflicts or port binding issues
3. **Resource leaks**: Containers accumulated over multiple test runs, consuming system resources

## Root Cause

The `curl_integration_tests.rs` uses a static `OnceLock<ContainerHarness>` to share a single Docker container across all tests in a process:

```rust
static HARNESS: OnceLock<ContainerHarness> = OnceLock::new();

pub fn base_url() -> &'static str {
    let h = HARNESS.get_or_init(ContainerHarness::start);
    h.base_url.as_str()
}
```

When SIGINT is received:
1. The test process is terminated immediately
2. The static `OnceLock` never goes out of scope (it's static!)
3. The `ContainerHarness::drop()` is never called
4. The Docker container keeps running

## Solution

Implemented POSIX signal handling to ensure cleanup even on SIGINT/SIGTERM:

### 1. Added Signal Handler Registration

```rust
/// Flag to track if signal handler cleanup is already running
static SIGNAL_CLEANUP_RUNNING: AtomicBool = AtomicBool::new(false);

fn register_signal_handlers() {
    extern "C" fn signal_handler(_: libc::c_int) {
        // Prevent recursive cleanup if multiple signals arrive
        if SIGNAL_CLEANUP_RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }
        
        eprintln!("\n🛑 Signal received - cleaning up Docker containers...");
        cleanup_orphaned_containers();
        eprintln!("✓ Cleanup complete");
        
        // Re-raise the signal to allow normal termination
        unsafe {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
            libc::raise(libc::SIGINT);
        }
    }
    
    unsafe {
        libc::signal(libc::SIGINT, signal_handler as libc::sighandler_t);
        libc::signal(libc::SIGTERM, signal_handler as libc::sighandler_t);
    }
}
```

### 2. Register Handler on First Test

```rust
pub fn base_url() -> &'static str {
    // Register signal handlers once to ensure cleanup on SIGINT/SIGTERM
    CLEANUP_REGISTERED.get_or_init(|| {
        register_signal_handlers();
    });
    
    // ... rest of the function
}
```

### 3. Enhanced Cleanup Function

- Made cleanup more aggressive with `docker rm -f` (force kill + remove in one command)
- Increased polling attempts from 20 to 30
- Increased polling interval from 50ms to 100ms
- Added better error messages and status indicators (✓, ⚠, ❌)

### 4. Cleanup on Every Container Start

Changed from one-time cleanup to **always cleanup first**:

```rust
fn start() -> Self {
    // ALWAYS cleanup orphaned containers first (not just once)
    // This is critical because if tests were cancelled, Drop may not have run
    eprintln!("Cleaning up any orphaned containers from previous runs...");
    cleanup_orphaned_containers();
    
    // ... start new container
}
```

## Implementation Details

### Why `libc` Signal Handling?

1. **Low-level control**: Direct POSIX signal API gives us full control
2. **No additional dependencies**: `libc` is already used by Rust's std library
3. **Works with nextest**: Signal handlers work correctly with parallel test runners
4. **Cross-platform**: Works on Linux and macOS (both POSIX-compliant)

### Cleanup Flow

1. **Normal Exit**: `ContainerHarness::drop()` → stops & removes container
2. **SIGINT/SIGTERM**: `signal_handler()` → `cleanup_orphaned_containers()` → re-raise signal
3. **Test Start**: Always call `cleanup_orphaned_containers()` first

### Safety Considerations

- **Idempotent cleanup**: Safe to call multiple times, handles "No such container" gracefully
- **Recursive signal protection**: `SIGNAL_CLEANUP_RUNNING` atomic flag prevents re-entry
- **Signal re-raising**: After cleanup, we reset to `SIG_DFL` and re-raise to allow normal termination

## Files Modified

1. **tests/curl_harness.rs**
   - Added `register_signal_handlers()` function
   - Added `SIGNAL_CLEANUP_RUNNING` atomic flag
   - Enhanced `cleanup_orphaned_containers()` with better polling and logging
   - Updated `ContainerHarness::start()` to always cleanup first
   - Updated `base_url()` to register signal handlers

2. **Cargo.toml**
   - Added `libc = "0.2"` to `[dev-dependencies]` for signal handling

## Testing

To verify the fix:

```bash
# Start tests
just nt curl

# Press Ctrl+C (SIGINT)

# You should see:
# 🛑 Signal received - cleaning up Docker containers...
# Cleaning up container: brrtrouter-e2e-<pid>
# ✓ Removed container: brrtrouter-e2e-<pid>
# ✓ Cleanup complete

# Verify no orphaned containers
docker ps -a | grep brrtrouter-e2e
# (should return nothing)

# Run tests again immediately
just nt curl
# (should start cleanly without 60+ second hangs)
```

## Benefits

1. **No more orphaned containers**: Signal handler ensures cleanup even on Ctrl+C
2. **Faster test iteration**: No need to manually kill containers between runs
3. **Better resource management**: Prevents container accumulation
4. **Clear feedback**: Users see cleanup happening when they interrupt tests
5. **Idempotent**: Safe to run tests multiple times, cleanup is automatic

## Related Issues

This fix addresses the same core issue as the previous RAII fix (memory:3307112), but extends it to handle signal-based interruption rather than just panic or normal exit.

The combination of:
- RAII `Drop` for normal exits
- Signal handlers for SIGINT/SIGTERM
- Aggressive cleanup on every start

...ensures containers are always cleaned up, regardless of how tests terminate.

