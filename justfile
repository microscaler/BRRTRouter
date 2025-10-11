# Tasks for local development

# default list of tasks
default:
	@just --list

# Build the SolidJS sample UI and output to pet_store static_site
build-ui:
	@echo "[BUILD] Building SolidJS UI..."
	cd sample-ui && npm install && npm run build:petstore
	@echo "[OK] UI built to examples/pet_store/static_site"

# Build Docker image for curl integration tests (cross-compiles for Linux, instant Docker copy)
build-test-image:
	cargo zigbuild --release -p pet_store --target x86_64-unknown-linux-musl
	mkdir -p build_artifacts
	cp target/x86_64-unknown-linux-musl/release/pet_store build_artifacts/
	docker build -f Dockerfile.test -t brrtrouter-petstore:e2e --rm --force-rm .

# Test that the TooManyHeaders patch is working (sends 100+ headers)
test-headers:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "🧪 Testing TooManyHeaders patch..."
	
	# Build and start server
	cargo build --release -p pet_store
	
	# Start server in background
	RUST_LOG=info cargo run --release -p pet_store -- \
		--spec examples/pet_store/doc/openapi.yaml \
		--test-api-key test123 &
	PID=$!
	
	# Wait for health
	for i in {1..30}; do
		if curl -s http://127.0.0.1:8080/health > /dev/null 2>&1; then
			break
		fi
		sleep 0.5
	done
	
	# Test with many headers
	HEADERS=""
	for i in {1..100}; do
		HEADERS="$HEADERS -H 'X-Test-Header-$i: value$i'"
	done
	
	echo "Sending request with 100+ headers..."
	eval "curl -s -H 'X-API-Key: test123' $HEADERS http://127.0.0.1:8080/health"
	
	# Cleanup
	kill $PID 2>/dev/null || true
	echo "✅ Test complete"

# Rebuild with patched source and run header tests
rebuild-test:
	#!/usr/bin/env bash
	set -euo pipefail
	cargo clean
	echo "✅ Cleaned build artifacts"
	cargo build --release -p pet_store
	echo "✅ Rebuilt with patched may_minihttp"
	just test-headers

# Verify TooManyHeaders fix in Tilt/K8s (THE REAL TEST)
verify-fix:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "🔍 Verifying TooManyHeaders fix in Tilt/K8s..."
	
	# Wait for pod
	kubectl wait --for=condition=ready pod -l app=petstore -n brrtrouter-dev --timeout=60s
	
	# Port forward
	kubectl port-forward -n brrtrouter-dev svc/petstore 8080:8080 &
	PF_PID=$!
	sleep 2
	
	# Test with many headers
	HEADERS=""
	for i in {1..100}; do
		HEADERS="$HEADERS -H 'X-Test-Header-$i: value$i'"
	done
	
	echo "Sending request with 100+ headers to K8s service..."
	eval "curl -s -H 'X-API-Key: test123' $HEADERS http://127.0.0.1:8080/health"
	
	# Cleanup
	kill $PF_PID 2>/dev/null || true
	echo "✅ Tilt/K8s verification complete"

# === Backup & Recovery ===

# Download Velero CRDs (one-time setup)
download-velero-crds:
	#!/usr/bin/env bash
	set -euo pipefail
	echo "📥 Downloading Velero CRDs..."
	curl -sL https://raw.githubusercontent.com/vmware-tanzu/velero/v1.13.0/config/crd/v1/crds/crds.yaml \
		-o k8s/velero-crds.yaml
	echo "✅ Downloaded to k8s/velero-crds.yaml"

# Start MinIO backup server (runs outside KIND)
start-minio:
	@echo "🚀 Starting MinIO backup server..."
	@docker-compose -f docker-compose-minio.yml up -d
	@echo "✅ MinIO running at http://localhost:9001"
	@echo "   Login: minioadmin / minioadmin123"

# Stop MinIO backup server
stop-minio:
	@docker-compose -f docker-compose-minio.yml down

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

# Build the pet store example
gen:
	cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force

# Force-regenerate the pet store example (explicit target)
gen-force:
	cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force

# Run the CLI generate command for any spec
generate spec="examples/openapi.yaml" force="":
	cargo run --bin brrtrouter-gen -- generate --spec {{spec}} {{force}}

