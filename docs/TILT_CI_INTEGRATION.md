# Tilt CI Integration in GitHub Actions

## Overview

We've added a comprehensive Kubernetes integration testing job using Tilt CI in our GitHub Actions workflow. This provides full end-to-end testing of the entire stack in a real Kubernetes environment.

## What It Does

The `tilt-ci` job:
1. ✅ Creates a real `kind` Kubernetes cluster
2. ✅ Builds all binaries (cross-compiled for Linux)
3. ✅ Deploys full stack via `tilt ci`:
   - Pet Store API
   - PostgreSQL
   - Redis
   - Prometheus
   - Grafana
   - Jaeger
   - OTEL Collector
   - Loki & Promtail
4. ✅ Waits for all services to be healthy
5. ✅ Tests API functionality
6. ✅ Verifies observability stack
7. ✅ Validates per-path metrics

## Benefits Over Docker-Based Tests

| Aspect | Docker Tests | Tilt CI Tests |
|--------|--------------|---------------|
| **Environment** | Single container | Full Kubernetes cluster |
| **Services** | Pet Store only | All 8 services |
| **Networking** | Docker network | Kubernetes services |
| **Configuration** | Docker env vars | ConfigMaps |
| **Observability** | None | Full stack (Prometheus, Grafana, Jaeger, Loki) |
| **Production Parity** | Low | High |

## What Gets Tested

### API Functionality
```bash
✅ Health endpoint
✅ Metrics endpoint
✅ Authenticated endpoints (with API key)
✅ Per-path metrics collection
```

### Observability Stack
```bash
✅ Prometheus health
✅ Grafana health
✅ OTEL Collector deployment
✅ Loki & Promtail deployment
```

### Infrastructure
```bash
✅ PostgreSQL deployment
✅ Redis deployment
✅ Kubernetes services
✅ ConfigMap configuration
✅ Pod health checks
```

## Workflow Position

```
build-and-test (unit tests, clippy, docs)
    ↓
    ├─→ tilt-ci (Kubernetes integration) ← NEW!
    ├─→ e2e-docker (Docker integration)
    └─→ perf-wrk (performance profiling)
         └─→ goose-load-test (load testing)
```

The `tilt-ci` job runs in parallel with `e2e-docker`, providing complementary test coverage:
- **`tilt-ci`**: Tests production-like Kubernetes deployment
- **`e2e-docker`**: Tests standalone Docker container

## Technical Details

### Cluster Creation
Uses `helm/kind-action@v1` to create a kind cluster:
```yaml
- name: Create kind cluster
  uses: helm/kind-action@v1
  with:
    cluster_name: brrtrouter-ci
    wait: 60s
```

### Tilt Installation
```bash
curl -fsSL https://raw.githubusercontent.com/tilt-dev/tilt/master/scripts/install.sh | bash
```

### Build Process
1. Cross-compile with `cargo-zigbuild` for Linux (`x86_64-unknown-linux-musl`)
2. Generate Pet Store from OpenAPI spec
3. Build Pet Store binary
4. Stage artifacts in `build_artifacts/`

### Tilt CI Execution
```bash
tilt ci --timeout 10m
```

**What `tilt ci` does:**
1. Reads `Tiltfile`
2. Builds Docker images
3. Loads images into kind cluster
4. Applies all Kubernetes manifests
5. Waits for pod readiness probes
6. Waits for liveness probes
7. Exits 0 if all healthy, non-zero if any failures

### Port Forwarding for Tests
```bash
kubectl port-forward -n brrtrouter-dev service/petstore 8080:8080 &
curl -f http://localhost:8080/health
```

## Timeout Configuration

- **Tilt CI**: 10-minute timeout
- **Overall Job**: 12-minute timeout (buffer for cleanup)
- **Kind Cluster**: 60-second wait for readiness

## Artifacts

### Tilt Logs
```yaml
- name: Upload Tilt logs
  uses: actions/upload-artifact@v4
  with:
    name: tilt-ci-logs
    path: ~/.tilt-dev/
```

Contains:
- Tilt execution logs
- Build output
- Deployment history
- Error messages

