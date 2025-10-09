# Local Development with Tilt + kind

This guide explains how to set up and use the BRRTRouter local development environment with Tilt and kind (Kubernetes in Docker).

## Overview

The local development setup provides:

- **Fast iteration**: Local Rust builds sync to containers in ~1-2 seconds
- **Full observability**: Prometheus, Grafana, Jaeger, and OpenTelemetry collector
- **Production-like environment**: Run in Kubernetes locally
- **Hot reload**: Automatic rebuild and deployment on code changes
- **No macOS reliability issues**: Service runs reliably in kind cluster

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      kind Cluster                            │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Namespace: brrtrouter-dev                             │ │
│  │                                                         │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌─────────────┐ │ │
│  │  │   Pet Store  │  │  Prometheus  │  │   Grafana   │ │ │
│  │  │   :8080      │  │   :9090      │  │   :3000     │ │ │
│  │  └──────────────┘  └──────────────┘  └─────────────┘ │ │
│  │                                                         │ │
│  │  ┌──────────────┐  ┌──────────────┐                   │ │
│  │  │    Jaeger    │  │     OTEL     │                   │ │
│  │  │   :16686     │  │  Collector   │                   │ │
│  │  └──────────────┘  └──────────────┘                   │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
         ↑                ↑               ↑               ↑
         │                │               │               │
    localhost:8080   localhost:9090  localhost:3000  localhost:16686
```

## Prerequisites

Install the following tools:

### macOS

```bash
# Docker Desktop
brew install --cask docker

# Kubernetes in Docker
brew install kind

# kubectl
brew install kubectl

# Tilt
brew install tilt

# Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSF https://sh.rustup.rs | sh
```

### Linux

```bash
# Docker
# Follow: https://docs.docker.com/engine/install/

# kind
curl -Lo ./kind https://kind.sigs.k8s.io/dl/latest/kind-linux-amd64
chmod +x ./kind
sudo mv ./kind /usr/local/bin/kind

# kubectl
# Follow: https://kubernetes.io/docs/tasks/tools/install-kubectl-linux/

# Tilt
curl -fsSL https://raw.githubusercontent.com/tilt-dev/tilt/master/scripts/install.sh | bash

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Quick Start

### 1. Create kind Cluster

```bash
./scripts/dev-setup.sh
```

This script will:
- Check prerequisites
- Create a kind cluster named `brrtrouter-dev`
- Configure port mappings
- Set kubectl context

### 2. Start Tilt

```bash
# Option 1: Direct command
tilt up

# Option 2: Using justfile
just dev-up
```

Press **space** in the terminal to open the Tilt web UI.

### 3. Access Services

Once Tilt reports all services are ready (green checkmarks):

| Service | URL | Credentials |
|---------|-----|-------------|
| **Pet Store API** | http://localhost:8080 | X-API-Key: test123 |
| **Grafana** | http://localhost:3000 | admin / admin |
| **Prometheus** | http://localhost:9090 | None |
| **Jaeger UI** | http://localhost:16686 | None |

### 4. Test the API

```bash
# Health check
curl http://localhost:8080/health

# List pets (requires API key)
curl -H "X-API-Key: test123" http://localhost:8080/pets

# Run all curl tests
just curls
```

### 5. Stop Development Environment

```bash
# Stop Tilt (Ctrl+C in Tilt terminal, or:)
tilt down

# Tear down kind cluster
./scripts/dev-teardown.sh

# Or use justfile
just dev-down
```

## Development Workflow

### Fast Iteration Cycle

1. **Edit Rust source code** in `src/` or `examples/pet_store/src/`
2. **Save the file**
3. Tilt automatically:
   - Runs `cargo build --release` locally (fast incremental compilation)
   - Syncs the binary to the container (~1-2 seconds)
   - Restarts the service
4. **Test immediately** at http://localhost:8080

### Regenerate from OpenAPI Spec

```bash
# Option 1: Automatic (Tilt watches templates/ and examples/openapi.yaml)
# Just edit the spec or templates and save

# Option 2: Manual trigger
# In Tilt UI, click the "regenerate-petstore" button

# Option 3: Command line
cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force
```

### Run Tests

```bash
# In Tilt UI, click "run-curl-tests" button

# Or from command line:
just curls

# Run Goose load test
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  --users 10 \
  --hatch-rate 2 \
  --run-time 30s
```

### View Logs

Logs are streaming in real-time in the Tilt UI. You can also:

```bash
# View petstore logs
kubectl logs -n brrtrouter-dev -l app=petstore -f

# View all pods
kubectl get pods -n brrtrouter-dev

# Describe petstore deployment
kubectl describe deployment -n brrtrouter-dev petstore
```

## Observability

### Metrics (Prometheus)