# Serve a spec with echo handlers
serve spec="examples/openapi.yaml" addr="0.0.0.0:8080":
	cargo run --bin brrtrouter-gen -- serve --spec {{spec}} --addr {{addr}}

# Serve a spec and watch for changes
watch spec="examples/openapi.yaml" addr="0.0.0.0:8080":
	cargo run --bin brrtrouter-gen -- serve --watch --spec {{spec}} --addr {{addr}}

# Run tests with output
build:
	cargo build
	cargo build --release

# Run tests with output
test:
	cargo test -- --nocapture

# CI-oriented test runs
test-ci:
	cargo test -- --nocapture

# Docker E2E tests (requires Docker on host)
test-e2e-docker:
	RUST_LOG=debug RUST_BACKTRACE=1 E2E_DOCKER=1 cargo test --test docker_integration_tests -- --nocapture

# HTTP integration tests against Dockerized example (no curl required)
test-e2e-http:
	RUST_LOG=debug RUST_BACKTRACE=1 cargo test --test curl_integration_tests -- --test-threads=1 --nocapture

# Convenience: run both E2E suites
e2e:
	just test-e2e-docker
	just test-e2e-http

act:
	E2E_DOCKER=1 act --container-architecture linux/amd64 -P ubuntu-latest=ghcr.io/catthehacker/ubuntu:act-22.04 -W .github/workflows/ci.yml -v

security:
	E2E_DOCKER=1  cargo test --test security_tests -- --nocapture
	
# Measure code coverage (requires cargo-llvm-cov)
coverage:
	cargo llvm-cov --no-report

# Generate and open documentation with Mermaid diagrams
docs:
	cargo doc --no-deps --lib --open

# Generate documentation without opening
docs-build:
	cargo doc --no-deps --lib

# Check documentation for warnings and broken links
docs-check:
	RUSTDOCFLAGS="-D warnings -D rustdoc::broken_intra_doc_links --html-in-header doc/head.html" cargo doc --no-deps --lib

# Run benchmarks
bench:
	cargo bench

# Profile the example server with cargo flamegraph
fg:
	cargo flamegraph -p pet_store --bin pet_store

# Start the Pet Store locally with correct spec/doc/config paths (foreground)
start-petstore:
	RUST_LOG=trace RUST_BACKTRACE=1 cargo run -p pet_store -- --spec doc/openapi.yaml --doc-dir examples/pet_store/doc --config config/config.yaml --test-api-key test123

start-petstore-stack:
	ulimit -n 65536
	# sudo launchctl limit maxfiles 65536 65536
	# sudo sysctl kern.ipc.somaxconn=4096
	# sudo sysctl net.inet.tcp.sendspace=1048576 net.inet.tcp.recvspace=1048576
	BRRTR_STACK_SIZE=0x4000  RUST_LOG=trace RUST_BACKTRACE=1 cargo run -p pet_store -- --spec doc/openapi.yaml --doc-dir examples/pet_store/doc --config examples/pet_store/config/config.yaml --test-api-key test123


