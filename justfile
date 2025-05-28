# Tasks for local development

# default list of tasks
default:
    @just --list

# Build the pet store example
build-pet-store:
    ./scripts/build_pet_store.sh

# Run tests with output
test:
    cargo test -- --nocapture

# Measure code coverage (requires cargo-tarpaulin)
coverage:
    cargo tarpaulin --fail-under 80
