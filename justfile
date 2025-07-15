# Tasks for local development

# default list of tasks
default:
    @just --list

# Build the pet store example
gen:
    cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force

# Compile the pet store example
pet-build:
    cd examples/pet_store && cargo build

# Check pet store with clippy
pet-check:
    cd examples/pet_store && cargo clippy -- -D warnings

# Run pet store tests
pet-test:
    cd examples/pet_store && cargo test

# Generate and build pet store in one command
pet-gen-build:
    just gen
    just pet-build

# Generate and check pet store with clippy
pet-gen-check:
    just gen
    just pet-check

# Start the pet store server for functional testing
pet-serve:
    cd examples/pet_store && cargo run --bin pet_store

# Run comprehensive functional tests against the pet store server
pet-curls:
    @echo "🧪 Testing pet store endpoints comprehensively..."
    @echo ""
    @echo "📋 Testing System Endpoints:"
    @echo "  ✅ Health endpoint..."
    curl -s -i 0.0.0.0:8080/health | head -n 10
    @echo ""
    @echo "  ✅ Metrics endpoint..."
    curl -s -i 0.0.0.0:8080/metrics | head -n 10
    @echo ""
    @echo "📋 Testing Pet Endpoints:"
    @echo "  ✅ GET /pets - List all pets..."
    curl -s -i 0.0.0.0:8080/pets | head -n 10
    @echo ""
    @echo "  ✅ POST /pets - Add a new pet..."
    curl -s -i -X POST 0.0.0.0:8080/pets -H "Content-Type: application/json" -d '{"name": "Buddy"}' | head -n 10
    @echo ""
    @echo "  ✅ GET /pets/{id} - Get specific pet..."
    curl -s -i 0.0.0.0:8080/pets/12345 | head -n 10
    @echo ""
    @echo "📋 Testing User Endpoints:"
    @echo "  ✅ GET /users - List all users..."
    curl -s -i 0.0.0.0:8080/users | head -n 10
    @echo ""
    @echo "  ✅ GET /users/{user_id} - Get specific user..."
    curl -s -i 0.0.0.0:8080/users/abc-123 | head -n 10
    @echo ""
    @echo "  ✅ GET /users/{user_id}/posts - List user posts..."
    curl -s -i 0.0.0.0:8080/users/abc-123/posts | head -n 10
    @echo ""
    @echo "  ✅ GET /users/{user_id}/posts/{post_id} - Get specific post..."
    curl -s -i 0.0.0.0:8080/users/abc-123/posts/post1 | head -n 10
    @echo ""
    @echo "📋 Testing Admin Endpoints:"
    @echo "  ✅ GET /admin/settings - Admin settings..."
    curl -s -i 0.0.0.0:8080/admin/settings | head -n 10
    @echo ""
    @echo "📋 Testing Item Endpoints:"
    @echo "  ✅ GET /items/{id} - Get item..."
    curl -s -i 0.0.0.0:8080/items/item-001 | head -n 10
    @echo ""
    @echo "  ✅ POST /items/{id} - Update/create item..."
    curl -s -i -X POST 0.0.0.0:8080/items/item-002 -H "Content-Type: application/json" -d '{"name": "New Item"}' | head -n 10
    @echo ""
    @echo "📋 Testing Event Stream Endpoints:"
    @echo "  ✅ GET /events - Event stream (first 5 seconds)..."
    timeout 5s curl -s -i 0.0.0.0:8080/events || echo "  📡 Event stream test completed (timeout expected)"
    @echo ""
    @echo "📋 Testing Error Handling:"
    @echo "  ✅ GET / - Root endpoint (should return 404)..."
    curl -s -i 0.0.0.0:8080/ | head -n 10
    @echo ""
    @echo "  ✅ GET /nonexistent - Non-existent endpoint..."
    curl -s -i 0.0.0.0:8080/nonexistent | head -n 10
    @echo ""
    @echo "🎉 Comprehensive API testing complete!"

# Complete validation: generate, build, check, and functional test
pet-full-test:
    @echo "🚀 Running complete pet store validation..."
    just pet-gen-build
    just pet-check
    @echo "✅ Compilation and clippy checks passed!"
    @echo "🧪 Starting functional tests..."
    @echo "Note: Start server with 'just pet-serve' in another terminal, then run 'just pet-curls'"

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

# Measure code coverage (requires cargo-llvm-cov)
coverage:
    cargo llvm-cov --no-report

# Run benchmarks
bench:
    cargo bench

# Profile the example server with cargo flamegraph
fg:
    cargo flamegraph -p pet_store --bin pet_store

# Run functional tests (backward compatibility)
curls:
    curl -i 0.0.0.0:8080/health
    echo ""
    curl -i 0.0.0.0:8080/metrics
    echo ""
    curl -i "http://0.0.0.0:8080/items/123?debug=true" -X POST -H "Content-Type: application/json" -d '{"name": "Ball"}'
    echo ""
    curl -i 0.0.0.0:8080

all: gen build test curls

# Run nextest for faster test execution
nextest-test:
    cargo nextest run --workspace --all-targets --fail-fast --retries 1

alias nt := nextest-test