# Start the example in background and then run curls (uses correct paths)
curls-start:
	@echo "Starting example server with test API key..."
	@RUST_LOG=trace RUST_BACKTRACE=1 cargo run --manifest-path examples/pet_store/Cargo.toml -- --spec doc/openapi.yaml --doc-dir examples/pet_store/doc --config examples/pet_store/config/config.yaml --test-api-key test123 &
	@echo "Waiting for server readiness on /health..."
	@for i in $$(seq 1 60); do \
		code=$$(curl -s -o /dev/null -w "%{http_code}" http://0.0.0.0:8080/health || true); \
		[ "$$code" = "200" ] && break; \
		sleep 0.5; \
	done
	@echo "Server ready. Running curls..."
	@just curls

# Self-contained curls (no parameters needed)
curls:
	# Infra
	curl -i "http://0.0.0.0:8080/health"
	echo ""
	curl -i "http://0.0.0.0:8080/metrics"
	echo ""

	# Pets
	curl -i -H "X-API-Key: test123" "http://0.0.0.0:8080/pets"
	echo ""
	curl -i -H "X-API-Key: test123" -H "Content-Type: application/json" -d '{"name":"Bella"}' "http://0.0.0.0:8080/pets"
	echo ""
	curl -i -H "X-API-Key: test123" "http://0.0.0.0:8080/pets/12345"
	echo ""

	# Users
	curl -i -H "X-API-Key: test123" "http://0.0.0.0:8080/users?limit=10&offset=0"
	echo ""
	curl -i -H "X-API-Key: test123" "http://0.0.0.0:8080/users/abc-123"
	echo ""
	curl -I -H "X-API-Key: test123" "http://0.0.0.0:8080/users/abc-123"   # HEAD
	echo ""
	curl -i -X OPTIONS -H "X-API-Key: test123" "http://0.0.0.0:8080/users/abc-123"
	echo ""
	curl -i -X DELETE -H "X-API-Key: test123" "http://0.0.0.0:8080/users/abc-123"
	echo ""

	# Posts
	curl -i -H "X-API-Key: test123" "http://0.0.0.0:8080/users/abc-123/posts?limit=5&offset=0"
	echo ""
	curl -i -H "X-API-Key: test123" "http://0.0.0.0:8080/users/abc-123/posts/post1"
	echo ""

	# Admin
	curl -i -H "X-API-Key: test123" "http://0.0.0.0:8080/admin/settings"
	echo ""

	# Items
	curl -i -H "X-API-Key: test123" "http://0.0.0.0:8080/items/550e8400-e29b-41d4-a716-446655440000"
	echo ""
	curl -i -H "X-API-Key: test123" -H "Content-Type: application/json" -X POST -d '{"name":"New Item"}' "http://0.0.0.0:8080/items/550e8400-e29b-41d4-a716-446655440000"
	echo ""

	# SSE (avoid blocking)
	curl -i -sS -m 2 -H "X-API-Key: test123" "http://0.0.0.0:8080/events" | head -n 1 || true
	echo ""

	# Download (JSON meta variant)
	curl -i -H "X-API-Key: test123" "http://0.0.0.0:8080/download/550e8400-e29b-41d4-a716-446655440000"
	echo ""

	# Form URL-encoded
	curl -i -H "X-API-Key: test123" -H "Content-Type: application/x-www-form-urlencoded" --data-urlencode "name=John" --data-urlencode "age=30" "http://0.0.0.0:8080/form"
	echo ""

	# Upload multipart (use a local file as payload)
	curl -i -H "X-API-Key: test123" -F "file=@Cargo.toml;type=text/plain" -F "metadata.note=example" "http://0.0.0.0:8080/upload"
	echo ""

	# Matrix and label style
	curl -i -H "X-API-Key: test123" "http://0.0.0.0:8080/matrix/;coords=1,2,3"
	echo ""
	curl -i -H "X-API-Key: test123" "http://0.0.0.0:8080/labels/.red"
	echo ""

	# Search with complex query, header and cookie
	curl -i -H "X-API-Key: test123" -H "X-Trace-Id: 123e4567-e89b-12d3-a456-426614174000" --cookie "session=abc" \
	  "http://0.0.0.0:8080/search?tags=a%7Cb%7Cc&filters%5Bname%5D=Bella&filters%5Btag%5D=pet"
	echo ""

	# Secure endpoint (requires API key)
	# This is a fake token, it will not work outside the example
	curl -i -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30=.sig" http://0.0.0.0:8080/secure
	echo ""

	# Webhook registration
	curl -i -H "X-API-Key: test123" -H "Content-Type: application/json" -d '{"url":"https://example.com/webhook"}' "http://0.0.0.0:8080/webhooks"
	echo ""

all: gen build test curls

# Run nextest for faster test execution
nextest-test:
	cargo nextest run --workspace --all-targets --fail-fast --retries 1

alias nt := nextest-test

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
# Tilt UI runs on port 10351 by default (configurable in Tiltfile)
# To use a different port: TILT_PORT=10352 just dev-up
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
		kind create cluster --config kind-config.yaml --wait 60s
		
		# Document the local registry
		kubectl apply -f k8s/local-registry-hosting.yaml
		echo "[OK] Kind cluster created"
	else
		echo "[OK] Kind cluster already exists"
	fi
	echo ""
	
	# Start Tilt
	echo "Starting Tilt (press 'space' to open web UI)..."
	tilt up

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
	@cargo build --release
	@cargo build --release -p pet_store
	@echo "Restarting Tilt..."
	@tilt down || true
	@tilt up
