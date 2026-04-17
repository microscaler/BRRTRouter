# Development Guide

This guide covers the development workflow, common tasks, and best practices for working on BRRTRouter.

## Prerequisites

- Rust 1.75+
- Docker
- kind (Kubernetes in Docker)
- kubectl
- Tilt

## Common Tasks

```bash
just dev-up           # Start development environment (kind + Tilt)
just test             # Run test suite
just nt               # Fast parallel tests with nextest (recommended)
just curls            # Test all API endpoints
just coverage         # Run tests with coverage (≥80% required)
just bench            # Run performance benchmarks
just docs             # Generate and open documentation
just build-ui         # Build SolidJS dashboard (auto-run by Tilt)
just dev-down         # Stop everything
```

## Development Workflow

1. **Edit code** in `src/` or `examples/pet_store/src/`
2. **Tilt auto-rebuilds** and syncs (~1-2s)
3. **Test immediately** with dashboard, curl, or Swagger UI
4. **View logs**: `kubectl logs -f -n brrtrouter-dev deployment/petstore`
5. **Check metrics**: http://localhost:3000 (Grafana)
6. **Trace requests**: http://localhost:16686 (Jaeger)

See [CONTRIBUTING.md](../CONTRIBUTING.md) for detailed development guide.

## Quick Reference

### Service URLs (when Tilt is running)

| Service | URL | Purpose |
|---------|-----|---------|
| **🎨 Interactive Dashboard** | http://localhost:8081/ | **START HERE** - SolidJS UI with live data, SSE, API testing |
| **Pet Store API** | http://localhost:8081 | Main API (local-dev default; k8s still uses 8080 via `PORT` env) |
| **Swagger UI** | http://localhost:8081/docs | OpenAPI documentation |
| **Health Check** | http://localhost:8081/health | Readiness probe |
| **Metrics** | http://localhost:8081/metrics | Prometheus metrics |
| **Grafana** | http://localhost:3000 | Dashboards (admin/admin) |
| **Prometheus** | http://localhost:9090 | Metrics database |
| **Jaeger** | http://localhost:16686 | Distributed tracing |
| **PostgreSQL** | localhost:5432 | Database (user: brrtrouter, db: brrtrouter, pass: dev_password) |
| **Redis** | localhost:6379 | Cache/session store |
| **Tilt Web UI** | http://localhost:10353 | Dev dashboard (press 'space' in terminal) |

### Environment Variables

BRRTRouter reads `BRRTR_STACK_SIZE` to determine the stack size for coroutines. The value can be a decimal number or a hex string like `0x8000`. If unset, the default stack size is `0x8000` (32 KiB).

## Working with Generated Code

**IMPORTANT**: Files under `examples/pet_store/` are **auto-generated**. Do not edit them directly!

### Generator Architecture

- **Generator logic**: `src/generator/`
- **Templates**: `templates/`
- **OpenAPI spec**: `examples/openapi.yaml`
- **Output**: `examples/pet_store/` (generated)

### Modifying Generated Code

1. Edit templates in `templates/` or generator logic in `src/generator/`
2. Regenerate the pet store example:
   ```bash
   just gen
   # or
   cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force
   ```
3. Verify the generated code compiles: `cargo build -p pet_store`
4. Run tests: `just nt`
5. Commit both template changes AND regenerated files together

## Testing

See [docs/TEST_DOCUMENTATION.md](TEST_DOCUMENTATION.md) for comprehensive testing guide.

## Performance

See [docs/PERFORMANCE.md](PERFORMANCE.md) for performance benchmarks and optimization guides.

## Related Documentation

- [Local Development](LOCAL_DEVELOPMENT.md) - Setting up the development environment
- [Contributing](../CONTRIBUTING.md) - Contribution guidelines
- [Architecture](ARCHITECTURE.md) - System design
- [Tilt Implementation](TILT_IMPLEMENTATION.md) - Development environment architecture
- [Generator: Impl and Dependencies Analysis](GENERATOR_IMPL_AND_DEPENDENCIES_ANALYSIS.md) - Impl directory generation and Cargo.toml dependencies config (cross-repo capture)

