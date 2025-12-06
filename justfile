# BRRTRouter Development Tasks
# Simplified for Tilt-based workflow

# Show available tasks
default:
	@just --list

# ============================================================================
# Building & Code Generation
# ============================================================================

# Build the SolidJS sample UI and output to pet_store static_site
build-ui:
	@echo "[BUILD] Building SolidJS UI..."
	cd sample-ui && npm install && npm run build:petstore
	@echo "[OK] UI built to examples/pet_store/static_site"

# Build Docker image for curl integration tests (cross-compiles for Linux)
build-test-image:
	cargo zigbuild --release -p pet_store --target x86_64-unknown-linux-musl
	mkdir -p build_artifacts
	cp target/x86_64-unknown-linux-musl/release/pet_store build_artifacts/
	docker build -f dockerfiles/Dockerfile.test -t brrtrouter-petstore:e2e --rm --force-rm .

# Generate code from OpenAPI spec (pet store example)
gen:
	cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force

# Generate code from any OpenAPI spec
generate spec="examples/openapi.yaml" force="":
	cargo run --bin brrtrouter-gen -- generate --spec {{spec}} {{force}}

# Serve a spec with echo handlers
serve spec="examples/openapi.yaml" addr="0.0.0.0:8080":
	cargo run --bin brrtrouter-gen -- serve --spec {{spec}} --addr {{addr}}

# Serve a spec and watch for changes (hot reload)
watch spec="examples/openapi.yaml" addr="0.0.0.0:8080":
	cargo run --bin brrtrouter-gen -- serve --watch --spec {{spec}} --addr {{addr}}

# ============================================================================
# Debug & Troubleshooting
# ============================================================================

# Start pet_store server with full debug logging and backtrace (logs to /tmp/petstore_debug.log)
debug-petstore:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "🔍 Starting pet_store with debug logging..."
	pkill -f "target/release/pet_store" 2>/dev/null || true
	sleep 1
	
	# Build release with debug symbols
	cargo build --release -p pet_store
	
	cd examples/pet_store && \
	RUST_BACKTRACE=full \
	RUST_LOG=debug,may=warn,may_minihttp=warn \
	BRRTR_STACK_SIZE=0x100000 \
	BRRTR_LOG_LEVEL=debug \
	BRRTR_LOG_FORMAT=json \
	../../target/release/pet_store \
		--spec doc/openapi.yaml \
		--config config/config.yaml \
		--static-dir static_site \
		--doc-dir doc \
		--test-api-key test123 2>&1 | tee /tmp/petstore_debug.log &
	
	sleep 3
	echo ""
	echo "✅ Server started in background"
	echo "   Log file: /tmp/petstore_debug.log"
	echo "   Health:   curl http://127.0.0.1:8080/health"
	echo "   Stop:     pkill -f pet_store"
	echo ""
	echo "Verifying server is running..."
	curl -s http://127.0.0.1:8080/health && echo ""

# Stop pet_store server
stop-petstore:
	@pkill -f "target/release/pet_store" 2>/dev/null && echo "✅ Server stopped" || echo "⚠️ Server not running"

# Watch pet_store debug logs live
watch-petstore:
	@tail -f /tmp/petstore_debug.log

# Show last 100 lines of pet_store logs
logs-petstore:
	@tail -100 /tmp/petstore_debug.log

# Run Goose load test against local server (logs to /tmp/goose_test.log)
goose-test users="10" runtime="30s":
	#!/usr/bin/env bash
	set -euo pipefail
	echo "🦆 Running Goose load test..."
	echo "   Users:   {{users}}"
	echo "   Runtime: {{runtime}}"
	echo "   Target:  http://127.0.0.1:8080"
	echo "   Log:     /tmp/goose_test.log"
	echo ""
	
	# Check if server is running
	if ! curl -s http://127.0.0.1:8080/health > /dev/null 2>&1; then
		echo "❌ Server not responding on port 8080"
		echo "   Start it first with: just debug-petstore"
		exit 1
	fi
	
	RUST_LOG=warn \
	cargo run --release --example api_load_test -- \
		--host http://127.0.0.1:8080 \
		--users {{users}} \
		--run-time {{runtime}} \
		--no-reset-metrics \
		--report-file /tmp/goose_report.html \
		2>&1 | tee /tmp/goose_test.log
	
	echo ""
	echo "✅ Goose test complete"
	echo "   Log:    /tmp/goose_test.log"
	echo "   Report: /tmp/goose_report.html"

# Quick smoke test - start server, run brief load test, stop
smoke-test:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "🔥 Running smoke test..."
	
	# Start server
	just debug-petstore
	sleep 2
	
	# Run brief load test
	just goose-test users=5 runtime=10s || true
	
	# Check if server crashed
	if ! curl -s http://127.0.0.1:8080/health > /dev/null 2>&1; then
		echo ""
		echo "❌ SERVER CRASHED during load test!"
		echo "   Check logs: tail -50 /tmp/petstore_debug.log"
		exit 1
	fi
	
	echo ""
	echo "✅ Smoke test passed - server survived load test"

