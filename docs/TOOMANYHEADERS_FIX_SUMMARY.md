# TooManyHeaders Fix - Complete Summary

## The Real Test: Tilt/K8s

The fix must work in production-like environment (Tilt/K8s) where the actual service handles real HTTP traffic.

## Quick Verification

```bash
# THE REAL TEST - Verifies fix works in Tilt/K8s
just verify-fix
```

This will:
1. ✅ Verify patch is applied
2. ✅ Rebuild pet_store binary with patched source
3. ✅ Ensure Tilt is running
4. ✅ Wait for pod to be ready
5. ✅ Send 10 requests with 50+ headers each
6. ✅ Check logs for `TooManyHeaders` errors
7. ✅ Report success/failure

## What Was Fixed

### Root Cause
`vendor/may_minihttp/src/request.rs` line 15:
```rust
// BEFORE (TOO SMALL):
pub(crate) const MAX_HEADERS: usize = 16;

// AFTER (FIXED):
pub(crate) const MAX_HEADERS: usize = 128;
```

### Configuration
`.cargo/config.toml`:
```toml
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
```

This tells Cargo to use our patched version from `vendor/` instead of downloading from crates.io.

## Expected Results

### Success Output
```
🔧 Verifying TooManyHeaders Fix in Tilt/K8s
=============================================

✅ Patch confirmed: MAX_HEADERS = 128
✅ Build complete
✅ Tilt is running
✅ Pod is ready
✅ Sent 10 requests with 50+ headers each

TooManyHeaders errors before: 0
TooManyHeaders errors after: 0
New errors: 0

╔════════════════════════════════════╗
║   🎉 SUCCESS! 🎉                   ║
║                                    ║
║   No TooManyHeaders errors!        ║
║   Fix is working in K8s!           ║
╚════════════════════════════════════╝

✅ Service handled 10 requests with 50+ headers each
✅ No TooManyHeaders errors in logs
✅ Patch is working correctly in Tilt/K8s
```

### What the Test Sends

Each request includes 50+ headers:
- Standard browser headers (User-Agent, Accept-*, etc.)
- Load balancer headers (X-Forwarded-*, X-Real-IP)
- Auth headers (Authorization, X-API-Key)
- Tracking headers (X-Request-ID, X-Trace-ID, X-Span-ID)
- Custom headers (X-Custom-1 through X-Custom-25)

**Total: 50+ headers per request**  
**Before patch**: Would fail at 17+ headers ❌  
**After patch**: Should handle all 50+ headers ✅

## Troubleshooting

### If Still Seeing TooManyHeaders Errors

```bash
# 1. Check if Tilt rebuilt with new binary
tilt logs build-petstore

# 2. Trigger manual rebuild
tilt trigger build-petstore

# 3. Or restart Tilt completely
tilt down
tilt up

# 4. Check the running pod has the patch
kubectl exec -n brrtrouter-dev -l app=petstore -- \
  cat /proc/self/cmdline
# Should show the binary path

# 5. Verify vendor config
cat .cargo/config.toml | grep -A 3 "vendored"
```

### Force Rebuild Everything

```bash
# Clean everything
cargo clean
rm -rf target/

# Rebuild
cargo build --release -p pet_store

# Restart Tilt
tilt down
tilt up

# Run verification
just verify-fix
```

## Manual Testing in K8s

If you want to test manually:

```bash
# 1. Check current errors
kubectl logs -n brrtrouter-dev -l app=petstore --tail=50 | grep TooManyHeaders

# 2. Send request with many headers
curl -v \
  -H "User-Agent: Mozilla/5.0" \
  -H "Accept: application/json" \
  -H "X-Test-1: 1" \
  -H "X-Test-2: 2" \
  ... (add 30+ more headers) \
  http://localhost:9090/health

# 3. Check logs again
kubectl logs -n brrtrouter-dev -l app=petstore --tail=50

# Should see HTTP 200 responses, NOT TooManyHeaders errors
```

## What Success Looks Like

### In Logs (Grafana/Loki)
```
[startup] 🚀 Starting pet_store server
[route] GET /health -> health_check
Server started successfully on 0.0.0.0:8080
```

**NO** lines containing:
```
failed to parse http request: TooManyHeaders  ❌ (shouldn't see this)
```

### In Metrics (Prometheus)
```promql
# Error rate should be 0%
rate(brrtrouter_requests_total{status=~"5.."}[5m])

# All requests should succeed
rate(brrtrouter_requests_total{status="200"}[5m])
```

### In Grafana Dashboard
- ✅ Request Rate: Shows all requests
- ✅ Error Rate: 0%
- ✅ Logs Panel: No "TooManyHeaders" errors

## Files Modified

1. **`vendor/may_minihttp/src/request.rs`** (line 15)
   - Changed `MAX_HEADERS` from 16 to 128
   - Added explanatory comment

2. **`.cargo/config.toml`** (lines 13-19)
   - Added vendored sources configuration

3. **`scripts/verify-tilt-fix.sh`** (NEW)
   - Real-world K8s testing script

4. **`justfile`** (line 25-27)
   - Added `verify-fix` command

## Why This Approach

### Why Vendor + Patch?
- ✅ **Immediate fix** - Don't wait for upstream
- ✅ **Controlled** - We own the change
- ✅ **Documented** - Clear what was changed and why
- ✅ **Reversible** - Can remove patch when upstream fixes

### Why Test in K8s?
- ✅ **Real environment** - Not just local tests
- ✅ **Real traffic** - Kubernetes probes, actual headers
- ✅ **Real HTTP parsing** - The actual service, not a mock
- ✅ **Production-like** - Same as what customers see

## Next Steps

### After Verification Passes

1. **Monitor in production**
   ```bash
   # Watch logs for 5 minutes
   kubectl logs -n brrtrouter-dev -l app=petstore -f | grep -i "TooMany\|error"
   
   # Should see no TooManyHeaders errors
   ```

2. **Load test**
   ```bash
   # Run Goose load tests
   cargo run --example api_load_test
   
   # Should handle all traffic successfully
   ```

3. **Update documentation**
   - Add note in README about the patch
   - Document maintenance procedure
   - Consider upstream PR

### Upstream Contribution

Consider creating an issue/PR for `may_minihttp`:

```markdown
Title: Increase MAX_HEADERS to handle modern web apps

Problem: Current limit of 16 headers causes TooManyHeaders errors
with modern web applications (browsers + load balancers + auth).

Proposed: Increase to 64 or 128 headers.
Memory impact: +2-5KB per request (negligible vs 64KB coroutine stack).

Would you accept a PR?
```

## Summary

✅ **Patch applied**: `MAX_HEADERS` 16 → 128  
✅ **Config updated**: Using vendored sources  
✅ **Test script ready**: `just verify-fix`  
✅ **Real test**: Sends 50+ headers to K8s service  
✅ **Clear reporting**: Success/failure with logs  

**Run the test:**
```bash
just verify-fix
```

**This is the definitive test** - if it passes, the fix works in production! 🎯

