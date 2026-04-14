# Fix-Permissions Init Containers - Not An Error

## What You're Seeing

When checking Tilt logs or Kubernetes events, you may see messages like:
```
[fix-permissions]
```

**This is NOT an error!** It's informational output from init containers.

## What Are Init Containers?

Init containers are specialized containers that run before the main application containers. They're used for setup tasks like:
- Creating directories
- Setting file permissions
- Downloading configuration
- Waiting for dependencies

## Fix-Permissions Init Containers

Our observability stack uses `fix-permissions` init containers for:
- **Prometheus**: Sets ownership to user 65534 (nobody) for /prometheus
- **Grafana**: Sets permissions for /var/lib/grafana
- **Loki**: Ensures /loki has correct permissions
- **Jaeger**: Prepares storage directories

### Example Configuration (Prometheus)
```yaml
initContainers:
  - name: fix-permissions
    image: busybox:1.36
    command:
      - sh
      - -c
      - |
        mkdir -p /prometheus
        chmod -R 777 /prometheus
        chown -R 65534:65534 /prometheus
    volumeMounts:
      - name: storage
        mountPath: /prometheus
    securityContext:
      runAsUser: 0  # Run as root to change permissions
```

## Why Are They Needed?

Many observability tools run as non-root users for security:
- Prometheus runs as user `nobody` (65534)
- Grafana runs as user `grafana` (472)
- Loki runs as user `loki` (10001)

These users need write access to their data directories, but Kubernetes PersistentVolumes are often created with root ownership. The init containers fix this mismatch.

## Checking Init Container Status

To verify init containers completed successfully:

```bash
# Check pod status
kubectl get pods -n brrtrouter-dev

# Check init container logs (usually empty if successful)
kubectl logs -n brrtrouter-dev <pod-name> -c fix-permissions

# Check pod events
kubectl describe pod -n brrtrouter-dev <pod-name>
```

## Common Issues (Not Related to fix-permissions)

If pods are stuck in `PodInitializing`:

### 1. Image Pull Issues
```bash
# Check events for image pull errors
kubectl get events -n brrtrouter-dev --field-selector involvedObject.name=<pod-name>

# Common causes:
# - Docker Hub rate limiting
# - Network issues (504 Gateway Timeout)
# - Image doesn't exist
```

### 2. Volume Mount Issues
```bash
# Check if PVC is bound
kubectl get pvc -n brrtrouter-dev

# Check if PV exists
kubectl get pv
```

### 3. Init Container Failures
```bash
# Check init container exit code
kubectl describe pod -n brrtrouter-dev <pod-name> | grep -A10 "Init Containers"
```

## Current Status

As of the last check:
- ✅ Prometheus: Running (init completed)
- ✅ Loki: Running (init completed)
- ✅ Jaeger: Running (init completed)
- ⏳ Grafana: Initializing (image pull in progress)
- ✅ All other components: Running

## Summary

The `[fix-permissions]` output is:
- ✅ Normal and expected
- ✅ Indicates init container execution
- ✅ Usually means setup is proceeding correctly
- ❌ NOT an error
- ❌ NOT something to fix

If you see this message, it means the init containers are doing their job to prepare the environment for the main application containers.
