# Tasks for local development

# default list of tasks
default:
	@just --list

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
	BRRTR_STACK_SIZE=0x4000  RUST_LOG=trace RUST_BACKTRACE=1 cargo run -p pet_store -- --spec doc/openapi.yaml --doc-dir examples/pet_store/doc --config config/config.yaml --test-api-key test123


# Start the example in background and then run curls (uses correct paths)
curls-start:
	@echo "Starting example server with test API key..."
	@RUST_LOG=trace RUST_BACKTRACE=1 cargo run --manifest-path examples/pet_store/Cargo.toml -- --spec doc/openapi.yaml --doc-dir examples/pet_store/doc --config config/config.yaml --test-api-key test123 &
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
