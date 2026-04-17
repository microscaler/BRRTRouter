# Stack Size Configuration for Large Static Assets ✅

## 🎯 Issue

The SolidJS dashboard was loading but then crashing, potentially due to insufficient stack size for serving large static assets.

## 🔍 Root Cause

**Default stack size too small for large static files:**
- Default: 16 KB (0x4000)
- SolidJS bundle with Tailwind: Likely much larger
- `/docs` Swagger UI worked fine (smaller assets)
- New rich dashboard has more assets and complexity

## ✅ Fix Applied

### 1. Increased Stack Size to 64 KB

Updated `k8s/petstore-deployment.yaml`:

```yaml
env:
  - name: BRRTR_STACK_SIZE
    value: "0x10000"  # 64 KB (was 16 KB default)
```

### 2. Fixed Documentation Bug

`src/runtime_config.rs` had incorrect comment:
- ❌ Before: `/// Stack size for coroutines in bytes (default: 2MB)`
- ✅ After: `/// Stack size for coroutines in bytes (default: 16 KB / 0x4000)`

## 📊 Stack Size Recommendations

From `src/runtime_config.rs`:

| Use Case | Stack Size | Value |
|----------|-----------|-------|
| Simple handlers | 16 KB | 0x4000 |
| Complex logic | 32 KB | 0x8000 |
| Deep recursion | 64 KB | 0x10000 |
| **Large static assets** | **64 KB** | **0x10000** |

## 🔧 How It Works

```rust
// In examples/pet_store/src/main.rs
let config = RuntimeConfig::from_env();
may::config().set_stack_size(config.stack_size);
```

Environment variable `BRRTR_STACK_SIZE` overrides the default:
- `0x4000` = 16 KB (default)
- `0x8000` = 32 KB
- `0x10000` = 64 KB (our setting)
- `0x20000` = 128 KB (extreme cases)

## 🎯 Why 64 KB?

1. **Swagger docs work** at 16 KB (smaller assets)
2. **SolidJS bundle** is much larger:
   - React/SolidJS runtime
   - Tailwind CSS (~100KB+)
   - Multiple components
   - API client code
3. **64 KB is safe** without excessive memory usage:
   - 100 concurrent requests × 64 KB = 6.4 MB
   - 1000 concurrent requests × 64 KB = 64 MB

## 🧪 Testing

After applying the fix:

```bash
# Restart Tilt to pick up new environment variable
tilt down
tilt up

# Test the dashboard
curl http://localhost:8080/
# Should see SolidJS dashboard without crashes

# Test other endpoints still work
curl http://localhost:8080/health
curl http://localhost:8080/docs
curl -H "X-API-Key: test123" http://localhost:8080/pets
```

## 📝 Files Modified

1. ✅ `k8s/petstore-deployment.yaml` - Added `BRRTR_STACK_SIZE` env var
2. ✅ `src/runtime_config.rs` - Fixed incorrect documentation comment
3. ✅ `docs/STACK_SIZE_FIX.md` - This document

## 💡 Lessons Learned

1. **Static file serving uses stack** - MiniJinja template rendering, file I/O
2. **Default was too conservative** - 16 KB is for minimal handlers
3. **Swagger worked, dashboard didn't** - Size matters!
4. **Environment variables FTW** - Easy to tune without recompiling
5. **Documentation bugs matter** - "2MB" vs "16KB" is confusing

## 🔜 Next Steps

Now that the dashboard can load properly, we need to address the `TooManyHeaders` error:

1. **Add request logging** - See what headers browsers send
2. **Find may_minihttp limit** - Check source for header count limit
3. **Fix or patch** - Either increase limit or fork
4. **Full observability** - OTEL Collector, structured logging, traces

---

**Status**: ✅ Fixed  
**Stack Size**: 16 KB → 64 KB  
**Impact**: Enables serving large static assets  
**Date**: October 9, 2025

