# Scripts → Justfile Migration

## Decision

**All shell scripts removed** - functionality moved to `justfile` and `Tiltfile`.

## Rationale

✅ **Single source of truth** - All commands in one place  
✅ **Self-documenting** - `just --list` shows everything  
✅ **Cross-platform** - Just handles platform differences  
✅ **Version controlled** - Justfile in git, scripts were too  
✅ **Simpler** - No chmod, no PATH issues  
✅ **Composable** - Just recipes can call each other  

## What Was Removed

All files in `scripts/` directory deleted. Functionality preserved in `justfile`.

### Infrastructure Scripts → Justfile

| Old Script | New Command | Notes |
|------------|-------------|-------|
| `scripts/dev-setup.sh` | `just dev-up` | Integrated into Tilt startup |
| `scripts/dev-teardown.sh` | `just dev-down` | Simple kind/docker commands |
| `scripts/start-registry.sh` | `just dev-registry` | Docker registry management |
| `scripts/download-velero-crds.sh` | `just download-velero-crds` | One-time curl command |

### Testing Scripts → Justfile

| Old Script | New Command | Status |
|------------|-------------|--------|
| `scripts/test-header-limits.sh` | Removed | Use `cargo test` instead |
| `scripts/rebuild-and-test.sh` | `just rebuild-test` | Inline in justfile |
| `scripts/verify-tilt-fix.sh` | `just verify-fix` | Inline in justfile |

### Verification Scripts → Justfile/kubectl

| Old Script | Replacement | Notes |
|------------|-------------|-------|
| `scripts/verify-observability.sh` | `kubectl get pods -n brrtrouter-dev` | Native kubectl |
| `scripts/verify-registry.sh` | `docker ps \| grep registry` | Native docker |
| `scripts/verify-everything.sh` | `just dev-status` | Inline in justfile |

### Utility Scripts → Removed

| Old Script | Replacement | Notes |
|------------|-------------|-------|
| `scripts/debug-pod.sh` | `kubectl logs/exec` | Use kubectl directly |
| `scripts/cleanup-test-containers.sh` | Integrated into test cleanup | Auto-cleanup in tests |
| `scripts/check-ports.sh` | `lsof -i :<port>` | Use system tools |
| `scripts/test-ui.sh` | `curl http://localhost:3000` | Simple curl |
| `scripts/build_pet_store.sh` | Tilt handles | Tilt builds automatically |
| `scripts/vendor-may-minihttp.sh` | Documentation only | Patch already applied |

## New Justfile Structure

```makefile
# Infrastructure
just dev-up              # Create KIND cluster + start Tilt
just dev-down            # Tear down everything
just dev-registry        # Start local Docker registry
just dev-status          # Show cluster status

# Backup
just download-velero-crds  # Download Velero CRDs
just start-minio          # Start MinIO backup server
just backup-now           # Create backup
just backup-list          # List backups

# Testing
just test                 # Run all tests
just nt                   # Run tests with nextest
just verify-fix           # Test TooManyHeaders fix

# Development
just gen                  # Generate from OpenAPI
just build                # Build everything
just fmt                  # Format code
```

## Migration Guide for Contributors

### Before (Scripts)

```bash
# Setup
./scripts/dev-setup.sh

# Test
chmod +x scripts/test-header-limits.sh
./scripts/test-header-limits.sh

# Verify
./scripts/verify-observability.sh

# Teardown
./scripts/dev-teardown.sh
```

### After (Justfile)

```bash
# Setup
just dev-up

# Test
just test

# Verify  
just dev-status

# Teardown
just dev-down
```

## Implementation Notes

### Docker Volumes (Persistent Storage)

**Old approach** (script):
```bash
./scripts/setup-persistent-volumes.sh
```

**New approach** (inline in dev-up):
```makefile
dev-up:
    @docker volume create brrtrouter-prometheus-data || true
    @docker volume create brrtrouter-loki-data || true
    @docker volume create brrtrouter-grafana-data || true
    @docker volume create brrtrouter-jaeger-data || true
    @just dev-registry
    @kind create cluster --config kind-config.yaml --wait 60s || true
    @tilt up
```

