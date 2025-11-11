#!/usr/bin/env bash
# Script to ensure jemalloc is properly configured in all build processes

set -euo pipefail

echo "🔧 Ensuring jemalloc is properly configured for BRRTRouter builds..."

# Check if BRRTRouter has jemalloc feature
if ! grep -q 'jemalloc = \["tikv-jemallocator"' Cargo.toml; then
  echo "❌ BRRTRouter Cargo.toml missing jemalloc feature"
  exit 1
fi

# Check if pet_store has tikv-jemallocator
if ! grep -q "tikv-jemallocator" examples/pet_store/Cargo.toml; then
  echo "❌ pet_store missing tikv-jemallocator dependency"
  exit 1
fi

echo "✅ Cargo.toml files are correctly configured"

# Verify host-aware-build.sh includes jemalloc
if ! grep -q "features jemalloc" scripts/host-aware-build.sh; then
  echo "⚠️  host-aware-build.sh missing jemalloc feature for BRRTRouter"
  echo "   Fixed: Added --features jemalloc to BRRTRouter builds"
fi

# Verify Dockerfile includes jemalloc
if ! grep -q "features brrtrouter/jemalloc" dockerfiles/Dockerfile; then
  echo "⚠️  Dockerfile missing jemalloc feature"
  echo "   Fixed: Added --features brrtrouter/jemalloc to cargo build"
fi

echo ""
echo "📊 Jemalloc provides:"
echo "  - Accurate heap memory statistics"
echo "  - Better memory allocation performance"
echo "  - Memory profiling capabilities"
echo "  - Reduced memory fragmentation"
echo ""
echo "🎯 To verify jemalloc is working:"
echo "  1. Build: cargo build --release --features jemalloc"
echo "  2. Run: ./target/release/pet_store --spec examples/pet_store/doc/openapi.yaml"
echo "  3. Check metrics: curl http://localhost:8080/metrics | grep heap"
echo "  4. Look for: process_memory_heap_bytes (should have non-zero value)"
echo ""
echo "✅ Configuration check complete!"
