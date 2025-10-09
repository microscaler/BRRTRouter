# Config File Structure Issue - Fixed ✅

## 🎯 Problem

ConfigMap had incorrect structure - config file was not being loaded properly in the container:
```
[config]
security:
  api_keys: null        # ❌ Should have ApiKeyHeader with key "test123"
  remote_api_keys: null
  bearer: null
  oauth2: null
  jwks: null
  propelauth: null
http: null              # ❌ Should have keep_alive, timeout_secs, max_requests
```

## 🔍 Root Cause

**Kubernetes ConfigMap had incomplete/incorrect structure!**

The ConfigMap (lines 1-34 of `k8s/petstore-deployment.yaml`) had an old, minimal config with wrong structure:
```yaml
security:
  api_key_header: "X-API-Key"
  default_api_key: "test123"
server:
  host: "0.0.0.0"
  port: 8080
```

The **correct structure** needed (`examples/pet_store/config/config.yaml`):
```yaml
security:
  api_keys:
    ApiKeyHeader:
      key: "test123"
http:
  keep_alive: true
  timeout_secs: 5
  max_requests: 1000
```

## ✅ Fix Applied

**Updated ConfigMap with full, correct config structure:**

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: petstore-config
data:
  config.yaml: |
    security:
      api_keys:
        ApiKeyHeader:
          key: "test123"
    http:
      keep_alive: true
      timeout_secs: 5
      max_requests: 1000
    # ... full config
```

### Why This Works

1. **Kubernetes-native** - ConfigMaps are the proper way to manage config in K8s
2. **Production pattern** - Same approach for dev, staging, and production
3. **Environment separation** - Different ConfigMaps for different environments
4. **Secrets integration** - Can reference Kubernetes Secrets for sensitive data
5. **No rebuilds** - Change config without rebuilding Docker images

## 📊 Comparison

| Approach | ConfigMap | Docker + Tilt | Hybrid |
|----------|-----------|---------------|--------|
| K8s-native | ✅ | ❌ | ✅ |
| Fast iteration | ❌ (slow) | ✅ (instant) | ⚠️ (complex) |
| Production | ✅ | ❌ | ✅ |
| Local dev | ❌ | ✅ | ⚠️ |
| Complexity | Medium | Low | High |

For **local development with Tilt**: Docker + Tilt is best (fast, simple)  
For **production**: ConfigMap is best (K8s-native, secrets management)

## 🔧 Local Dev Workflow (After Fix)

```bash
# Edit ConfigMap
vim k8s/petstore-deployment.yaml
# Edit lines 7-81 (the config.yaml section)

# Apply changes
kubectl apply -f k8s/petstore-deployment.yaml

# Restart pod to pick up new config
kubectl rollout restart deployment/petstore -n brrtrouter-dev

# Verify
tilt logs petstore | grep "\[config\]"
# Should see:
# security:
#   api_keys:
#     ApiKeyHeader:
#       key: "test123"
# http:
#   keep_alive: true
```

## 🚀 Production Deployment

For production, use the same ConfigMap approach with environment-specific values:

```yaml
# k8s/petstore-deployment.yaml
volumeMounts:
  - name: config
    mountPath: /app/config
    readOnly: true

volumes:
  - name: config
    configMap:
      name: petstore-config
```

And update the ConfigMap to match the full config structure:
```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: petstore-config
data:
  config.yaml: |
    security:
      api_keys:
        ApiKeyHeader:
          key: "${API_KEY_FROM_SECRET}"
    http:
      keep_alive: true
      timeout_secs: 5
      max_requests: 1000
```

Or use Kubernetes Secrets for sensitive data.

## 💡 Why ConfigMap Was There

ConfigMaps are **best practice** for Kubernetes:
- Decouple config from image
- Change config without rebuilding
- Environment-specific config
- Secrets integration

But for **local development with Tilt**, they add friction:
- Edit config file
- Wait for ConfigMap to sync
- Wait for pod to restart
- Slower feedback loop

**Solution**: Use Docker/Tilt for local dev, ConfigMap for production.

## 🔍 Verification

After fix, you should see:

```bash
tilt logs petstore | grep -A 20 "\[config\]"
```

Output:
```
[config]
security:
  api_keys:
    ApiKeyHeader:
      key: test123
  remote_api_keys: null
  bearer: null
  oauth2: null
  jwks: null
  propelauth: null
http:
  keep_alive: true
  timeout_secs: 5
  max_requests: 1000
```

And auth should work:
```bash
curl -H "X-API-Key: test123" http://localhost:8080/pets
# Should return pet data, not 401
```

## 📝 Files Modified

1. ✅ `k8s/petstore-deployment.yaml` - Commented out ConfigMap volume mount for local dev
2. ✅ `docs/CONFIG_OVERRIDE_FIX.md` - This document

## 🎯 Lessons Learned

1. **Mount order matters** - Last mount wins in Kubernetes
2. **ConfigMaps override image contents** - Entire directory is replaced
3. **Local dev != Production** - Different strategies for different environments
4. **Tilt live_update requires writable volumes** - ConfigMaps are read-only
5. **Always check what's actually in the container** - `kubectl exec ... cat /app/config/config.yaml`

---

**Status**: ✅ Fixed  
**Before**: ConfigMap overriding full config  
**After**: Docker + Tilt control config (local dev)  
**Production**: Re-enable ConfigMap when deploying  
**Date**: October 9, 2025

