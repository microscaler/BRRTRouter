# 🎉 Welcome, New Contributor!

This guide will get you from zero to fully productive in **5 minutes**.

## ✅ Prerequisites Checklist

Before starting, install these tools:

- [ ] **Docker** - Container runtime ([Install](https://docs.docker.com/get-docker/))
- [ ] **kind** - Kubernetes in Docker ([Install](https://kind.sigs.k8s.io/docs/user/quick-start/))
- [ ] **kubectl** - Kubernetes CLI ([Install](https://kubernetes.io/docs/tasks/tools/))
- [ ] **Tilt** - Dev environment automation ([Install](https://docs.tilt.dev/install.html))
- [ ] **just** - Task runner ([Install](https://github.com/casey/just#installation))
- [ ] **Rust** - 1.70+ ([Install](https://rustup.rs/))

### Quick Install (macOS)

```bash
brew install docker kind kubectl tilt just rustup
```

### Quick Install (Linux - Ubuntu/Debian)

```bash
# Docker
sudo apt-get install docker.io

# kubectl
sudo apt-get install kubectl

# kind
curl -Lo ./kind https://kind.sigs.k8s.io/dl/latest/kind-linux-amd64
chmod +x ./kind && sudo mv ./kind /usr/local/bin/

# Tilt
curl -fsSL https://github.com/tilt-dev/tilt/releases/latest/download/tilt.$(uname -s)-$(uname -m).tar.gz | tar -xzv tilt
sudo mv tilt /usr/local/bin/

# just
curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## 🚀 Step-by-Step Setup

### 1. Clone the Repository (30 seconds)

```bash
git clone https://github.com/microscaler/BRRTRouter.git
cd BRRTRouter
```

### 2. Create kind Cluster (1 minute)

```bash
./scripts/dev-setup.sh
```

This creates a local Kubernetes cluster called `brrtrouter-dev` with port mappings for all services.

**Expected output:**
```
✅ Docker is running
✅ kind is installed
✅ kubectl is installed
✅ tilt is installed
🎉 Creating kind cluster 'brrtrouter-dev'...
✅ Cluster created successfully
```

### 3. Start Tilt (2 minutes)

```bash
tilt up
```

**What happens:**
- Compiles BRRTRouter library
- Generates pet_store example
- Cross-compiles to x86_64 Linux
- Builds Docker image
- Deploys to kind cluster
- Starts PostgreSQL, Redis, Prometheus, Grafana, Jaeger

**Expected output (in Tilt UI):**
```
[build-brrtrouter]   ✅ Finished in 15s
[gen-petstore]       ✅ Finished in 3s
[build-petstore]     ✅ Finished in 12s
[petstore]           ✅ Running
[postgres]           ✅ Running
[redis]              ✅ Running
[prometheus]         ✅ Running
[grafana]            ✅ Running
[jaeger]             ✅ Running
```

**Pro Tip**: Press `space` to open the Tilt web UI for a visual dashboard.

### 4. Verify Everything Works (30 seconds)

Open a **new terminal** and run:

```bash
# Health check
curl http://localhost:8080/health
# Expected: {"status":"ok"}

# API test (authenticated)
curl -H "X-API-Key: test123" http://localhost:8080/pets
# Expected: JSON array of pets

# Metrics
curl http://localhost:8080/metrics | head -20
# Expected: Prometheus metrics

# Swagger UI
open http://localhost:8080/docs
# Expected: Interactive API documentation

# Query PostgreSQL
psql -h localhost -U brrtrouter -d brrtrouter
# Password: dev_password_change_in_prod

# Connect to Redis
redis-cli -h localhost -p 6379
```

**If all commands work: 🎉 You're ready to contribute!**

## 🎯 Your First Contribution

### Option A: Fix a Documentation Typo (Easiest)

1. Find a typo in `README.md` or `docs/*.md`
2. Edit the file
3. Run `cargo fmt`
4. Submit a PR

### Option B: Add a Test (Good First Issue)

1. Look for `#[ignore]` tests in `tests/*.rs`
2. Fix the test to work with current API
3. Run `just nt` to verify
4. Submit a PR

### Option C: Improve Generated Code (Intermediate)

1. Edit templates in `templates/*.txt`
2. Regenerate: `just gen`
3. Verify it compiles: `cargo build -p pet_store`
4. Test: `just nt`
5. Submit a PR

### Option D: Add a Feature (Advanced)

1. Check [docs/ROADMAP.md](ROADMAP.md) for planned features
2. Create an issue to discuss your approach
3. Implement with tests
4. Ensure coverage ≥80%: `just coverage`
5. Submit a PR

## 🔄 Daily Development Workflow

### Starting Work

```bash
# Start Tilt (if not already running)
tilt up
```

### Making Changes

```bash
# 1. Edit code in src/ or examples/pet_store/src/
vim src/router/core.rs

# 2. Tilt automatically:
#    - Detects file changes
#    - Rebuilds in ~1-2 seconds
#    - Syncs binary to container
#    - Hot reloads the service

# 3. Test immediately
curl -H "X-API-Key: test123" http://localhost:8080/pets
```

### Checking Logs

```bash
# View service logs
kubectl logs -f -n brrtrouter-dev deployment/petstore

# View build logs (in Tilt UI)
# Press 'space' and click on 'build-petstore'
```

### Running Tests

```bash
# Fast parallel tests (recommended)
just nt

# Standard cargo test
just test

# Check coverage (must be ≥80%)
just coverage
```

### Before Committing

```bash
# Format code
cargo fmt

# Run tests
just nt

# Check docs
just docs

# (Optional) Load test
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 -u10 -r2 -t30s
```

### Stopping Work

```bash
# Stop Tilt (Ctrl-C in Tilt terminal)
# Or gracefully:
tilt down
```

## 📚 Important Resources

| Resource | Link | Purpose |
|----------|------|---------|
| **Local Development Guide** | [docs/LOCAL_DEVELOPMENT.md](LOCAL_DEVELOPMENT.md) | Complete Tilt setup |
| **Architecture** | [docs/ARCHITECTURE.md](ARCHITECTURE.md) | System design |
| **Contributing Guide** | [CONTRIBUTING.md](../CONTRIBUTING.md) | Full workflow |
| **Test Documentation** | [docs/TEST_DOCUMENTATION.md](TEST_DOCUMENTATION.md) | Test suite overview |
| **Roadmap** | [docs/ROADMAP.md](ROADMAP.md) | Future plans |

## 🔧 Troubleshooting

### "kind cluster not found"

```bash
# Recreate cluster
./scripts/dev-setup.sh
```

### "Port already in use"

```bash
# Check what's using the ports
lsof -i :8080 -i :9090 -i :3000 -i :16686

# Stop conflicting services or change ports in kind-config.yaml
```

### "Tilt build failed"

```bash
# Check Tilt logs
tilt logs build-petstore

# Force rebuild
tilt down
tilt up
```

### "Tests failing"

```bash
# Ensure Tilt is running
tilt up

# Run tests with verbose output
cargo test -- --nocapture

# Check service health
curl http://localhost:8080/health
```

### "Cross-compilation failing"

```bash
# Install cargo-zigbuild
cargo install cargo-zigbuild

# Verify zig is installed
zig version

# If zig not found:
brew install zig  # macOS
# or download from https://ziglang.org/download/
```

## 🎓 Learning Path

1. **Week 1**: Get familiar with the codebase
   - Run the pet store example
   - Read [docs/ARCHITECTURE.md](ARCHITECTURE.md)
   - Browse `src/` and understand module structure

2. **Week 2**: Make small improvements
   - Fix documentation typos
   - Add missing doc comments
   - Improve error messages

3. **Week 3**: Add tests
   - Increase coverage in under-tested modules
   - Add integration tests
   - Test edge cases

4. **Week 4+**: Contribute features
   - Pick a feature from the roadmap
   - Discuss approach in an issue
   - Implement with tests and docs

## 🤝 Getting Help

- **GitHub Issues**: [Create an issue](https://github.com/microscaler/BRRTRouter/issues/new)
- **GitHub Discussions**: [Ask a question](https://github.com/microscaler/BRRTRouter/discussions)
- **Code Review**: Mention @microscaler in your PR for review

## 🎉 Welcome to the Team!

We're excited to have you contributing to BRRTRouter. Every contribution matters, whether it's:

- 📝 Fixing typos
- 🐛 Reporting bugs
- ✨ Adding features
- 📊 Improving performance
- 🧪 Writing tests
- 📚 Enhancing documentation

**Thank you for making BRRTRouter better!** 🚀

