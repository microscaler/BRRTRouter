# Testing Without SolidJS Bundle

## 🎯 Objective

Test the API and service without the SolidJS bundle to isolate whether the crash is caused by:
1. **The SolidJS bundle itself** (large JS file)
2. **The HTTP server's handling of assets**
3. **Something else entirely**

## 🔍 Key Observation

**Swagger UI works fine and uses CDN!**

```html
<!-- Swagger UI at /docs uses external CDN resources -->
<script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui.css" />
```

**Implications:**
- ✅ CDN loading works (no CORS/network issues)
- ✅ Server can serve HTML that loads external resources
- ❓ Self-hosted JS bundle (~30KB) causes crash
- ❓ Is it the bundle size, content, or serving mechanism?

## ✅ Changes Made

### 1. Disabled SolidJS Build in Tiltfile

```python
# 0. Build sample-ui (SolidJS + Tailwind) - DISABLED for testing
# Uncomment to re-enable rich dashboard
# local_resource(
#     'build-sample-ui',
#     'cd sample-ui && yarn install && yarn build:petstore',
#     ...
# )
```

### 2. Updated package.json

```json
{
  "scripts": {
    "dev": "vite",
    "build": "vite build",                    // ← Builds to sample-ui/dist
    "build:petstore": "vite build --outDir ../examples/pet_store/static_site --emptyOutDir",
    "preview": "vite preview"
  }
}
```

**Now:**
- `yarn build` - Builds to `sample-ui/dist/` (SolidJS bundle preserved)
- `yarn build:petstore` - Builds to `examples/pet_store/static_site/` (when we want to deploy it)

### 3. Created Simple Static Site

**`examples/pet_store/static_site/index.html`** - Minimal HTML with:
- ✅ Simple inline CSS (no external files)
- ✅ Links to API endpoints
- ✅ No JavaScript at all
- ✅ ~2KB total size

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <title>BRRTRouter Pet Store - Simple</title>
    <style>/* Simple inline CSS */</style>
</head>
<body>
    <h1>🐾 BRRTRouter Pet Store</h1>
    <a href="/health">Health Check</a>
    <a href="/metrics">Metrics</a>
    <a href="/docs">Swagger UI</a>
</body>
</html>
```

## 🧪 Test Plan

### Step 1: Test Simple Static Site
```bash
# Restart Tilt
tilt down
tilt up

# Test root page
curl http://localhost:8080/
# Should see simple HTML (no crash!)

# Test it loads in browser
open http://localhost:8080/
```

### Step 2: Test API Endpoints
```bash
# Health check
curl http://localhost:8080/health
# Should see: {"status":"ok"}

# Metrics
curl http://localhost:8080/metrics
# Should see Prometheus metrics

# Swagger UI (uses CDN)
curl http://localhost:8080/docs
# Should see Swagger UI HTML

# API with auth
curl -H "X-API-Key: test123" http://localhost:8080/pets
# Should see pet data
```

### Step 3: Check for TooManyHeaders
```bash
# Monitor logs
tilt logs petstore | grep -i "TooManyHeaders"

# Refresh page multiple times in browser
for i in {1..10}; do
  curl -s http://localhost:8080/ > /dev/null
  echo "Request $i"
done

# Check if errors occur
tilt logs petstore | tail -50
```

### Step 4: Test Swagger UI (Known Working)
```bash
# Swagger uses CDN and works
open http://localhost:8080/docs
# Interact with API through Swagger UI
```

## 📊 Expected Outcomes

### If Simple HTML Works
✅ **Root cause**: SolidJS bundle content or size  
**Next steps**:
- Try serving just the CSS file
- Try serving a small JS file
- Gradually add complexity

### If Simple HTML Also Crashes
❌ **Root cause**: Static file serving mechanism itself  
**Next steps**:
- Check `TooManyHeaders` errors more carefully
- May be browser headers vs Tilt health probe headers
- Add request logging to see what's happening

### If TooManyHeaders Persists
⚠️ **Root cause**: `may_minihttp` header limit  
**Next steps**:
- Find header limit in `may_minihttp` source
- Patch or fork library
- Or switch to different HTTP library

## 🔄 Restoring SolidJS

When testing is complete:

### Quick Restore
```bash
# Uncomment in Tiltfile
vim Tiltfile
# Uncomment lines 23-35 (build-sample-ui)
# Uncomment line 84 (resource_deps)

# Rebuild UI to petstore
cd sample-ui
yarn build:petstore

# Restart Tilt
tilt down
tilt up
```

### Or Keep Separate
Keep SolidJS in `sample-ui/dist/` for development:
```bash
cd sample-ui
yarn dev
# Visit http://localhost:5173 for UI development
# API still at http://localhost:8080
```

## 💡 Why This Helps

1. **Isolate the problem**
   - Simple HTML = Eliminate bundle as cause
   - API-only = Test without static files at all

2. **Compare with Swagger**
   - Swagger works (CDN resources)
   - Our bundle crashes (self-hosted)
   - What's the difference?

3. **Gradual reintroduction**
   - Start simple
   - Add complexity step-by-step
   - Find exact breaking point

## 📝 Files Modified

1. ✅ `Tiltfile` - Commented out `build-sample-ui` resource
2. ✅ `sample-ui/package.json` - Split build commands
3. ✅ `examples/pet_store/static_site/index.html` - Simple HTML placeholder
4. ✅ `docs/TESTING_WITHOUT_SOLIDJS.md` - This document

## 🎯 SolidJS Preserved

**`sample-ui/` remains unchanged:**
- All source code intact
- `yarn dev` still works for development
- `yarn build:petstore` can rebuild when ready
- No code deleted, just build process disabled

---

**Status**: ✅ Ready to test  
**SolidJS**: Preserved in `sample-ui/`  
**Static site**: Simple HTML placeholder  
**Next**: `tilt down && tilt up` and test!  
**Date**: October 9, 2025

