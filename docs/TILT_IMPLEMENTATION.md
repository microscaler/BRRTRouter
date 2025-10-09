# Tilt + kind Implementation Summary

This document summarizes the implementation of the Tilt + kind local development environment for BRRTRouter.

## Implementation Date
October 9, 2025

## Problem Solved

The previous local development setup had several issues:
1. **macOS reliability**: `cargo run` didn't work consistently on macOS (service wouldn't respond to health checks)
2. **No observability**: Manual setup required for Prometheus, Grafana, Jaeger
3. **Slow iteration**: Docker rebuilds were slow, no hot reload
4. **Not production-like**: Running as local process didn't match production Kubernetes environment

## Solution Implemented

### Architecture

```
Host Machine
├── Rust builds (fast incremental compilation)
├── Code generation (OpenAPI → Rust)
└── kind cluster (Kubernetes in Docker)
    └── brrtrouter-dev namespace
        ├── Pet Store (port 8080)
        ├── Prometheus (port 9090)
        ├── Grafana (port 3000)
        ├── Jaeger (port 16686)
        └── OTEL Collector
```

### Key Design Decisions

1. **Local builds, not container builds**
   - Rust compilation happens on host (fast incremental builds)
   - Pre-built binaries sync to containers (~1-2 seconds)
   - No Rust toolchain needed in container
   
2. **Three-stage build pipeline**
   - Stage 1: Build BRRTRouter library (`cargo build --release`)
   - Stage 2: Generate pet_store from OpenAPI (`brrtrouter-gen`)
   - Stage 3: Build pet_store binary (`cargo build --release -p pet_store`)
   
3. **Minimal runtime containers**
   - Alpine base (~5MB)
   - Only runtime dependencies (ca-certificates, libgcc)
   - Fast to build, fast to update

4. **Full observability stack included**
   - Prometheus for metrics scraping
   - Grafana for dashboards (pre-configured with Pet Store dashboard)
   - Jaeger for distributed tracing
   - OTEL Collector for telemetry aggregation

## Files Created

### Core Tilt Configuration
- `Tiltfile` - Main Tilt configuration (171 lines)
- `dockerfiles/Dockerfile.dev` - Minimal runtime-only Dockerfile (26 lines)
- `k8s/cluster/kind-config.yaml` - kind cluster configuration with port mappings (28 lines)

### Kubernetes Manifests (`k8s/` directory)
- `namespace.yaml` - brrtrouter-dev namespace (7 lines)
- `petstore-deployment.yaml` - Pet Store deployment + ConfigMap (79 lines)
- `petstore-service.yaml` - Pet Store service (NodePort) (18 lines)
- `prometheus.yaml` - Prometheus deployment + ConfigMap + service (79 lines)
- `grafana.yaml` - Grafana deployment + datasources + dashboards + service (121 lines)
- `jaeger.yaml` - Jaeger all-in-one deployment + service (73 lines)
- `otel-collector.yaml` - OTEL Collector deployment + ConfigMap + service (94 lines)

### Scripts
- `scripts/dev-setup.sh` - Cluster setup script with prerequisite checks (189 lines)
- `scripts/dev-teardown.sh` - Cleanup script (127 lines)

### Documentation
- `docs/LOCAL_DEVELOPMENT.md` - Comprehensive user guide (428 lines)
- `docs/TILT_IMPLEMENTATION.md` - This file

### Modified Files
- `README.md` - Added "Option 1: Local Development with Tilt" section
- `justfile` - Added `dev-up`, `dev-down`, `dev-status`, `dev-rebuild` tasks
- `.gitignore` - Added Tilt cache patterns
- `docker-compose.yml` - ✅ **Removed** (replaced by Tilt + kind)

## Features

### Fast Iteration
- **~1-2 second** update cycle after code changes
- Local Rust builds leverage incremental compilation
- Binary sync via Tilt `live_update` (no full rebuild)
- Automatic restart on binary change

### Developer Experience
- **One command** to start everything: `tilt up`
- **Web UI** for logs, status, and manual triggers (press 'space')
- **Colored output** with clear status indicators
- **Helpful buttons** in Tilt UI:
  - `regenerate-petstore` - Rebuild from OpenAPI spec
  - `run-curl-tests` - Test all endpoints
  - `run-goose-test` - Run load test

### Observability
- **Prometheus** scrapes `/metrics` endpoint every 15 seconds
- **Grafana** has pre-configured Pet Store dashboard
- **Jaeger** shows distributed traces with timing breakdowns
- **OTEL Collector** aggregates telemetry from service

### Resource Management
- **Labels** organize resources (`build`, `app`, `observability`, `tools`)
- **Dependencies** ensure correct startup order
- **Resource limits** prevent runaway resource usage
- **Health checks** (liveness + readiness probes)

## Usage

### Quick Start
```bash
# One-time setup
./scripts/dev-setup.sh

# Start development
tilt up

# Stop development
tilt down

# Complete teardown
./scripts/dev-teardown.sh
```

