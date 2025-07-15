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