# ============================================================================
# Testing & Quality
# ============================================================================

# Run all tests
test:
	cargo test -- --nocapture

# Run tests with nextest (faster, parallel execution)
nt:
	cargo nextest run --workspace --all-targets

# Run tests with code coverage
coverage:
	cargo llvm-cov --html
	@echo "Coverage report generated in target/llvm-cov/html/index.html"

# Run benchmarks
bench:
	cargo bench


# ============================================================================
# Instrumentation & Profiling (cargo-instruments)
# ============================================================================

# Profile with Time Profiler (default) - shows where CPU time is spent
profile:
	@echo "🔬 Profiling pet_store with Time Profiler..."
	@echo "Building release binary with debug symbols..."
	cargo build --release --features jemalloc
	cd examples/pet_store && cargo build --release
	@echo "Starting profiling (will run for ~10 seconds)..."
	cargo instruments -t "Time Profiler" \
		-p pet_store \
		--bin pet_store \
		--release \
		--no-open \
		--time-limit 10000 \
		--output target/instruments/time-profile \
		-- --spec examples/pet_store/doc/openapi.yaml \
		   --addr 127.0.0.1:8090 \
		   --config examples/pet_store/config/config.yaml \
		   --static-dir examples/pet_store/static_site \
		   --doc-dir examples/pet_store/doc
	@echo "✅ Profile saved to target/instruments/time-profile.trace"
	@echo "Run 'open target/instruments/time-profile.trace' to view in Instruments"

# Profile memory allocations - track heap allocations and deallocations
profile-alloc:
	@echo "🔬 Profiling memory allocations..."
	cargo build --release --features jemalloc
	cd examples/pet_store && cargo build --release
	cargo instruments -t "Allocations" \
		-p pet_store \
		--bin pet_store \
		--release \
		--no-open \
		--time-limit 10000 \
		--output target/instruments/allocations \
		-- --spec examples/pet_store/doc/openapi.yaml \
		   --addr 127.0.0.1:8090 \
		   --test-api-key test123
	@echo "✅ Profile saved to target/instruments/allocations.trace"

# Profile for memory leaks - detect unreleased memory
profile-leaks:
	@echo "🔬 Checking for memory leaks..."
	cargo build --release --features jemalloc
	cd examples/pet_store && cargo build --release
	cargo instruments -t "Leaks" \
		-p pet_store \
		--bin pet_store \
		--release \
		--no-open \
		--time-limit 15000 \
		--output target/instruments/leaks \
		-- --spec examples/pet_store/doc/openapi.yaml \
		   --addr 127.0.0.1:8090 \
		   --test-api-key test123
	@echo "✅ Profile saved to target/instruments/leaks.trace"

# Profile system calls - see what system resources are being used
profile-syscalls:
	@echo "🔬 Profiling system calls..."
	cargo build --release --features jemalloc
	cd examples/pet_store && cargo build --release
	cargo instruments -t "System Trace" \
		-p pet_store \
		--bin pet_store \
		--release \
		--no-open \
		--time-limit 10000 \
		--output target/instruments/syscalls \
		-- --spec examples/pet_store/doc/openapi.yaml \
		   --addr 127.0.0.1:8090 \
		   --test-api-key test123
	@echo "✅ Profile saved to target/instruments/syscalls.trace"

# Profile with Activity Monitor - track CPU, memory, disk, network usage
profile-activity:
	@echo "🔬 Monitoring system activity..."
	cargo build --release --features jemalloc
	cd examples/pet_store && cargo build --release
	cargo instruments -t "Activity Monitor" \
		-p pet_store \
		--bin pet_store \
		--release \
		--no-open \
		--time-limit 10000 \
		--output target/instruments/activity \
		-- --spec examples/pet_store/doc/openapi.yaml \
		   --addr 127.0.0.1:8090 \
		   --test-api-key test123
	@echo "✅ Profile saved to target/instruments/activity.trace"

# List all available instrument templates
profile-list:
	@echo "📋 Available Instruments templates:"
	@cargo instruments --list-templates

# Clean up instrument traces
profile-clean:
	@echo "🧹 Cleaning instrument traces..."
	rm -rf target/instruments
	@echo "✅ Cleaned"

