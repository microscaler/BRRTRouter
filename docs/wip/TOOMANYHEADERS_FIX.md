# TooManyHeaders Fix - RESOLVED ✅

## Problem
BRRTRouter was crashing with `TooManyHeaders` errors when:
- Refreshing the `/docs` Swagger page multiple times
- Handling browser traffic with 12-15 headers
- Processing API gateway/Kubernetes ingress traffic with 20+ headers

## Root Cause
The underlying `may_minihttp` library had a hardcoded limit of 16 headers (from `httparse::MAX_HEADERS`). Modern HTTP traffic commonly exceeds this:
- Browsers: 12-15 headers (typical)
- API Gateways: 15-25 headers
- Kubernetes Ingress: 20+ headers (with tracing/metadata)

## Solution

### 1. Fork and Enhance `may_minihttp`
**Repository**: https://github.com/microscaler/may_minihttp  
**Branch**: `feat/configurable-max-headers`

#### Changes Made:
- ✅ Added `HttpServerWithHeaders<T, const N: usize>` using const generics
- ✅ Kept `HttpServer` unchanged at 16 headers (backwards compatible)
- ✅ Supported header sizes: 32 (Standard), 64 (Large), 128 (XLarge), Custom
- ✅ Enhanced error messages with actual header count and suggestions
- ✅ Fixed MAY runtime initialization in tests
- ✅ Improved server readiness checks
- ✅ Added HTTP keep-alive support
- ✅ 74 passing tests including Goose load tests

### 2. Update BRRTRouter to Use 32 Headers
**File**: `src/server/http_server.rs`

```rust
use may_minihttp::{HttpServerWithHeaders, HttpService};

// In HttpServer::start()
let handle = HttpServerWithHeaders::<_, 32>(self.0).start(addr)?;
```

**Result**: BRRTRouter now handles 32 headers by default, suitable for production traffic.

## Verification

### Before Fix
```
❌ Swagger /docs page crashes after multiple refreshes
❌ TooManyHeaders errors in logs
❌ Service restarts required
```

### After Fix
```
✅ Swagger /docs page works reliably with multiple refreshes
✅ No TooManyHeaders errors
✅ Stable service operation
✅ Pet Store service running smoothly in Tilt/K8s
```

## Testing Performed

### Local Development (Tilt + KIND)
- ✅ Pet Store service stable
- ✅ Swagger UI (`/docs`) handles repeated refreshes
- ✅ No crashes or TooManyHeaders errors
- ✅ Full observability stack operational

### Load Testing
- ✅ 74 tests passing in `may_minihttp`
- ✅ Goose load tests with varying header counts
- ✅ Boundary testing (16, 17, 20, 32, 64 headers)
- ✅ Realistic browser and API gateway traffic patterns

## Files Modified

### may_minihttp Fork
- `src/http_server.rs` - Added `HttpServerWithHeaders` and `each_connection_loop_with_headers`
- `src/lib.rs` - Exported `HttpServerWithHeaders`
- `src/request.rs` - Enhanced error messages
- `tests/*.rs` - Fixed MAY runtime initialization, added comprehensive tests
- `PR.md` - Documented changes for upstream PR

### BRRTRouter
- `src/server/http_server.rs` - Changed to use `HttpServerWithHeaders<_, 32>`
- `Cargo.toml` - Points to fork with `feat/configurable-max-headers` branch

## Performance Impact
- **Zero-cost abstraction**: Const generics compile to same code as hardcoded values
- **No runtime overhead**: Header limit checked at compile time
- **Memory**: ~512 bytes per connection (16 headers → 32 headers)

## Backwards Compatibility
- ✅ Existing `HttpServer` users unaffected (still 16 headers)
- ✅ Opt-in upgrade path via `HttpServerWithHeaders`
- ✅ No breaking changes to `may_minihttp` API

## Next Steps
1. ✅ ~~Fork and fix `may_minihttp`~~ - COMPLETE
2. ✅ ~~Update BRRTRouter to use 32 headers~~ - COMPLETE
3. ✅ ~~Verify in Tilt/K8s environment~~ - COMPLETE
4. 🔄 Submit PR to upstream `may_minihttp` (optional)
5. 🔄 Monitor production traffic patterns

## References
- **Fork**: https://github.com/microscaler/may_minihttp
- **Branch**: `feat/configurable-max-headers`
- **Upstream Issue**: https://github.com/Xudong-Huang/may_minihttp/issues/18
- **BRRTRouter Docs**: `docs/LOAD_TESTING.md`, `docs/TEST_SETUP_TEARDOWN.md`

---
**Status**: ✅ **RESOLVED**  
**Date**: October 10, 2025  
**Verified**: Pet Store service stable in Tilt, Swagger UI working after multiple refreshes

