#!/usr/bin/env bash
set -euox pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
cargo fetch
cargo build -p pet_store "$@"
