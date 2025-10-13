# SIGINT Docker Cleanup Fix - Summary

## Problem Solved

When running `just nt` (nextest) and pressing Ctrl+C, Docker containers were left orphaned, causing subsequent test runs to hang for 60+ seconds.

## Root Cause

The `curl_integration_tests` use a static `OnceLock<ContainerHarness>` to share one container across all tests. When SIGINT is received, the process exits immediately, the static never goes out of scope, and `Drop` is never called.

## Solution Implemented

Added POSIX signal handling to ensure cleanup even on SIGINT/SIGTERM:

### Code Changes

**1. `tests/curl_harness.rs`** - Added signal handler registration:
- `register_signal_handlers()` - Registers SIGINT/SIGTERM handlers
- `SIGNAL_CLEANUP_RUNNING` - Atomic flag to prevent recursive cleanup
- Updated `base_url()` to register handlers on first use
- Enhanced `cleanup_orphaned_containers()` with better logging and polling
- Changed `ContainerHarness::start()` to **always** cleanup first

**2. `Cargo.toml`** - Added dependency:
```toml
libc = "0.2"  # For signal handling in tests (SIGINT cleanup)
```

### User Experience

Before:
```bash
$ just nt curl
# Press Ctrl+C
^C
$ docker ps | grep brrtrouter
brrtrouter-e2e-12345  # Container still running!
$ just nt curl  # Next run hangs for 60+ seconds
```

After:
```bash
$ just nt curl
# Press Ctrl+C
🛑 Signal received - cleaning up Docker containers...
✓ Removed container: brrtrouter-e2e-12345
✓ Cleanup complete
^C
$ docker ps | grep brrtrouter  # Nothing!
$ just nt curl  # Starts immediately
```

## Documentation Created

- **`docs/SIGINT_CLEANUP_FIX.md`** - Full technical explanation
- **`docs/TEST_SETUP_TEARDOWN.md`** - Added "Signal Handling for Static Resources" section

## Key Insights

1. **RAII alone is insufficient** for static resources and SIGINT
2. **Signal handlers are necessary** when cleanup is critical
3. **Defense in depth**: Always cleanup on start + Drop + signal handler
4. **Platform-specific**: Uses POSIX `libc::signal()` (works on Linux/macOS)

## Testing

Run the curl integration tests and press Ctrl+C:

```bash
just nt curl
# Press Ctrl+C - you should see cleanup messages

# Verify no containers left behind
docker ps -a | grep brrtrouter-e2e

# Run again immediately - should start cleanly
just nt curl
```

## Files Modified

1. `tests/curl_harness.rs` - Signal handler implementation
2. `Cargo.toml` - Added `libc` dependency
3. `docs/SIGINT_CLEANUP_FIX.md` - New comprehensive documentation
4. `docs/TEST_SETUP_TEARDOWN.md` - Added signal handling section
5. `docs/SIGINT_FIX_SUMMARY.md` - This summary

## Related Work

This extends the RAII cleanup work (memory:3307112) to handle signal-based interruption, completing the comprehensive cleanup story for BRRTRouter tests.