### Kubernetes Logs
Always collected (even on failure):
- Pet Store application logs (last 100 lines)
- OTEL Collector logs (last 50 lines)
- Full resource listing (`kubectl get all`)

## Failure Handling

The job includes `if: always()` conditions to ensure diagnostic information is collected even when tests fail:

```yaml
- name: Check deployment status
  if: always()
  run: |
    kubectl get all -n brrtrouter-dev
    kubectl logs -n brrtrouter-dev deployment/petstore --tail=100
```

## Performance Characteristics

**Typical execution time:**
- Cluster creation: ~1-2 minutes
- Binary builds: ~3-5 minutes
- Tilt CI deployment: ~2-4 minutes
- API tests: ~30 seconds
- Observability tests: ~30 seconds
- **Total: ~8-12 minutes**

## Environment Variables

```yaml
env:
  RUST_BACKTRACE: "1"
  RUST_LOG: "info"
```

- `RUST_BACKTRACE`: Enable backtraces for debugging
- `RUST_LOG`: Set log level (info to avoid excessive output)

## Dependencies

### Required Tools (Installed by Job)
- ✅ Rust stable toolchain
- ✅ `x86_64-unknown-linux-musl` target
- ✅ `musl-tools` (musl-gcc for linking)
- ✅ `cargo-zigbuild` (cross-compilation)
- ✅ Tilt CLI
- ✅ kind (via helm/kind-action)
- ✅ kubectl (via helm/kind-action)

### Optional Tools
- ✅ yarn (for sample UI build, if present)

## Local Reproduction

To reproduce the CI environment locally:

```bash
# 1. Create kind cluster
kind create cluster --name brrtrouter-ci

# 2. Build binaries
cargo zigbuild --release --target x86_64-unknown-linux-musl
cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force
cargo zigbuild --release --target x86_64-unknown-linux-musl -p pet_store
mkdir -p build_artifacts
cp target/x86_64-unknown-linux-musl/release/pet_store build_artifacts/

# 3. Run Tilt CI
tilt ci --timeout 10m

# 4. Test
kubectl port-forward -n brrtrouter-dev service/petstore 8080:8080 &
curl http://localhost:8080/health
curl http://localhost:8080/metrics | grep brrtrouter_path

# 5. Cleanup
tilt down
kind delete cluster --name brrtrouter-ci
```

## Future Enhancements

### Planned Additions
1. **Metrics Validation**: Query Prometheus for specific metrics
2. **Log Aggregation**: Verify Loki received logs
3. **Trace Validation**: Check Jaeger for traces
4. **Load Testing**: Run Goose tests against Kubernetes deployment
5. **Chaos Testing**: Inject failures and verify recovery

### Potential Optimizations
1. **Cache kind images**: Speed up cluster creation
2. **Parallel builds**: Build binaries and create cluster simultaneously
3. **Incremental updates**: Use `tilt up` for faster iterations
4. **Resource limits**: Reduce service resources for faster scheduling

## Troubleshooting

### Common Issues

**Tilt CI timeout:**
```bash
# Check pod status
kubectl get pods -n brrtrouter-dev

# Check pod events
kubectl describe pod -n brrtrouter-dev <pod-name>

# Check logs
kubectl logs -n brrtrouter-dev deployment/petstore
```

**Image not found:**
```bash
# Verify image was loaded into kind
docker exec brrtrouter-ci-control-plane crictl images
```

**Service not ready:**
```bash
# Check service endpoints
kubectl get endpoints -n brrtrouter-dev

# Check pod readiness
kubectl get pods -n brrtrouter-dev -o wide
```

## References

- [Tilt CI Documentation](https://docs.tilt.dev/ci.html)
- [kind in CI](https://kind.sigs.k8s.io/docs/user/quick-start/#creating-a-cluster)
- [helm/kind-action](https://github.com/helm/kind-action)

---

**Status**: ✅ Implemented in `.github/workflows/ci.yml`  
**Job Name**: `tilt-ci`  
**Execution Time**: ~8-12 minutes  
**Runs**: On every PR and push to main  
**Dependencies**: `build-and-test` job must pass  
**Date**: October 9, 2025

