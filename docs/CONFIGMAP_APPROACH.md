# ConfigMap-Based Configuration ✅

## 🎯 Approach

Using Kubernetes ConfigMaps for application configuration - the **proper, production-ready way**.

## ✅ What We Did

### 1. Updated ConfigMap with Full Config

Replaced the minimal ConfigMap with the complete config structure from `examples/pet_store/config/config.yaml`:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: petstore-config
  namespace: brrtrouter-dev
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
    
    database:
      host: "postgres"
      port: 5432
      # ... full config
```

### 2. Re-Enabled ConfigMap Volume Mount

```yaml
volumeMounts:
  - name: config
    mountPath: /app/config
    readOnly: true

volumes:
  - name: config
    configMap:
      name: petstore-config
```

## 🎯 Why This Is Better

### Production-Ready Pattern
- ✅ **Kubernetes-native** - Standard approach for all K8s deployments
- ✅ **Environment separation** - Dev, staging, prod use different ConfigMaps
- ✅ **Secrets integration** - Can reference Kubernetes Secrets
- ✅ **GitOps friendly** - Config changes tracked in version control
- ✅ **No image rebuilds** - Change config without rebuilding containers

### Configuration Management
```bash
# Update config
kubectl edit configmap petstore-config -n brrtrouter-dev

# Or apply from file
kubectl apply -f k8s/petstore-deployment.yaml

# Restart to pick up changes
kubectl rollout restart deployment/petstore -n brrtrouter-dev
```

## 🔄 Local Development Workflow

### Option 1: Edit ConfigMap Directly (Recommended)

```bash
# 1. Edit the ConfigMap in k8s/petstore-deployment.yaml
vim k8s/petstore-deployment.yaml
# Change lines 7-81 (the config.yaml section)

# 2. Apply changes
kubectl apply -f k8s/petstore-deployment.yaml

# 3. Restart pod to pick up new config
kubectl rollout restart deployment/petstore -n brrtrouter-dev

# 4. Verify
tilt logs petstore | grep -A 20 "\[config\]"
```

### Option 2: Edit via kubectl

```bash
# Edit ConfigMap interactively
kubectl edit configmap petstore-config -n brrtrouter-dev

# Restart pod
kubectl rollout restart deployment/petstore -n brrtrouter-dev
```

### Option 3: Tilt Integration (Advanced)

We could add a `local_resource` to Tilt to sync config changes:

```python
# Tiltfile (future enhancement)
local_resource(
    'update-config',
    'kubectl create configmap petstore-config --from-file=config.yaml=examples/pet_store/config/config.yaml -n brrtrouter-dev --dry-run=client -o yaml | kubectl apply -f -',
    deps=['examples/pet_store/config/config.yaml'],
    labels=['config'],
)
```

This would auto-update the ConfigMap when the local file changes.

## 📊 Comparison: ConfigMap vs Docker

| Aspect | ConfigMap | Docker Image |
|--------|-----------|--------------|
| **Production** | ✅ Best practice | ❌ Anti-pattern |
| **Secrets** | ✅ K8s Secrets integration | ❌ Baked into image |
| **Updates** | ✅ No rebuild needed | ❌ Must rebuild image |
| **Environment** | ✅ One image, many configs | ❌ One image per env |
| **GitOps** | ✅ Track in Git | ⚠️ Mixed (code + config) |
| **Local iteration** | ⚠️ Edit + restart pod | ✅ Edit + Tilt sync |
| **Learning curve** | ⚠️ K8s knowledge needed | ✅ Simple files |

## 🏗️ Production Deployment

### Using Kubernetes Secrets

For production, use Secrets for sensitive data:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: petstore-secrets
  namespace: production
type: Opaque
stringData:
  api-key: "prod_key_from_vault"
  db-password: "secure_password"
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: petstore-config
  namespace: production
data:
  config.yaml: |
    security:
      api_keys:
        ApiKeyHeader:
          key: "${API_KEY}"  # Injected from Secret
    
    database:
      password: "${DB_PASSWORD}"  # Injected from Secret
```

### Environment Variable Substitution

Use an init container or envsubst:

```yaml
initContainers:
  - name: config-init
    image: alpine
    command:
      - sh
      - -c
      - |
        apk add --no-cache gettext
        envsubst < /config-template/config.yaml > /config/config.yaml
    env:
      - name: API_KEY
        valueFrom:
          secretKeyRef:
            name: petstore-secrets
            key: api-key
    volumeMounts:
      - name: config-template
        mountPath: /config-template
      - name: config
        mountPath: /config
```

## 🔍 Verification

After applying the ConfigMap approach:

```bash
# 1. Check ConfigMap exists and has content
kubectl get configmap petstore-config -n brrtrouter-dev -o yaml | grep -A 50 "config.yaml"

# Should see full config with api_keys and http sections

# 2. Check pod is using ConfigMap
kubectl describe pod -n brrtrouter-dev -l app=petstore | grep -A 5 "Mounts:"
# Should see: config from petstore-config

# 3. Check config is loaded in app
tilt logs petstore | grep -A 20 "\[config\]"
# Should see:
#   security:
#     api_keys:
#       ApiKeyHeader:
#         key: test123
#   http:
#     keep_alive: true

# 4. Test authentication works
curl -H "X-API-Key: test123" http://localhost:8080/pets
# Should return pet data (not 401)
```

## 📝 Files Modified

1. ✅ `k8s/petstore-deployment.yaml`
   - Updated ConfigMap with full config structure (lines 1-81)
   - Re-enabled ConfigMap volume mount (lines 152-154, 168-170)
2. ✅ `docs/CONFIGMAP_APPROACH.md` - This document
3. ✅ `docs/CONFIG_OVERRIDE_FIX.md` - Updated to reflect ConfigMap approach
4. ✅ `docs/THREE_FIXES_SUMMARY.md` - Updated to reflect ConfigMap approach

## 💡 Best Practices

### 1. Separate Concerns
- **ConfigMap**: Non-sensitive config (URLs, timeouts, feature flags)
- **Secret**: Sensitive data (API keys, passwords, tokens)

### 2. Environment Strategy
```
k8s/
  base/
    deployment.yaml       # Common config
  overlays/
    dev/
      configmap.yaml      # Dev-specific config
    staging/
      configmap.yaml      # Staging config
    production/
      configmap.yaml      # Production config
      secrets.yaml        # Production secrets
```

Use Kustomize or Helm for managing environments.

### 3. Config Validation
Always validate config before applying:

```bash
# Dry run to check syntax
kubectl apply -f k8s/petstore-deployment.yaml --dry-run=client

# Validate YAML structure
yq eval '.data."config.yaml"' k8s/petstore-deployment.yaml
```

### 4. Rollback Strategy
```bash
# View config history
kubectl rollout history deployment/petstore -n brrtrouter-dev

# Rollback if config breaks something
kubectl rollout undo deployment/petstore -n brrtrouter-dev
```

## 🔜 Future Enhancements

1. **Tilt auto-sync** - Local file changes auto-update ConfigMap
2. **Config validation** - Schema validation before applying
3. **Environment overlays** - Kustomize for dev/staging/prod
4. **Secret management** - Integrate with Vault or SOPS
5. **Hot reload** - Watch ConfigMap changes without pod restart

---

**Status**: ✅ Implemented  
**Approach**: Kubernetes ConfigMaps (production-ready)  
**Benefits**: K8s-native, secrets integration, no rebuilds  
**Trade-off**: Slightly slower local iteration (edit + restart pod)  
**Date**: October 9, 2025