### KIND Cluster Setup

**Moved to**: `just dev-up`

Key steps integrated:
1. Create Docker volumes
2. Start Docker registry
3. Create KIND cluster
4. Configure registry in cluster
5. Start Tilt

### Registry Setup

**Moved to**: `just dev-registry`

```makefile
dev-registry:
    #!/usr/bin/env bash
    if [ "$(docker inspect -f '{{.State.Running}}' kind-registry 2>/dev/null || true)" != 'true' ]; then
        docker run -d --restart=always \
            -p "127.0.0.1:5001:5000" \
            --network bridge \
            --name kind-registry \
            registry:2
    fi
```

### Velero CRDs Download

**Moved to**: `just download-velero-crds`

```makefile
download-velero-crds:
    curl -sL https://raw.githubusercontent.com/vmware-tanzu/velero/v1.12.3/config/crd/v1/crds/crds.yaml \
        -o k8s/velero/crds.yaml
```

## Benefits Realized

### Before (Shell Scripts)

❌ Scripts scattered across `scripts/` directory  
❌ Need to `chmod +x` each script  
❌ Hard to discover what's available  
❌ Can't easily compose scripts  
❌ Platform-specific (bash/zsh differences)  
❌ No argument validation  
❌ Verbose error handling needed  

### After (Justfile)

✅ All commands in one place  
✅ `just --list` shows everything  
✅ Auto-discovery of commands  
✅ Recipes call each other easily  
✅ Cross-platform (Just handles it)  
✅ Built-in argument handling  
✅ Clean, readable syntax  

## Testing Migration

### Test Infrastructure

All test scripts consolidated:

```makefile
# Run all tests
test:
    cargo test

# Run with nextest (faster)
nt:
    cargo nextest run

# Test specific component
test-router:
    cargo test router

# Integration tests
test-integration:
    cargo test --test '*_tests'
```

### Removed Test Scripts

- `test-header-limits.sh` → Use `cargo test` with proper test cases
- `verify-tilt-fix.sh` → Test through `just verify-fix` which sends real traffic
- `rebuild-and-test.sh` → `just rebuild-test`

## Documentation Updates

Updated docs to reference `just` commands:

- ✅ `docs/CONTRIBUTING.md` - Use `just` commands
- ✅ `docs/LOCAL_DEVELOPMENT.md` - All examples use `just`
- ✅ `README.md` - Quick start with `just dev-up`
- ✅ `docs/DECLARATIVE_INFRASTRUCTURE_AUDIT.md` - No scripts audit

## Rollback Plan

If needed, scripts are in git history:

```bash
# Restore scripts from git
git checkout HEAD~1 -- scripts/

# Or cherry-pick specific script
git show HEAD~1:scripts/dev-setup.sh > scripts/dev-setup.sh
chmod +x scripts/dev-setup.sh
```

## Future Considerations

### If Scripts Become Necessary Again

**Criteria for adding a script:**
1. Complex logic (>100 lines)
2. Need for functions/libraries
3. Platform-specific behavior
4. Called by CI only (not developers)

**If criteria met:**
- Add to `ci/` directory (not `scripts/`)
- Document why justfile wasn't sufficient
- Keep focused on single purpose

### Tiltfile Extensions

For dev-only automation, prefer Tiltfile extensions:

```python
# Tiltfile
local_resource(
    'setup-volumes',
    cmd='docker volume create brrtrouter-prometheus-data || true',
    auto_init=True,
)
```

## Summary

📁 **Deleted**: `scripts/` directory (17 files)  
📝 **Updated**: `justfile` (added 10+ new recipes)  
🎯 **Result**: Single source of truth for all development commands  

**Commands now discoverable via:**
```bash
just --list
```

**No more:**
```bash
ls scripts/
chmod +x scripts/*.sh
./scripts/some-script.sh --help
```

✅ **Simpler**  
✅ **More discoverable**  
✅ **Better documented**  
✅ **Easier to maintain**  

