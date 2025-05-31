# Tasks for local development

# default list of tasks
default:
    @just --list

# Build the pet store example
gen:
    cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force

# Run tests with output
build:
    cargo build

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
    curl -i 0.0.0.0:8080/metrics
    curl -i "http://0.0.0.0:8080/items/123?debug=true" -X POST -H "Content-Type: application/json" -d '{"name": "Ball"}'

all: gen build test curls