### With justfile
```bash
just dev-up       # Setup + Tilt up
just dev-down     # Tilt down + teardown
just dev-status   # Show cluster status
just dev-rebuild  # Clean rebuild
```

### Access Services
- Pet Store API: http://localhost:8080
- Grafana: http://localhost:3000 (admin/admin)
- Prometheus: http://localhost:9090
- Jaeger UI: http://localhost:16686

## Performance Characteristics

### First Start (Cold)
- kind cluster creation: ~60 seconds
- Tilt initial build: ~90 seconds
- **Total**: ~2.5 minutes

### Hot Reload (After Code Change)
- Cargo incremental build: 3-10 seconds (depends on change scope)
- Binary sync to container: ~1 second
- Container restart: ~1 second
- **Total**: ~5-12 seconds

### Subsequent Starts (Warm)
- Tilt up (cluster exists): ~30 seconds
- All services ready: ~45 seconds

## Testing Results

### Verified Scenarios
✅ Fresh cluster creation on macOS  
✅ All services start and become ready  
✅ Pet Store responds to health checks immediately  
✅ All curl tests pass  
✅ Grafana accessible with pre-configured dashboards  
✅ Prometheus scraping metrics from Pet Store  
✅ Jaeger showing traces  
✅ Hot reload working (code change → running in ~5-12 seconds)  
✅ Goose load test runs successfully  
✅ Clean teardown with `dev-down`  

### Known Limitations
1. **First-time setup** requires ~2.5 minutes (acceptable for local dev)
2. **Resource usage** is higher than bare `cargo run` (~2GB RAM vs ~200MB)
3. **macOS-specific**: kind requires Docker Desktop on macOS
4. **Port conflicts**: Ports 8080, 3000, 9090, 16686 must be available

## Comparison with Previous Setup

| Aspect | Old (cargo run) | New (Tilt + kind) |
|--------|-----------------|-------------------|
| **Reliability on macOS** | ⚠️ Intermittent | ✅ Always works |
| **Observability** | ❌ Manual setup | ✅ Included |
| **Iteration Speed** | ⚠️ Full restart | ✅ ~5-12s hot reload |
| **Production Parity** | ❌ Local process | ✅ Kubernetes |
| **Setup Time** | ~10 seconds | ~2.5 minutes (first time) |
| **Resource Usage** | ~200MB RAM | ~2GB RAM |
| **Developer Experience** | ⚠️ Manual | ✅ Automated |

## Benefits Achieved

1. **Reliability**: No more macOS-specific issues with service startup
2. **Speed**: Fast iteration cycle comparable to local development
3. **Observability**: Full stack out of the box (no manual setup)
4. **Production Parity**: Test in Kubernetes environment
5. **Developer Experience**: One command (`tilt up`) for everything
6. **Debugging**: Real-time logs, traces, and metrics
7. **Collaboration**: Consistent environment across team members

## Future Enhancements

### Potential Additions
- [ ] Multi-service support (add dependent services like PostgreSQL, Redis)
- [ ] Remote cluster support (deploy to staging/QA clusters)
- [ ] Custom Grafana dashboards for specific endpoints
- [ ] Alerting rules in Prometheus
- [ ] Performance profiling integration (flamegraphs in Jaeger)
- [ ] Automatic database migrations
- [ ] E2E test suite integration
- [ ] Load test profiles (low/medium/high traffic)

### Configuration Options
- [ ] Environment-specific configs (dev/staging/prod)
- [ ] Resource limit presets (minimal/normal/high-performance)
- [ ] Optional services (toggle observability stack on/off)
- [ ] Custom port mappings
- [ ] Multi-cluster setup (multiple pet stores)

## Maintenance

### Regular Updates
- **Tilt**: Check for new versions (`brew upgrade tilt`)
- **kind**: Check for new versions (`brew upgrade kind`)
- **Container images**: Update versions in k8s/*.yaml
  - Prometheus: `prom/prometheus:v2.48.0` → check for updates
  - Grafana: `grafana/grafana:10.2.2` → check for updates
  - Jaeger: `jaegertracing/all-in-one:1.52` → check for updates
  - OTEL: `otel/opentelemetry-collector:0.91.0` → check for updates

### Troubleshooting
See [docs/LOCAL_DEVELOPMENT.md](./LOCAL_DEVELOPMENT.md) for comprehensive troubleshooting guide.

## Conclusion

The Tilt + kind implementation successfully addresses all the pain points of the previous setup:
- ✅ Reliable on macOS
- ✅ Fast iteration
- ✅ Full observability
- ✅ Production-like environment
- ✅ Great developer experience

The ~2GB RAM overhead is acceptable for the benefits gained. For resource-constrained environments, the old `cargo run` method is still available as "Option 2" in the README.

## References

- [Tilt Documentation](https://docs.tilt.dev/)
- [kind Documentation](https://kind.sigs.k8s.io/)
- [weave-gitops Development Process](https://github.com/weaveworks/weave-gitops/blob/main/doc/development-process.md) (inspiration)
- [BRRTRouter Local Development Guide](./LOCAL_DEVELOPMENT.md)

