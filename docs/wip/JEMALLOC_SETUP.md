# Jemalloc Setup for BRRTRouter

## Overview

BRRTRouter and pet_store use `tikv-jemallocator` for improved memory management and profiling capabilities. This document covers the setup and verification of jemalloc integration across the build pipeline.

## Configuration Status

### ✅ Completed Updates

1. **BRRTRouter Library** (`Cargo.toml`)
   - Added `tikv-jemallocator` and `tikv-jemalloc-ctl` as optional dependencies
   - Created `jemalloc` feature flag
   - Global allocator configured in `src/lib.rs` when feature enabled

2. **Pet Store Example** (`examples/pet_store/Cargo.toml`)
   - Has `tikv-jemallocator = { version = "0.5", features = ["profiling"] }`
   - Uses its own global allocator in `main.rs`
   - Template updated to conditionally include allocator

3. **Build Scripts**
   - `scripts/host-aware-build.sh`: Updated to include `--features jemalloc` for BRRTRouter
   - `dockerfiles/Dockerfile`: Updated to include `--features brrtrouter/jemalloc`
   - `dockerfiles/Dockerfile.dev`: Uses pre-built binary from host

4. **Templates** (`templates/main.rs.txt`)
   - Conditionally includes jemalloc allocator
   - Prevents conflicts when BRRTRouter provides allocator

## Build Commands

### Local Development

```bash
# Build BRRTRouter with jemalloc
cargo build --release --features jemalloc

# Build pet_store (automatically uses jemalloc)
cd examples/pet_store
cargo build --release

# Using host-aware build script (for Tilt)
./scripts/host-aware-build.sh brr  # Builds BRRTRouter with jemalloc
./scripts/host-aware-build.sh pet  # Builds pet_store
```

### Docker Build

```bash
# Standard Docker build (includes jemalloc)
docker build -f dockerfiles/Dockerfile -t brrtrouter-petstore .

# Development build (uses pre-built binary)
docker build -f dockerfiles/Dockerfile.dev -t brrtrouter-petstore:dev .
```

### Tilt Development

```bash
# Tilt automatically uses host-aware-build.sh
tilt up

# The build pipeline:
# 1. host-aware-build.sh brr → builds BRRTRouter with jemalloc
# 2. host-aware-build.sh pet → builds pet_store
# 3. Binary copied to build_artifacts/
# 4. Docker image built with pre-built binary
```

## Jemalloc Benefits

### 1. Memory Profiling
- Accurate heap statistics via `tikv-jemalloc-ctl`
- Memory allocation tracking
- Leak detection capabilities

### 2. Performance
- Better memory allocation performance
- Reduced fragmentation
- Lower memory overhead
- Better multi-threaded performance

### 3. Observability
When jemalloc is enabled with BRRTRouter's jemalloc feature:
- `process_memory_heap_bytes`: Actual heap usage
- `process_memory_allocations`: Total allocation count
- Memory growth tracking
- Per-handler memory attribution (future)

## Verification

### 1. Build Verification

```bash
# Run verification script
./scripts/ensure-jemalloc.sh

# Expected output:
✅ Cargo.toml files are correctly configured
✅ Configuration check complete!
```

### 2. Runtime Verification

```bash
# Start pet_store
./target/release/pet_store --spec examples/pet_store/doc/openapi.yaml

# Check metrics
curl http://localhost:8080/metrics | grep memory

# Look for:
process_memory_rss_bytes <value>
process_memory_vss_bytes <value>
process_memory_peak_rss_bytes <value>
```

### 3. Heap Metrics (when available)

With full jemalloc integration:
```bash
# Check heap-specific metrics
curl http://localhost:8080/metrics | grep heap

# When working correctly:
process_memory_heap_bytes <non-zero-value>
process_memory_allocations <count>
```

## Current Limitations

### Pet Store Allocator

Pet store currently uses its own jemalloc allocator, which means:
- Memory tracking works at process level (RSS, VSS)
- Heap-specific metrics require additional integration
- Both BRRTRouter and pet_store have jemalloc benefits

### Musl Target

When building for `x86_64-unknown-linux-musl`:
- Static linking provides portability
- Jemalloc is statically linked into the binary
- No external dependencies required

## Troubleshooting

### Issue: Heap metrics not showing

**Cause**: Pet store has its own allocator that may not expose metrics.

**Solution**: The memory middleware uses `memory-stats` crate for cross-platform memory monitoring, which provides RSS and VSS. Heap metrics require jemalloc-ctl integration.

### Issue: Build fails with allocator conflict

**Cause**: Both BRRTRouter and pet_store trying to define global allocator.

**Solution**: The template now uses conditional compilation:
```rust
#[cfg(not(feature = "brrtrouter/jemalloc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;
```

### Issue: Docker build not using jemalloc

**Cause**: Missing feature flag in Docker build command.

**Solution**: Updated Dockerfile includes:
```dockerfile
cargo build --release --features brrtrouter/jemalloc -p pet_store
```

## Future Enhancements

1. **Heap Profiling**
   - Enable `prof:true` in jemalloc
   - Generate heap profiles with `jeprof`
   - Visualize allocation patterns

2. **Memory Attribution**
   - Track allocations per handler
   - Identify memory-hungry endpoints
   - Optimize based on usage patterns

3. **Grafana Integration**
   - Heap usage panels
   - Allocation rate metrics
   - Memory efficiency tracking

## References

- [tikv-jemallocator](https://github.com/tikv/jemallocator)
- [jemalloc documentation](http://jemalloc.net/jemalloc.3.html)
- [Memory profiling in Rust](https://github.com/jemalloc/jemalloc/wiki/Use-Case:-Heap-Profiling)