# Run pet_store under load and profile simultaneously
profile-load:
	@echo "🔬 Profiling under load..."
	@echo "Building release binary..."
	cargo build --release --features jemalloc
	cd examples/pet_store && cargo build --release
	@echo "Starting profiling with load generation..."
	# Start the server with profiling in background
	cargo instruments -t "Time Profiler" \
		-p pet_store \
		--bin pet_store \
		--release \
		--no-open \
		--time-limit 30000 \
		--output target/instruments/load-profile \
		-- --spec examples/pet_store/doc/openapi.yaml \
		   --addr 127.0.0.1:8090 \
		   --test-api-key test123 &
	# Wait for server to start
	@sleep 3
	# Generate load
	@echo "Generating load..."
	@for i in {1..5}; do \
		(curl -s -H "x-api-key: test123" http://127.0.0.1:8090/pets > /dev/null &); \
		(curl -s -H "x-api-key: test123" http://127.0.0.1:8090/users > /dev/null &); \
		(curl -s -H "x-api-key: test123" http://127.0.0.1:8090/metrics > /dev/null &); \
	done
	@echo "Load generation started. Waiting for profile to complete..."
	@wait
	@echo "✅ Profile saved to target/instruments/load-profile.trace"

# ============================================================================
# Documentation
# ============================================================================

# Generate and open documentation with Mermaid diagrams
docs:
	cargo doc --no-deps --lib --open

# Generate documentation without opening
docs-build:
	cargo doc --no-deps --lib

# Check documentation for warnings and broken links
docs-check:
	RUSTDOCFLAGS="-D warnings -D rustdoc::broken_intra_doc_links --html-in-header doc/head.html" cargo doc --no-deps --lib

# ============================================================================
# API Testing
# ============================================================================

# Test Pet Store API endpoints (requires running server on localhost:8080)
# Use with: just dev-up (in another terminal), then just curls
curls:
	@bash scripts/curls.sh

# ============================================================================
# Backup & Recovery (Velero)
# ============================================================================

# Download Velero CRDs (one-time setup)
download-velero-crds:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "📥 Downloading Velero CRDs..."
	curl -sL https://raw.githubusercontent.com/vmware-tanzu/velero/v1.13.0/config/crd/v1/crds/crds.yaml \
		-o k8s/velero/crds.yaml
	echo "✅ Downloaded to k8s/velero/crds.yaml"

# Start MinIO backup server (runs outside KIND)
start-minio:
	@echo "🚀 Starting MinIO backup server..."
	@docker-compose -f k8s/velero/docker-compose-minio.yml up -d
	@echo "✅ MinIO running at http://localhost:9001"
	@echo "   Login: minioadmin / minioadmin123"

# Stop MinIO backup server
stop-minio:
	@docker-compose -f k8s/velero/docker-compose-minio.yml down

# Create manual backup now
backup-now:
	@velero backup create brrtrouter-manual-$(shell date +%Y%m%d-%H%M%S) \
		--include-namespaces brrtrouter-dev \
		--wait

# List all backups
backup-list:
	@velero backup get

# Restore from backup (usage: just backup-restore <backup-name>)
backup-restore name:
	@velero restore create restore-$(shell date +%Y%m%d-%H%M%S) \
		--from-backup {{name}} \
		--wait

# Backup before major operations
backup-before-upgrade:
	@velero backup create pre-upgrade-$(shell date +%Y%m%d-%H%M%S) \
		--include-namespaces brrtrouter-dev \
		--labels purpose=upgrade \
		--wait
	@echo "✅ Pre-upgrade backup created"

# ============================================================================
# Local Development with Tilt + kind
# ============================================================================

# Start local Docker registry for project images (needed for Tilt)
dev-registry:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "Starting local Docker registry..."
	
	# Check if registry is already running
	RUNNING=$(docker inspect -f '{''{ .State.Running }''}' kind-registry 2>/dev/null || echo false)
	if [ "$RUNNING" = "true" ]; then
		echo "[OK] Registry already running"
		exit 0
	fi
	
	# Ensure kind network exists
	docker network create kind 2>/dev/null || true
	
	# Start registry on kind network (for local project images only)
	docker run -d --restart=always \
		-p "127.0.0.1:5001:5000" \
		--network kind \
		--name kind-registry \
		registry:2
	
	echo "[OK] Registry started on localhost:5001"
	echo "     Registry connected to kind network"
	echo "     For local project images (localhost:5001/...)"

# Verify local registry setup
dev-registry-verify:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "Verifying local registry..."
	
	# Check if running
	RUNNING=$(docker inspect -f '{''{ .State.Running }''}' kind-registry 2>/dev/null || echo false)
	if [ "$RUNNING" != "true" ]; then
		echo "[FAIL] Registry not running"
		exit 1
	fi
	
	# Check connectivity
	if curl -s http://127.0.0.1:5001/v2/_catalog > /dev/null; then
		echo "[OK] Registry accessible on localhost:5001"
	else
		echo "[FAIL] Registry not accessible"
		exit 1
	fi
	
	# Check if connected to kind
	if docker network inspect kind >/dev/null 2>&1; then
		if docker network inspect kind | grep -q kind-registry; then
			echo "[OK] Registry connected to kind network"
		else
			echo "[WARN] Registry not on kind network (will connect on cluster creation)"
		fi
	fi

# Verify observability stack (metrics, logs, traces)
dev-observability-verify:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "Verifying observability stack..."
	
	# Check pods
	echo ""
	echo "Pods in brrtrouter-dev:"
	kubectl get pods -n brrtrouter-dev -l component=observability
	
	# Check each component
	for component in prometheus grafana jaeger loki promtail otel-collector; do
		if kubectl get pod -n brrtrouter-dev -l app=$component 2>/dev/null | grep -q Running; then
			echo "[OK] $component running"
		else
			echo "[FAIL] $component not running"
		fi
	done

# Start local development environment (kind + Tilt)
# Tilt UI: http://localhost:10351 (press 'space' to open)
# Pet Store API: http://localhost:8080
# Grafana: http://localhost:3000
# Prometheus: http://localhost:9090
# Jaeger: http://localhost:16686
dev-up:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "Starting BRRTRouter development environment..."
	echo ""
	
	# Create persistent Docker volumes
	echo "Creating persistent Docker volumes..."
	docker volume create brrtrouter-prometheus-data 2>/dev/null && echo "[OK] Prometheus volume" || echo "[OK] Prometheus volume exists"
	docker volume create brrtrouter-loki-data 2>/dev/null && echo "[OK] Loki volume" || echo "[OK] Loki volume exists"
	docker volume create brrtrouter-grafana-data 2>/dev/null && echo "[OK] Grafana volume" || echo "[OK] Grafana volume exists"
	docker volume create brrtrouter-jaeger-data 2>/dev/null && echo "[OK] Jaeger volume" || echo "[OK] Jaeger volume exists"
	echo ""
	
	# Start Docker registry (for local project images)
	just dev-registry
	echo ""
	
	# Create kind cluster if it doesn't exist
	if ! kind get clusters 2>/dev/null | grep -q '^brrtrouter-dev$'; then
		echo "Creating kind cluster..."
		kind create cluster --config k8s/cluster/kind-config.yaml --wait 60s
		
		# Document the local registry
		kubectl apply -f k8s/core/local-registry-hosting.yaml
		echo "[OK] Kind cluster created"
	else
		echo "[OK] Kind cluster already exists"
	fi
	echo ""
	
	# Start Tilt
	echo "Starting Tilt (press 'space' to open web UI)..."
	tilt up

# Stop local development environment
dev-down:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "Stopping Tilt..."
	tilt down || true
	echo ""
	
	echo "Deleting kind cluster..."
	kind delete cluster --name brrtrouter-dev || true
	echo ""
	
	echo "Stopping Docker registry..."
	docker stop kind-registry 2>/dev/null || true
	docker rm kind-registry 2>/dev/null || true
	echo ""
	
	echo "[OK] Development environment stopped"
	echo "Note: Persistent volumes preserved (run 'docker volume prune' to remove)"

# Check development environment status
dev-status:
	@echo "KIND cluster status:"
	@kind get clusters | grep brrtrouter-dev || echo "[FAIL] Cluster not found"
	@echo ""
	@echo "Kubernetes pods:"
	@kubectl get pods -n brrtrouter-dev 2>/dev/null || echo "[FAIL] Namespace not found"
	@echo ""
	@echo "Services:"
	@kubectl get svc -n brrtrouter-dev 2>/dev/null || echo "[FAIL] No services found"

# Rebuild and redeploy (useful after major changes)
dev-rebuild:
	@echo "Rebuilding all components..."
	@cargo clean
	@cargo build --release -p pet_store
	@echo "Restarting Tilt..."
	@tilt down || true
	@tilt up

# Clean Docker state (fixes KIND cluster creation issues)
dev-clean:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "Cleaning Docker state for KIND..."
	echo ""
	
	# Stop any running KIND clusters
	echo "Stopping KIND clusters..."
	kind delete clusters --all 2>/dev/null || true
	
	# Kill any orphaned containers
	echo "Killing orphaned containers..."
	docker kill $(docker ps -q) 2>/dev/null || true
	
	# Remove all stopped containers
	echo "Removing stopped containers..."
	docker rm -f $(docker ps -aq) 2>/dev/null || true
	
	# Remove KIND network
	echo "Removing KIND network..."
	docker network rm kind 2>/dev/null || true
	
	# Remove registry if it exists
	echo "Removing registry container..."
	docker rm -f kind-registry 2>/dev/null || true
	
	# Prune system (keeps volumes)
	echo "Pruning Docker system..."
	docker system prune -f
	
	echo ""
	echo "[OK] Docker state cleaned"
	echo "[OK] Volumes preserved for data persistence"
	echo ""
	echo "Now run: just dev-up"