1. Open http://localhost:9090
2. Try queries:
   ```promql
   # Request rate
   rate(brrtrouter_requests_total[5m])
   
   # Response latency
   histogram_quantile(0.95, rate(brrtrouter_request_duration_seconds_bucket[5m]))
   
   # Error rate
   rate(brrtrouter_errors_total[5m])
   ```

### Dashboards (Grafana)

1. Open http://localhost:3000
2. Login with `admin` / `admin`
3. Navigate to **Dashboards** → **BRRTRouter Pet Store**
4. See real-time metrics:
   - Request rate
   - Response latency (p50, p95)
   - Error rate
   - Active connections

### Traces (Jaeger)

1. Open http://localhost:16686
2. Select service: **petstore**
3. Click **Find Traces**
4. Explore distributed traces showing:
   - Request flow
   - Timing breakdown
   - Span details

## Troubleshooting

### Port Already in Use

If you see "port 8080 already in use":

```bash
# Find and kill process using port 8080
lsof -ti:8080 | xargs kill -9

# Or use a different port (edit k8s/cluster/kind-config.yaml)
```

### kind Cluster Won't Start

```bash
# Delete and recreate
kind delete cluster --name brrtrouter-dev
./scripts/dev-setup.sh
```

### Tilt Build Fails

```bash
# Check Rust compilation locally first
cargo check --all

# Clean and rebuild
cargo clean
tilt down
tilt up
```

### Service Not Responding

```bash
# Check pod status
kubectl get pods -n brrtrouter-dev

# Check pod logs
kubectl logs -n brrtrouter-dev -l app=petstore

# Restart deployment
kubectl rollout restart deployment/petstore -n brrtrouter-dev
```

### Slow Builds

```bash
# Ensure you're using release builds (faster runtime)
cargo build --release

# Check Tilt is using live_update (not full rebuilds)
# Look for "Syncing files" in Tilt UI logs

# Clear Tilt cache
rm -rf .tilt-cache/
```

## Advanced

### Customize Resource Limits

Edit `k8s/petstore-deployment.yaml`:

```yaml
resources:
  requests:
    memory: "256Mi"  # Increase for larger workloads
    cpu: "200m"
  limits:
    memory: "1Gi"
    cpu: "2000m"
```

### Add Custom Middleware

1. Edit `src/middleware/mod.rs`
2. Save (Tilt rebuilds automatically)
3. Binary syncs to container
4. Service restarts
5. Test immediately

### Debug with `RUST_LOG`

Edit `k8s/petstore-deployment.yaml`:

```yaml
env:
  - name: RUST_LOG
    value: "trace"  # or debug, info, warn, error
```

Apply changes:

```bash
kubectl apply -f k8s/petstore-deployment.yaml
```

### Multiple Environments

Run multiple kind clusters:

```bash
# Edit k8s/cluster/kind-config.yaml and change cluster name
name: brrtrouter-staging

# Create with different name
kind create cluster --config k8s/cluster/kind-config.yaml

# Switch contexts
kubectl config use-context kind-brrtrouter-staging
```

## Comparison: Tilt vs cargo run

| Feature | Tilt + kind | cargo run |
|---------|-------------|-----------|
| **Reliability on macOS** | ✅ Always works | ⚠️ Intermittent issues |
| **Observability** | ✅ Full stack included | ❌ Manual setup |
| **Hot Reload** | ✅ ~1-2 seconds | ⚠️ Full restart |
| **Production Parity** | ✅ Kubernetes | ❌ Local process |
| **Setup Time** | ~2 minutes | ~10 seconds |
| **Resource Usage** | ~2GB RAM | ~200MB RAM |

## Tips and Best Practices

1. **Use Tilt UI**: Press `space` after `tilt up` to open the web interface
2. **Watch Logs**: Keep Tilt UI open to see real-time logs and build status
3. **Manual Triggers**: Use buttons in Tilt UI for on-demand actions
4. **Resource Labels**: Tilt organizes resources by labels (`build`, `app`, `observability`, `tools`)
5. **Parallel Builds**: Library builds happen in parallel with Docker operations
6. **Clean Slate**: Run `just dev-down && just dev-up` for a fresh environment

## Next Steps

- [Architecture Documentation](./ARCHITECTURE.md)
- [Testing Documentation](./TEST_DOCUMENTATION.md)
- [Load Testing with Goose](./GOOSE_LOAD_TESTING.md)
- [Contributing Guidelines](../CONTRIBUTING.md)

## Getting Help

If you encounter issues:

1. Check Tilt UI logs for error messages
2. Run `kubectl get pods -n brrtrouter-dev` to see pod status
3. Check this troubleshooting guide
4. Open an issue on GitHub with:
   - Tilt logs
   - `kubectl describe pod <pod-name> -n brrtrouter-dev` output
   - Your OS and tool versions

