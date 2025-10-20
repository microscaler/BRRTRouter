#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   scripts/host-aware-build.sh brr [extra args...]
#   scripts/host-aware-build.sh pet [extra args...]
#
# Selects the correct build strategy based on host OS/arch:
# - macOS: cargo zigbuild --target x86_64-unknown-linux-musl
# - Linux x86_64: cargo build --target x86_64-unknown-linux-musl with musl-gcc linker
#
# The first argument selects which build to perform:
#   brr → build BRRTRouter library
#   pet → build pet_store binary

if [[ $# -lt 1 ]]; then
  echo "usage: $0 [brr|pet] [extra cargo args...]" >&2
  exit 2
fi

target=${1}
shift || true

os_name=$(uname -s || echo unknown)
arch=$(uname -m || echo unknown)

use_zigbuild=true
if [[ ${os_name} == Linux && ${arch} == x86_64 ]]; then
  use_zigbuild=false
fi

if [[ ${target} == "brr" ]]; then
  if [[ ${use_zigbuild} == true ]]; then
    exec cargo zigbuild --release --target x86_64-unknown-linux-musl --lib "$@"
  else
    exec env CC_x86_64_unknown_linux_musl=musl-gcc \
      CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc \
      cargo build --release --target x86_64-unknown-linux-musl --lib "$@"
  fi
elif [[ ${target} == "pet" ]]; then
  if [[ ${use_zigbuild} == true ]]; then
    exec cargo zigbuild --release --features jemalloc --target x86_64-unknown-linux-musl -p pet_store "$@"
  else
    exec env CC_x86_64_unknown_linux_musl=musl-gcc \
      CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc \
      cargo build --release --features jemalloc --target x86_64-unknown-linux-musl -p pet_store "$@"
  fi
else
  echo "unknown build target: ${target} (expected 'brr' or 'pet')" >&2
  exit 3
fi


