# Tasks for local development

# default list of tasks
default:
    @just --list

# Build the pet store example
genpet:
    cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force

# Run tests with output
build:
    cargo build

# Run tests with output
test:
    cargo test -- --nocapture

# Measure code coverage (requires cargo-llvm-cov)
coverage:
    cargo llvm-cov --fail-under 80 --no-report

# Run benchmarks
bench:
    cargo bench
