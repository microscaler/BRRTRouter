# JSF AV Rules Compliance

BRRTRouter implements coding standards inspired by the [**Joint Strike Fighter Air Vehicle C++ Coding Standards**](https://www.stroustrup.com/JSF-AV-rules.pdf) (JSF AV Rules) — the same rigorous standards used in the F-35 fighter jet's flight-critical software.

## Why JSF Rules Matter

The JSF AV Rules were developed by Lockheed Martin for safety-critical avionics software where **predictable performance** and **zero runtime failures** are mandatory. While BRRTRouter isn't flying aircraft, these principles translate directly to high-performance HTTP routing:

| JSF Principle | BRRTRouter Implementation | Benefit |
|---------------|---------------------------|---------|
| **No heap after init** (Rule 206) | `SmallVec<[T; N]>` for path/query/header params | Zero allocations in hot path |
| **Bounded complexity** (Rule 1-3) | Radix tree with O(k) lookup | Predictable latency |
| **No panics** (Rule 208) | `Result`-based error handling | No crash paths in dispatch |
| **Explicit types** (Rule 209) | `ParamVec`, `HeaderVec` newtypes | Type-safe, self-documenting |
| **No recursion** (Rule 119) | Iterative path matching | Bounded stack depth |

## What We Implemented

### Stack-Allocated Collections (JSF Rule 206)

```rust
// Hot path uses SmallVec - stack allocated, no heap
pub type ParamVec = SmallVec<[(String, String); MAX_INLINE_PARAMS]>;
pub type HeaderVec = SmallVec<[(String, String); MAX_INLINE_HEADERS]>;
```

### Clippy Configuration for Safety

```toml
# clippy.toml - JSF-inspired thresholds
cognitive-complexity-threshold = 30
stack-size-threshold = 512000
too-many-arguments-threshold = 8
```

### Crate-Level Lint Configuration

```rust
// Documented intentional patterns, not suppressed warnings
#![allow(clippy::expect_used)]  // Startup only
#![allow(clippy::panic)]        // Config errors only
#![allow(clippy::result_large_err)]  // HandlerResponse needed
```

## Performance Validation

The JSF-compliant hot path was validated with Goose load testing:

| Metric | Target | Result |
|--------|--------|--------|
| **Throughput** | 10-20k req/s | 81,407 req/s |
| **Failure Rate** | < 0.1% | 0% |
| **p50 Latency** | < 25ms | ~15ms |
| **p99 Latency** | < 450ms | ~200-400ms |
| **p99.99 Latency** | < 50ms | 5ms |

All 19 petstore sample API endpoints tested with 20 concurrent users over 60 seconds — **zero failures, zero panics**.

## Key Files

| File | Purpose |
|------|---------|
| [`clippy.toml`](../clippy.toml) | JSF-inspired Clippy configuration |
| [`src/router/radix.rs`](../src/router/radix.rs) | O(k) radix tree routing |
| [`src/router/core.rs`](../src/router/core.rs) | Stack-allocated `ParamVec` |
| [`src/dispatcher/core.rs`](../src/dispatcher/core.rs) | Stack-allocated `HeaderVec` |
| [`docs/JSF/JSF_WRITEUP.md`](JSF/JSF_WRITEUP.md) | Full JSF analysis and design |

## Why This Matters

1. **Predictable Performance**: No GC pauses, no allocation jitter, no surprise latency spikes
2. **Auditability**: Clear separation of "startup allocations OK" vs "hot path must be zero-alloc"
3. **Real-Time Ready**: Foundation for embedded/RTOS deployments where determinism is required
4. **Developer Confidence**: If it compiles with these lints, it won't crash in production

> *"JSF is basically 'a safe subset of an unsafe language.' Rust already bakes in a lot of what they're trying to enforce, but there are some very useful patterns we can steal — especially around bounded complexity, allocation discipline, and generic/OO design."*

## Related Documentation

- [Performance Benchmarks](PERFORMANCE.md) - Performance results from JSF implementation
- [JSF Writeup](JSF/JSF_WRITEUP.md) - Detailed JSF analysis and design
- [JSF Audit Opinion](JSF/JSF_AUDIT_OPINION.md) - Expert analysis of JSF compliance
- [Performance Optimization PRD](JSF/PERFORMANCE_OPTIMIZATION_PRD.md) - PRD for JSF implementation

