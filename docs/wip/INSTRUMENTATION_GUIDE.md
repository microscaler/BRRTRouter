# BRRTRouter Instrumentation & Profiling Guide

## Overview

BRRTRouter provides comprehensive instrumentation and profiling capabilities using:
- **cargo-instruments**: macOS-native profiling with Xcode Instruments
- **jemalloc**: Advanced heap profiling and memory statistics
- **memory-stats**: Cross-platform memory monitoring
- **Custom telemetry**: Built-in metrics and logging

## Prerequisites

### macOS (Required for cargo-instruments)
```bash
# Install Xcode Command Line Tools
xcode-select --install

# Install cargo-instruments
cargo install cargo-instruments

# Verify installation
cargo instruments --version
```

### Memory Profiling Setup
BRRTRouter uses `tikv-jemallocator` for accurate heap statistics:
- Enabled via `--features jemalloc` when building BRRTRouter
- Pet Store example includes jemalloc by default
- Provides heap allocation tracking and leak detection

## Quick Start

### Basic CPU Profiling
```bash
# Profile CPU usage for 10 seconds
just profile

# View the results
open target/instruments/time-profile.trace
```

### Memory Leak Detection
```bash
# Check for memory leaks
just profile-leaks

# View leak report
open target/instruments/leaks.trace
```

## Available Profiling Commands

All profiling commands are available through the justfile:

| Command | Purpose | Duration | Output |
|---------|---------|----------|--------|
| `just profile` | CPU time profiling | 10s | `target/instruments/time-profile.trace` |
| `just profile-alloc` | Heap allocations tracking | 10s | `target/instruments/allocations.trace` |
| `just profile-leaks` | Memory leak detection | 15s | `target/instruments/leaks.trace` |
| `just profile-syscalls` | System call analysis | 10s | `target/instruments/syscalls.trace` |
| `just profile-activity` | System resource monitoring | 10s | `target/instruments/activity.trace` |
| `just profile-load` | Profile under load | 30s | `target/instruments/load-profile.trace` |
| `just profile-list` | List available templates | - | Console output |
| `just profile-clean` | Clean trace files | - | Removes `target/instruments/` |

## Profiling Templates

### Time Profiler (Default)
Shows where CPU time is spent in your application.
```bash
just profile
```
**Use for**: Identifying hot paths, optimizing algorithms, finding CPU bottlenecks

### Allocations
Tracks all heap allocations and deallocations.
```bash
just profile-alloc
```
**Use for**: Understanding memory usage patterns, identifying excessive allocations

### Leaks
Detects memory that was allocated but never freed.
```bash
just profile-leaks
```
**Use for**: Finding memory leaks, identifying zombie coroutines

### System Trace
Monitors system calls and kernel interactions.
```bash
just profile-syscalls
```
**Use for**: I/O optimization, understanding system resource usage

### Activity Monitor
Comprehensive system resource tracking.
```bash
just profile-activity
```
**Use for**: Overall system impact, resource consumption analysis

## Understanding Results

### Time Profiler Output
- **Call Tree**: Shows function call hierarchy with time percentages
- **Heavy Stack Trace**: Identifies the hottest code paths
- **Thread Timeline**: Visualizes thread activity over time

### Memory Profiler Output
- **Allocation List**: All allocations with sizes and stack traces
- **Leak Report**: Unreferenced memory blocks
- **Growth Chart**: Memory usage over time

## Integration with BRRTRouter Telemetry

### Built-in Memory Metrics
BRRTRouter provides real-time memory metrics at `/metrics`:
```bash
curl http://localhost:8080/metrics | grep memory
```

Metrics include:
- `process_memory_rss_bytes`: Resident set size
- `process_memory_vss_bytes`: Virtual memory size
- `process_memory_heap_bytes`: Heap usage (with jemalloc)
- `process_memory_allocations`: Total allocation count

### Coroutine Stack Usage
BRRTRouter tracks actual stack usage (not just allocation):
```bash
# Enable stack usage tracking (automatically done)
export BRRTR_STACK_SIZE=65537  # Odd number enables tracking
```

### Grafana Dashboards
Memory metrics are visualized in Grafana:
- Memory Usage Over Time
- Memory Growth Rate
- Per-Handler Memory Impact
- Leak Detection Alerts

## Performance Optimization Workflow

### 1. Baseline Measurement
```bash
# Get baseline performance
just profile
open target/instruments/time-profile.trace
```

### 2. Load Testing
```bash
# Profile under realistic load
just profile-load
```

### 3. Memory Analysis
```bash
# Check for leaks
just profile-leaks

# Analyze allocation patterns
just profile-alloc
```

### 4. Optimization
Based on profiling results:
- Optimize hot paths identified in Time Profiler
- Reduce allocations in frequently called functions
- Fix any memory leaks detected
- Adjust coroutine stack sizes if needed

### 5. Verification
```bash
# Re-run profiles to verify improvements
just profile
just profile-alloc
```

## Common Issues & Solutions

### Issue: "xctrace exited with error"
**Solution**: Ensure the output directory exists:
```bash
mkdir -p target/instruments
```

### Issue: Global allocator conflict
**Solution**: The template now conditionally includes jemalloc only when needed.

### Issue: No heap metrics in /metrics
**Solution**: Build with jemalloc feature:
```bash
cargo build --release --features jemalloc
```

### Issue: Profile shows no symbols
**Solution**: Ensure debug symbols are included:
```toml
[profile.release]
debug = true
```

## Advanced Usage

### Custom Profiling Duration
Modify the `--time-limit` parameter in justfile recipes:
```bash
# Change from 10000ms to 30000ms
--time-limit 30000
```

### Profile Specific Scenarios
Add custom arguments to the pet_store invocation:
```bash
cargo instruments -t "Time Profiler" \
    -p pet_store \
    --bin pet_store \
    -- --spec your-spec.yaml \
       --hot-reload  # Enable hot reload during profiling
```

### Continuous Profiling
For long-running analysis:
```bash
# Remove time limit for manual stop
cargo instruments -t "Time Profiler" \
    -p pet_store \
    --bin pet_store \
    -- --spec examples/pet_store/doc/openapi.yaml
```

## Best Practices

1. **Always profile release builds**: Debug builds have different performance characteristics
2. **Use realistic workloads**: Profile with actual API traffic patterns
3. **Profile regularly**: Make it part of your development workflow
4. **Compare profiles**: Save baselines to track improvements/regressions
5. **Focus on hot paths**: Optimize the code that runs most frequently
6. **Monitor memory growth**: Even small leaks accumulate over time

## Integration with CI/CD

While cargo-instruments requires macOS, you can:
1. Run profiling on macOS CI runners
2. Export metrics to monitoring systems
3. Set up alerts for performance regressions
4. Archive trace files for historical comparison

## Related Documentation

- [Memory Middleware](../src/middleware/memory.rs): Real-time memory tracking
- [Runtime Config](../src/runtime_config.rs): Stack size configuration
- [Grafana Dashboards](../k8s/observability/grafana-dashboards.yaml): Memory visualization
- [Performance Tips](../src/lib.rs): Optimization guidelines

## Conclusion

BRRTRouter's instrumentation provides deep visibility into:
- CPU usage and hot paths
- Memory allocation patterns
- System resource consumption
- Potential memory leaks
- Coroutine stack usage

Regular profiling helps maintain optimal performance and catch issues early in development.
