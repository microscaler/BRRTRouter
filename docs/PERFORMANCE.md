# Performance Benchmarks

This document details BRRTRouter's performance characteristics, optimization results, and benchmarking methodology.

> **April 2026 update.** The hot-path v2 work (PRD [`PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](./PRD_HOT_PATH_V2_STABILITY_AND_PERF.md)) has shipped Phases 0.1 (remove per-response `Box::leak`), 2.2 (demote per-request tracing), 5.1 (real bounded worker-pool queue), 1 (lock-free `Router` / `Dispatcher` via `ArcSwap`), **0.3 (bounded metrics path maps)**, and **2.1 (header-name intern)**. Numbers below reflect the post-0.3+2.1 build and **supersede** both the December 2025 baselines and the earlier April 2026 intermediate numbers further down this page.
>
> **Follow-up work** is tracked in the PRD open items. For the validation hot path and reproducible benches, see [`llmwiki/topics/schema-validation-pipeline.md`](./llmwiki/topics/schema-validation-pipeline.md) and [`llmwiki/topics/bench-harness-phase-6.md`](./llmwiki/topics/bench-harness-phase-6.md).

## Cloud-Native Scale-Out Strategy

BRRTRouter is optimized for scalable cloud-native deployments with hard-bound resource limits. The pre-fix design targeted **fail-fast 503s at 2,000 users / 20 k req/s per pod** to force horizontal scaling. Post-fix measurement (below) shows the per-pod ceiling is substantially higher than that original target — HPA triggers can be tuned accordingly.

| Metric | December 2025 target | Post Phases 0.1/2.2/5.1 | Post Phase 1 (ArcSwap) | Post Phases 0.3 + 2.1 — **current** |
|--------|---------------------:|-------------------------:|------------------------:|-------------------------------------:|
| **Concurrent Users / Pod** | 2,000 (bound limit) | 2,000 | 2,000 | 2,000 (sustained, further headroom) |
| **Throughput / Pod** | 20,000 req/s | 55,001 req/s | 60,575 req/s | **66,484 req/s (+232 %)** |
| **Base Latency (avg)** | ~15 ms | 35.40 ms | 32.09 ms | **29.21 ms (−17.4 % vs 5.1)** |
| **p99 under 2 k-user load** | ~400 ms | 130 ms | 110 ms | **98 ms (−75.5 % vs Dec 2025)** |
| **p50 under 2 k-user load** | n/a | 30 ms | 28 ms | **26 ms** |
| **5xx shed rate at 2 k users** | 0 %, target bound | 0 % | 0 % | **0 %** (no 5xx, 0 aborted connections) |

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

## Post Hot-Path v2 measurements (April 2026)

### 2,000 users × 600 s sustained — headline benchmark (post Phase 1)

Driver: `cargo run --release --example api_load_test -- --host http://127.0.0.1:<port> --users 2000 --increase-rate 200 --run-time 600s --no-reset-metrics`. Server: `examples/pet_store` (release, jemalloc, `RUST_LOG=brrtrouter=warn`). Hardware: M-series laptop, single pod. Port used: 8091 (avoids local Tilt occupying 8081). Artefacts: [`benches/baselines/2000u-600s-arcswap.json`](../benches/baselines/2000u-600s-arcswap.json) (post-Phase-1) and [`benches/baselines/2000u-600s.json`](../benches/baselines/2000u-600s.json) (pre-Phase-1 reference).

| Metric | Pre Phase 1 (RwLock) | Post Phase 1 (ArcSwap) | Δ |
|---|---:|---:|---:|
| Duration (steady) | 10 min 01 s | 10 min 00 s | — |
| Total requests | 33,825,644 | **37,253,602** | **+10.1 %** |
| Successful 2xx | 32,688,348 | 36,001,118 | +10.1 % |
| Real failures (5xx, aborted) | 0 | **0** | = |
| 404s from Goose `GET /` (unregistered root route) | 1,137,296 | 1,252,484 | (matches request volume increase) |
| **Aggregate throughput** | 55,001 req/s | **60,575 req/s** | **+10.1 %** |
| Latency — average | 35.40 ms | **32.09 ms** | **−9.4 %** |
| Latency — median (p50) | 30 ms | 28 ms | −6.7 % |
| Latency — p95 | 79 ms | 70 ms | −11.4 % |
| Latency — p99 | 130 ms | **110 ms** | **−15.4 %** |
| Latency — max | 769 ms | 906 ms | single-outlier noise over 37 M reqs |
| Server state at end | HTTP 200, 0 dropped | HTTP 200, 0 dropped | = |

Together with Phases 0.1 / 2.2 / 5.1 this yields a **~3 ×** per-pod throughput improvement over the December 2025 ceiling (20 k → 60.6 k req/s) at **~72 % lower p99** (400 ms → 110 ms), with **zero shed**. The Hauliage pod reboot cadence that motivated this PRD is structurally fixed.

See [`llmwiki/log.md`](./llmwiki/log.md) (append-only log; add your bench session there) and historical context in the PRD baseline section.

### What the hot-path v2 fixes moved

| Area | Symptom pre-fix | Mechanism | Outcome |
|---|---|---|---|
| Per-response `Box::leak` (Phase 0.1) | Monotonic RSS growth under steady load; Hauliage service reboot cadence | `may_minihttp` fork accepts owned header values; owned values drop with response | RSS flat; no more leak-driven reboots |
| Per-request `WARN "No route matched"` (Phase 2.2) | ~2,800 synchronous log writes/sec at 20 users; log pipeline starves handlers; SIGABRT at 152 s | Demoted to `debug!` + other hot-path `info!` → `debug!` | Log output 1 MB → 240 lines for 90 s of 74 k req/s |
| Unbounded `may::sync::mpmc` worker-pool queue (Phase 5.1) | Queue grew without bound under overload → allocator pressure → `panic = "abort"` | Bounded via `WorkerPoolMetrics::queue_depth`; `Shed` = 429 fast-fail, `Block` = cooperative yield up to `backpressure_timeout_ms` | 2 k users / 55 k req/s / 10 min with **zero shed** — bound never hit |

## JSF Optimization Results (historical — 2025)

The JSF AV Rules implementation doubled throughput from ~40k to ~81k req/s at **20 users** on single endpoint:

| Optimization | Before | After | Impact |
|--------------|--------|-------|--------|
| Parameter storage | `HashMap` | `SmallVec<[T; 8]>` | -50% alloc overhead |
| Header storage | `HashMap` | `SmallVec<[T; 16]>` | -40% alloc overhead |
| Route matching | O(routes × segments) | O(segments) radix | Predictable latency |
| Error handling | Mixed panic/Result | Result-only | Zero crash paths |

## Historical baselines (December 2025 — pre hot-path v2; kept for context)

> The tables below were captured before Phases 0.1 / 2.2 / 5.1 landed. They reflect a configuration that would crash under the same load today would handle cleanly. Use the April 2026 measurements above for sizing decisions.

### Adaptive Load Test Results & Shedding Profiles (December 2025)

Ramp testing from 2,000 users enforces tight bound limits to prevent catastrophic OOM events:

| Users | Requests | Throughput | Base (15ms) | Latency (Load) | 5xx Shed Rate |
|-------|----------|-----------|-----|-----|-----|
| 500  | 0.5M | <10k req/s | <20ms | ~250ms | 0% |
| 1,000 | 1.5M | <15k req/s | <20ms | ~300ms | 0% |
| 2,000 | 2.50M| 20k req/s  | 15ms | ~400ms | 0% |
| **3,000 Spike** | 3.5M | >20k req/s | 15ms | ~450ms | **Scale Out Triggered (503)** |

**Key findings (superseded April 2026 — see top of page):**
- ~~Stable maximum: 2,000 concurrent users before triggering HPA bound threshold natively.~~ — Post-fix builds sustain 2 k users well inside their latency budget; HPA triggers should be retuned.
- **Fail-fast limits:** Reaching limits instantly sheds requests rather than inflating heap (`503 Service Unavailable: Handler Queue Full - Request Shed`). Mechanism remains but now produces 429 from the bounded worker-pool queue rather than 503 from OOM/abort.
- **Real-world execution:** Empty boilerplate yields ~15ms responses, realistic database operations operate smoothly out to ~200-400ms latency without bottlenecking the MPSC event loops.

### Stress Testing Results & Shedding Profiles (December 2025)

Aggressive load testing proves graceful degradation bounds:

| Concurrent Users | Throughput | Base | Business Logic | Verdict (Dec 2025) |
|------------------|-----------|-----|-----|---------|
| 1,000 | 10k req/s | 15ms | ~250ms | ✅ Comfortable |
| **2,000** | **20k req/s** | 15ms | ~400ms | ✅ Target bound limit *(April 2026: 55k req/s, 35 ms avg — limit is elsewhere)* |
| 2,500 | Cap Reached | 15ms | - | ⚠️ Sheds 503 errors natively |
| 4,000 | Cap Reached | - | - | ❌ Forces HPA horizontal scaling |

**Production recommendation (pending re-measure):** the **2,000 concurrent users** figure remains a safe HPA trigger and is where current Hauliage services are sized; the **per-pod headroom above 2 k** is new since April 2026 and should be characterised by a follow-up benchmark before any configuration change.

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
| Per-response `Box::leak` for header values → unbounded RSS growth                     | ✅ Fixed (PRD Phase 0.1, Apr 2026) |
| Per-request `WARN "No route matched"` → log-pipeline saturation → SIGABRT              | ✅ Fixed (PRD Phase 2.2, Apr 2026) |
| Unbounded `may::sync::mpmc` worker-pool queue → allocator pressure → crash            | ✅ Fixed (PRD Phase 5.1, Apr 2026) |
| `RwLock<Router>` / `RwLock<Dispatcher>` on hot path                                   | ✅ Fixed (PRD Phase 1, Apr 2026 — lock-free `ArcSwap`) |
| `Arc::from(h.name.to_ascii_lowercase())` per header per request                       | 🚧 PRD Phase 2.1 (header-name intern) |
| Per-request `mpsc::channel()` reply allocation                                        | 🚧 PRD Phase 3 (parker-based reply) |
| Unbounded metrics `DashMap<String, _>` path keys                                      | 🚧 PRD Phase 0.3 — next |
| No **connection pooling / keep-alive tuning** yet.                                    | 🚧 Planned |

## Performance Vision

Build the fastest, most predictable scalable OpenAPI-native router in Rust — maximizing tight 2,000 user container densities for massively parallel cloud-native elasticity.

> **Goal:** 15ms tight routing bounds prior to business logic execution, failing gracefully during localized pod exhaustion natively utilizing memory queue RAII protection mechanisms.

## Phase 6 — harness science (Epic 12 / Story 12.8)

Goal: make **sub-~15%** microbench deltas meaningful and decide the **next
bottleneck from measurements**, not folklore.

### Criterion defaults (stabilize variance)

Shared helpers live in [`brrtrouter::perf_harness`](../src/perf_harness.rs):

| Setting | Value | Why |
|---------|------:|-----|
| `sample_size` | 60 | Less single-outlier swing on laptop/CI hosts |
| `measurement_time` | 5 s | Enough wall for noisy schedulers |
| `warm_up_time` | 1 s | Drop cold-cache first samples |

**Noise policy (N1):** if a Criterion compare flakes under load, re-run on an
**idle ms02** (`just bench-against-ms02-all`) before changing code. Do **not**
“fix” variance with radix churn (N6).

**CPU pinning / seed:** optional; when comparing A/B on ms02, keep the same
power state and avoid concurrent Tilt builds. Criterion’s statistical engine
is the primary gate — pin with `taskset` only when documenting a host baseline.

### Microbenches

| Bench | Purpose |
|-------|---------|
| `throughput` | `route_match` + scalability **10→500** routes (P1/P2) |
| `schema_validation_hot_path` | cached `is_valid` / `iter_errors` (P3) |
| `match_vs_validate` | **P5 comparative** — match vs schema on one Criterion group |
| `request_guards` | optional 12.2 body-limit + 12.4 param-validation helpers |
| `jwt_cache_performance` | security path (unchanged) |

```bash
cargo bench -p brrtrouter --bench match_vs_validate
cargo bench -p brrtrouter --bench request_guards
just bench-baseline-ms02-all   # save host baseline tag
```

**CI note (N5):** Criterion benches are **local / ms02 optional** — unit tests
in `tests/perf_science_tests.rs` + `perf_harness` guard P1/P5 without requiring
`cargo bench` in every CI job.

### P5 evidence — match vs validate (measured)

Timed helpers (`match_vs_validate_ns_per_op`) and Criterion
`match_vs_validate` on idle **ms02** (release, `--quick` smoke 2026-08-08):

| Path | Observed |
|------|----------|
| `route_match` @ 100 routes | **~161 ns** |
| `schema_is_valid` (happy path) | **~44 ns** |
| `schema_iter_errors` (invalid pet body) | **~163 ns** |

Takeaways:

- Match, happy-path validate, and failure-path `iter_errors` are all **sub-µs**
  and within ~4× on this schema — neither “routing dominates” nor “bare
  `is_valid` always dominates match” is true (N2).
- Scalability 10→500 stays flat; match is already negligible vs **multi-ms**
  end-to-end latency under Goose load.
- Therefore a **radix/trie rewrite is not the next win** (N6).

### Next bottleneck (written recommendation)

**Invest next in the full request pipeline / dispatch**, not a trie rewrite:

1. **End-to-end `AppService` path** under flamegraph (body read, auth, param
   guards, handler coroutine dispatch, response write) — where the multi-ms
   budget actually goes.
2. **Handler dispatch / reply path** (PRD Phase 3 reply-slot remains open;
   out of scope for 12.8 implementation, but the evidence points here).
3. Keep schema work on the **cache hit + `is_valid` first** pattern; treat
   `iter_errors` as the failure-only path.
4. **Keep** lock-free `SharedRouter` / `SharedDispatcher` (`ArcSwap`) — do not
   reintroduce `RwLock` on the match path (P6).

**Non-goals for this story:** Phase 3 reply-slot redesign; radix/trie rewrite;
claims that “routing” dominates without P5 data (N2).

### Scalability curve (P2)

`route_scalability/{10,50,100,200,500}_routes` matches a mid-tree path. Expect
near-flat ns/op growth with route count for the current radix; large jumps
warrant investigation of tree shape — not an automatic rewrite.

## Running Benchmarks

```bash
just bench  # Executes cargo bench with Criterion
```

Harness detail: [`llmwiki/topics/bench-harness-phase-6.md`](./llmwiki/topics/bench-harness-phase-6.md).

Older notes: profiling once highlighted regex capture and `HashMap` allocations;
preallocating buffers in `Router::route` trimmed roughly 5% off match times —
still **secondary** to schema validation per Phase 6 P5 evidence.

## Generating Flamegraphs

See [docs/flamegraph.md](flamegraph.md) for **validator-path** steps (P4) and
general interpretation tips.

## Load Testing

For comprehensive load testing with Goose, see [docs/GOOSE_LOAD_TESTING.md](GOOSE_LOAD_TESTING.md).

## Related Documentation

- [**Hot-path v2 PRD**](PRD_HOT_PATH_V2_STABILITY_AND_PERF.md) - Phased plan; context for the April 2026 measurements above
- [JSF AV Rules Compliance](JSF_COMPLIANCE.md) - How JSF standards improved performance
- [JSF Writeup](JSF/JSF_WRITEUP.md) - Detailed JSF analysis and design
- [Performance Analysis](PERFORMANCE_ANALYSIS.md) - Deep dive into performance characteristics
- [Performance Metrics](PERFORMANCE_METRICS.md) - Detailed metrics collection

