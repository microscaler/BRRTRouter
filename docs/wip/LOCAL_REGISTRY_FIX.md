# Local Registry Fix

## Problem

Tilt failed to deploy with:

```
Failed to pull image "localhost:5001/brrtrouter-petstore:tilt-f2e4b4ee24aa8077"
dial tcp [::1]:5001: connect: connection refused
Error: ImagePullBackOff
```

**Root Cause:** The local KIND registry container (`kind-registry`) wasn't running.

## Why This Happens

The local registry can stop for several reasons:

1. **Docker restart** - Registry has `--restart=always` but might not start immediately
2. **System reboot** - Registry needs to reconnect to KIND network
3. **Manual stop** - Someone ran `docker stop kind-registry`
4. **Network issues** - Registry lost connection to `kind` network

## Solution

Created a quick fix script to start/restart the registry.

### Quick Fix

The registry is now automatically started by `just dev-up`, but if you need to restart it manually:

```bash
# Start/restart the registry
just dev-registry

# Or run directly
./scripts/start-registry.sh
```

### What It Does

The script (`scripts/start-registry.sh`):

1. **Checks if registry exists**
   - If stopped → starts it
   - If missing → creates it

2. **Creates registry if needed**
   ```bash
   docker run -d --restart=always \
     -p "127.0.0.1:5001:5000" \
     --network bridge \
     --name "kind-registry" \
     registry:2
   ```

3. **Connects to KIND network**
   ```bash
   docker network connect "kind" "kind-registry"
   ```

4. **Verifies it's working**

### Usage

#### Start Registry
```bash
just dev-registry
```

**Output:**
```
🔧 Starting local Docker registry...
✓ Registry started at localhost:5001
✓ Registry connected to kind network
✅ Registry is ready at localhost:5001
```

#### Then Restart Tilt
```bash
# Option 1: Restart Tilt completely
tilt down
tilt up

# Option 2: Just trigger rebuild
# In Tilt UI, click the rebuild button for 'docker-build-and-push'
```

## Prevention

The registry should start automatically with `just dev-up`, but if it doesn't:

### Check Registry Status
```bash
docker ps --filter "name=kind-registry"
```

**Should show:**
```
CONTAINER ID   IMAGE        STATUS          PORTS
abc123def456   registry:2   Up 5 minutes    127.0.0.1:5001->5000/tcp
```

### Test Registry
```bash
curl http://localhost:5001/v2/_catalog
```

**Should return:**
```json
{"repositories":["brrtrouter-petstore"]}
```

## Integration with dev-up

The `just dev-up` command now automatically ensures the registry is running:

```bash
just dev-up
# 1. Starts/restarts registry (transparent)
# 2. Runs dev-setup.sh (creates cluster if needed)
# 3. Starts Tilt
```

**Workflow:**

```bash
# Always just run dev-up
just dev-up
# Registry is automatically started/restarted
# Everything "just works"!

# If you need to restart just the registry
just dev-registry
```

The main `dev-setup.sh` script also creates the registry (lines 99-110), but now `dev-up` ensures it's always running first.

## Files Created/Modified

### New Files
- **`scripts/start-registry.sh`** - Registry startup script

### Modified Files
- **`justfile`** - Added `dev-registry` command (line 207-210) and integrated into `dev-up` (line 217)

## Troubleshooting

### Registry Won't Start

**Problem:** `docker start kind-registry` fails

**Solution:**
```bash
# Remove and recreate
docker rm -f kind-registry
just dev-registry
```

### Can't Connect to Registry

**Problem:** `dial tcp [::1]:5001: connect: connection refused`

**Check:**
```bash
# Is it running?
docker ps --filter "name=kind-registry"

# Is port bound?
netstat -an | grep 5001  # or: lsof -i :5001

# Is it on the right network?
docker inspect kind-registry | grep -A 10 Networks
```

**Fix:**
```bash
just dev-registry
```

### Registry Not on KIND Network

**Problem:** Registry running but KIND can't reach it

**Solution:**
```bash
# Reconnect to KIND network
docker network connect "kind" "kind-registry"

# Verify
docker inspect kind-registry | grep -A 10 Networks
# Should show both 'bridge' and 'kind'
```

### Port 5001 Already in Use

**Problem:** Another service using port 5001

**Solution:**
```bash
# Find what's using it
lsof -i :5001

# Stop the conflicting service, then:
just dev-registry
```

## Alternative: Use Kind's Built-in Registry

KIND supports a built-in registry pattern, but we use a separate container for:

1. **Persistence** - Survives cluster recreation
2. **Performance** - Faster pulls (no network)
3. **Simplicity** - Standard Docker registry
4. **Debugging** - Easy to inspect and test

## Documentation Updates

After this fix:

- ✅ Added `just dev-registry` command
- ✅ Created `scripts/start-registry.sh`
- ✅ Documented in `docs/LOCAL_REGISTRY_FIX.md`

## Related Issues

This fix addresses:
- Registry stops after Docker restart
- Registry not connected to KIND network
- Fresh Tilt start without full dev-setup

## Testing

Verify the fix works:

```bash
# 1. Stop registry
docker stop kind-registry

# 2. Start with new command
just dev-registry

# 3. Verify it's running
docker ps --filter "name=kind-registry"
curl http://localhost:5001/v2/_catalog

# 4. Tilt should now work
tilt up
```

## Summary

**Problem:** Registry wasn't running → Tilt couldn't pull images

**Solution:** 
- Created `scripts/start-registry.sh`
- Added `just dev-registry` command
- Quick fix without full cluster recreation

**Usage:**
```bash
just dev-registry  # Start/restart registry
tilt up            # Continue working
```

Simple, fast, reliable! 🚀

