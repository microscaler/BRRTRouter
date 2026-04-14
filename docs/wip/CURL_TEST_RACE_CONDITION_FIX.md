# CURL Integration Test Race Condition Fix

## Problem

The `curl_openapi_yaml_served` test was failing intermittently (2/3 success rate) with:
```
docker port failed after 15 retries: exit code Some(1)
Error: No public port '8080/tcp' published for <container_id>
```

## Root Cause Analysis

The failure was caused by a **race condition** between:
1. Container startup (`docker run -d` returns immediately)
2. Docker daemon setting up network and port mapping
3. Test querying the port mapping (`docker port`)

**Key Issues:**
- No initial delay before first port query - Docker needs time to allocate random port and set up iptables rules
- Only checked if container was "running", not if it had exited/crashed
- No fallback mechanism when `docker port` failed
- Insufficient diagnostics when failures occurred
- Retry logic started too aggressively (100ms initial delay)

## Solution

### 1. Polling-Based Approach (No Fixed Delays)
Replaced fixed delays with adaptive polling that checks port availability immediately:
- Start with quick 50ms polling to detect fast setups (local development)
- Exponential backoff (50ms → 6.4s) adapts to slower environments (CI, GitHub Actions)
- No fixed delays - the system adapts to actual Docker readiness

### 2. Container Health Checks
- Check if container is running
- **NEW**: Check if container exited with non-zero code (crashed)
- Include container logs in error messages when container exits

### 3. Fallback Mechanism
- Primary: Use `docker port` (simpler, more reliable)
- **NEW**: Fallback to `docker inspect` after 3 retries if `docker port` fails
- This handles cases where Docker's port mapping is set up but `docker port` hasn't caught up yet

### 4. Enhanced Diagnostics
When port query fails after all retries, now includes:
- Container logs (last 100 lines)
- Full container inspect output
- Exit code information
- All previous error details

### 5. Improved Retry Logic
- Increased max retries from 15 to 30 (adapts to slower CI environments)
- **Polling-based**: Start with 50ms quick polling, then exponential backoff
- Exponential backoff: 50ms, 100ms, 200ms, 400ms, 800ms, 1.6s, 3.2s, 6.4s...
- Progress logging every 5 retries (and first 3 attempts for visibility)
- **No fixed delays** - adapts to actual Docker readiness

## Changes Made

**File**: `tests/curl_harness.rs`

1. **Removed fixed delays** - replaced with adaptive polling mechanism
2. Added container exit code checking
3. Added `docker inspect` fallback for port mapping
4. Enhanced error messages with container logs and inspect output
5. Increased retry count to 30 and improved backoff timing (50ms → 6.4s)
6. Added progress logging (every 5 retries + first 3 attempts)

## Testing

The fix addresses the race condition by:
- Giving Docker adequate time to set up networking
- Detecting container crashes early
- Providing multiple ways to query port mapping
- Offering comprehensive diagnostics for debugging

## Expected Outcome

- **Reliability**: Test should now pass consistently (near 100% success rate)
- **Adaptability**: Works in fast local environments (50ms polling) and slower CI environments (up to 20s timeout)
- **Debugging**: When failures do occur, comprehensive diagnostics will help identify root causes
- **Performance**: Fast in optimal conditions, adapts gracefully to slower environments

## Related Issues

This fix addresses intermittent test failures that were blocking CI/CD pipelines and causing confusion during development.

