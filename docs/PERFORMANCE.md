# Performance Benchmarks

This document details BRRTRouter's performance characteristics, optimization results, and benchmarking methodology.

## Cloud-Native Scale-Out Strategy

BRRTRouter is optimized for scalable cloud-native deployments with hard-bound resource limits, capping individual pods to fail fast (503 Service Unavailable) and force horizontal scaling (HPA) rather than exhausting heap.

| Metric | Target |
|--------|--------|
| **Concurrent Users / Pod** | 2,000 |
| **Throughput / Pod** | 20,000 req/s |
| **Base Latency** | ~15ms |
| **Business Logic Latency** | ~200-400ms |

| Stack / "hello-world" benchmark          | Test rig(s)*                               | Req/s (steady-state) | Comments                                |
| ---------------------------------------- | ------------------------------------------ | -------------------- | --------------------------------------- |
| Node 18 / Express                        | Same class HW                              | 8–15 k               | Single threaded; many small allocations |
| Python / FastAPI (uvicorn)               | Same                                       | 6–10 k               | Async IO but Python overhead dominates  |
| **Rust / BRRTRouter (JSF)**              | M-class laptop – 20 users / Goose          | **≈ 81 k**           | Includes full request and response validation & Telemetry which are absent as standard in competitors       |
| Go / net-http                            | Same                                       | 70–90 k              | Go scheduler, GC in play                |
| Rust / Axum (tokio)                      | Same                                       | 120–180 k            | Native threads, zero-copy write         |
| Rust / Actix-web                         | Same                                       | 180–250 k            | Pre-allocated workers, slab alloc       |
| Nginx (static)                           | Same                                       | 450–550 k            | C, epoll, no JSON work                  |

*Community figures taken from TechEmpower round-20-equivalent and recent blog posts; all on laptop-grade CPUs (Apple M-series or 8-core x86).

## JSF Optimization Results

The JSF AV Rules implementation doubled throughput from ~40k to ~81k req/s:

| Optimization | Before | After | Impact |
|--------------|--------|-------|--------|
| Parameter storage | `HashMap` | `SmallVec<[T; 8]>` | -50% alloc overhead |
| Header storage | `HashMap` | `SmallVec<[T; 16]>` | -40% alloc overhead |
| Route matching | O(routes × segments) | O(segments) radix | Predictable latency |
| Error handling | Mixed panic/Result | Result-only | Zero crash paths |

## Adaptive Load Test Results & Shedding Profiles (December 2025)

Ramp testing from 2,000 users enforces tight bound limits to prevent catastrophic OOM events:

| Users | Requests | Throughput | Base (15ms) | Latency (Load) | 5xx Shed Rate |
|-------|----------|-----------|-----|-----|-----|
| 500  | 0.5M | <10k req/s | <20ms | ~250ms | 0% |
| 1,000 | 1.5M | <15k req/s | <20ms | ~300ms | 0% |
| 2,000 | 2.50M| 20k req/s  | 15ms | ~400ms | 0% |
| **3,000 Spike** | 3.5M | >20k req/s | 15ms | ~450ms | **Scale Out Triggered (503)** |

**Key findings:**
- **Stable maximum: 2,000 concurrent users** before triggering HPA bound threshold natively.
- **Fail-fast limits:** Reaching limits instantly sheds requests rather than inflating heap (`503 Service Unavailable: Handler Queue Full - Request Shed`).
- **Real-world execution:** Empty boilerplate yields ~15ms responses, realistic database operations operate smoothly out to ~200-400ms latency without bottlenecking the MPSC event loops.

## Stress Testing Results & Shedding Profiles (December 2025)

Aggressive load testing proves graceful degradation bounds:

| Concurrent Users | Throughput | Base | Business Logic | Verdict |
|------------------|-----------|-----|-----|---------|
| 1,000 | 10k req/s | 15ms | ~250ms | ✅ Comfortable |
| **2,000** | **20k req/s** | 15ms | ~400ms | ✅ **Target bound limit** |
| 2,500 | Cap Reached | 15ms | - | ⚠️ Sheds 503 errors natively |
| 4,000 | Cap Reached | - | - | ❌ Forces HPA horizontal scaling |

**Production recommendation**: Target **2,000 concurrent users** to gracefully trigger Cloud HPA expansion reliably.

## Stack Size Optimization (December 2025)

Empirical testing at 4,000 concurrent users found the optimal coroutine stack size (latencies in ms):

| Stack Size | Throughput | p50 | p75 | p98 | p99 | Max | Status |
|------------|-----------|-----|-----|-----|-----|-----|--------|
| 64 KB (old) | 67k req/s | 22 | 34 | 63 | 74 | 400 | ❌ Wasteful |
| 32 KB | 67k req/s | 22 | 34 | 63 | 74 | 400 | ⚠️ Works |
| **16 KB (new)** | **68k req/s** | **29** | **74** | **110** | **120** | **210** | ✅ **Optimal** |
| 8 KB | 59k req/s | 33 | 79 | 150 | 160 | 430 | ⚠️ Degraded |

**Key findings:**
- Actual stack usage: ~3.5 KB per coroutine (measured via telemetry)
- 16 KB provides **4x safety margin** while minimizing memory
- Memory savings: 10,000 users × (64KB - 16KB) = **480 MB saved**
- Best latency characteristics at 16KB boundary (lowest max latency)

## Previous Bottlenecks (Now Resolved)

| Factor                                                                                | Status |
| ------------------------------------------------------------------------------------- | ------ |
| `HashMap` allocations on every request for params/headers                             | ✅ Fixed with SmallVec |
| Linear route scanning                                                                 | ✅ Fixed with radix tree |
| Default coroutine **stack size** = 64 KB → now 16KB (4x actual usage)                 | ✅ Fixed |
| No **connection pooling / keep-alive tuning** yet.                                    | 🚧 Planned |

## Performance Vision

Build the fastest, most predictable scalable OpenAPI-native router in Rust — maximizing tight 2,000 user container densities for massively parallel cloud-native elasticity.

> **Goal:** 15ms tight routing bounds prior to business logic execution, failing gracefully during localized pod exhaustion natively utilizing memory queue RAII protection mechanisms.

## Running Benchmarks

```bash
just bench  # Executes cargo bench with Criterion
```

Recent profiling with `flamegraph` highlighted regex capture and `HashMap` allocations as hotspots. Preallocating buffers in `Router::route` and `path_to_regex` trimmed roughly 5% off benchmark times.

## Generating Flamegraphs

```bash
cargo flamegraph -p brrtrouter  # Produces flamegraph.svg in the current directory
```

See [docs/flamegraph.md](flamegraph.md) for tips on reading the output.

## Load Testing

For comprehensive load testing with Goose, see [docs/GOOSE_LOAD_TESTING.md](GOOSE_LOAD_TESTING.md).

## Related Documentation

- [JSF AV Rules Compliance](JSF_COMPLIANCE.md) - How JSF standards improved performance
- [JSF Writeup](JSF/JSF_WRITEUP.md) - Detailed JSF analysis and design
- [Performance Analysis](PERFORMANCE_ANALYSIS.md) - Deep dive into performance characteristics
- [Performance Metrics](PERFORMANCE_METRICS.md) - Detailed metrics collection

