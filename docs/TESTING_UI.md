# Testing the UI

## 🔍 Quick Diagnostic Commands

### 1. Check Files in Container

```bash
# Check what files are in the container
kubectl exec -n brrtrouter-dev deployment/petstore -- ls -la /app/static_site/

# Expected output:
# drwxrwxrwx    3 root     root    index.html
# drwxrwxrwx    2 root     root    assets/
# -rw-r--r--    1 root     root    dummy.txt
```

### 2. Check index.html Content in Container

```bash
# View the actual index.html served by the container
kubectl exec -n brrtrouter-dev deployment/petstore -- cat /app/static_site/index.html

# Expected: HTML with SolidJS app structure
# Should contain: <div id="root"></div>
# Should contain: <script type="module" crossorigin src="/assets/index-*.js">
```

### 3. Test HTTP Endpoint

```bash
# Test from outside the cluster
curl -v http://localhost:8080/

# Expected:
# HTTP/1.1 200 OK
# Content-Type: text/html
# <html>...SolidJS app...</html>
```

### 4. Test Asset Loading

```bash
# Get the asset filename from index.html
curl http://localhost:8080/ | grep -o 'assets/index-[^"]*\.js'

# Then test loading it
curl -I http://localhost:8080/assets/index-DNFbvFAK.js

# Expected: HTTP/1.1 200 OK
```

## 🔧 If Files Are Outdated

### Option 1: Trigger Tilt Sync

```bash
# In the BRRTRouter directory
# Touch a file to trigger live_update
touch examples/pet_store/static_site/index.html

# Tilt should sync within 1-2 seconds
```

### Option 2: Force Rebuild

```bash
# In Tilt UI
# Click on "build-sample-ui" → Click "Force Update"

# Or restart everything
tilt down
tilt up
```

### Option 3: Manual Pod Restart

```bash
# Force Kubernetes to restart the pod
kubectl rollout restart deployment/petstore -n brrtrouter-dev

# Wait for new pod
kubectl wait --for=condition=ready pod -l app=petstore -n brrtrouter-dev --timeout=60s
```

## 📊 Expected Behavior

### Root Endpoint (/)
- **Request**: `GET http://localhost:8080/`
- **Response**: 
  - Status: `200 OK`
  - Content-Type: `text/html`
  - Body: SolidJS app HTML with `<div id="root"></div>`

### Assets
- **Request**: `GET http://localhost:8080/assets/index-*.js`
- **Response**:
  - Status: `200 OK`
  - Content-Type: `application/javascript` or `text/javascript`
  - Body: Compiled SolidJS JavaScript bundle

- **Request**: `GET http://localhost:8080/assets/index-*.css`
- **Response**:
  - Status: `200 OK`
  - Content-Type: `text/css`
  - Body: Tailwind CSS (purged)

### API Endpoints (from UI)
- **Health**: `GET /health` → `{"status": "ok"}`
- **Pets**: `GET /pets` (with `X-API-Key: test123`) → Array of pets
- **Users**: `GET /users` (with `X-API-Key: test123`) → Array of users

## 🐛 Common Issues

### Issue 1: Old index.html

**Symptoms**: Browser shows "It works!" placeholder

**Cause**: Old HTML not overwritten by Tilt live_update

**Fix**:
```bash
# Force copy
cd sample-ui
yarn build:copy

# Then trigger Tilt sync
cd ..
touch examples/pet_store/static_site/index.html
```

### Issue 2: 404 on Assets

**Symptoms**: 
- HTML loads
- Browser console shows: `GET /assets/index-*.js 404 (Not Found)`

**Cause**: Assets not in container

**Fix**:
```bash
# Check container
kubectl exec -n brrtrouter-dev deployment/petstore -- ls -la /app/static_site/assets/

# If empty, rebuild
tilt trigger build-sample-ui
```

### Issue 3: Blank Page

**Symptoms**:
- HTML loads (200 OK)
- Assets load (200 OK)
- Page is blank/white

**Cause**: JavaScript error

**Fix**:
1. Open browser DevTools (F12)
2. Check Console tab for errors
3. Check Network tab - all requests 200?
4. Check if SolidJS bundle is valid

### Issue 4: CORS Errors

**Symptoms**: Browser console shows CORS errors when calling `/pets` or `/users`

**Cause**: Shouldn't happen (same origin)

**Fix**: Check if API key is correct (`X-API-Key: test123`)

## ✅ Success Checklist

- [ ] `kubectl exec` shows correct `index.html` in container
- [ ] `kubectl exec` shows `assets/` directory with JS and CSS
- [ ] `curl http://localhost:8080/` returns SolidJS HTML
- [ ] `curl http://localhost:8080/assets/index-*.js` returns 200
- [ ] Browser loads http://localhost:8080 without errors
- [ ] Browser console has no 404 errors
- [ ] Browser shows stats grid with data
- [ ] Pet list and user list populate
- [ ] Quick links are visible

## 🚀 Quick Test Script

```bash
#!/bin/bash
echo "=== Testing BRRTRouter UI ==="

echo -n "1. Health check: "
curl -s http://localhost:8080/health | grep -q "ok" && echo "✅" || echo "❌"

echo -n "2. Root HTML: "
curl -s http://localhost:8080/ | grep -q "root" && echo "✅" || echo "❌"

echo -n "3. Container index.html: "
kubectl exec -n brrtrouter-dev deployment/petstore -- cat /app/static_site/index.html | grep -q "root" && echo "✅" || echo "❌"

echo -n "4. Container assets: "
kubectl exec -n brrtrouter-dev deployment/petstore -- ls /app/static_site/assets/ | grep -q ".js" && echo "✅" || echo "❌"

echo -n "5. API with auth: "
curl -s -H "X-API-Key: test123" http://localhost:8080/pets | grep -q "id" && echo "✅" || echo "❌"

echo ""
echo "=== Open browser to test UI ==="
echo "http://localhost:8080"
```

Save as `scripts/test-ui.sh`, make executable, and run:
```bash
chmod +x scripts/test-ui.sh
./scripts/test-ui.sh
```

---

**Date**: October 9, 2025  
**Status**: Diagnostic Guide

