# PRD — Hot-path v2: stability & perf

> **Status:** Active. Several phases shipped. Phase 3.1–3.4 deferred as architecturally infeasible at current scale.

Targets the Hauliage dev-env "microservice needs reboot" pattern. Collapses hot-path findings from the wiki log into a phased plan with measurable outcomes.

**Baseline (Dec 2025):** ~20k req/s, ~400 ms avg latency, scale-out triggered.

---

## Shipped phases

### Phase 0 — Stop the bleeding

**Phase 0.1** — Remove per-response `Box::leak` (unbounded heap growth).

Upstream fix landed in `may_minihttp` (`feat/response-header-owned-values`, commit `f9daffe`): adds `IntoResponseHeader` + `ResponseHeader { Static, Owned }`; `Response::header` is now generic over static/owned inputs. Owned values drop with the response. 3 `Box::leak` call sites removed. 0 unbounded heap growth.

**Phase 0.3** — Bound metrics DashMap keys.

`MetricsMiddleware::{path_metrics, status_metrics}` capped via `BRRTR_METRICS_PATH_MAX` (default 4096). Novel keys beyond cap collapse to `__other` bucket with `brrtrouter_metrics_path_overflow_total` Prometheus counter. Optimized to `DashMap::get()` (read lock) for hit path, `entry()` + `AtomicUsize` only on miss path.

> **Code anchors:** `src/middleware/metrics.rs`, `Cargo.toml` (dep `BRRTR_METRICS_PATH_MAX`)

### Phase 1 — Lock-free Router / Dispatcher

Replaced `RwLock<Router>` and `RwLock<Dispatcher>` with `ArcSwap`. Hot-path reads collapse from `RwLock::read().expect(...)` to `self.router.load().route(...)`. No lock poisoning. Hauliage hot-reload uses RCU pattern: `load_full` → clone → `on_reload` → `store`.

> **Code anchors:** `src/server/service.rs`, `src/hot_reload.rs`, `Cargo.toml` (dep `arc-swap = "1.7"`)
> **Bench result:** +10.1% throughput (60,575 req/s vs 55,001), −9.4% avg latency (32.09 ms), p99 110 ms (−15.4%).

### Phase 2.1 — Header name interning

`src/server/header_intern.rs`: 24-entry pre-allocated `Arc<str>` table for common HTTP header names. Hit path (>95% of real traffic) is an `Arc::clone`, zero allocation. Miss path unchanged. Replaces 2 heap allocations per header (`to_ascii_lowercase` + `Arc::from(&str)`) with a refcount bump.

### Phase 2.2 — Per-request log demotion

Demoted per-request logs from `warn!` / `info!` to `debug!`:
- `"No route matched"` → `debug!` (was `warn!`)
- `RequestLogger Drop` auth success/completed → `debug!`
- `server::request` parsed/body read → `debug!`
- `dispatcher::core` 5 handler-lifecycle events → `debug!`

Eliminated ~1 MB/sec log flood under stress.

### Phase 2.6 bonus — `write_json_error`

Switched to `serde_json::to_vec` while in the file.

### Phase 5.1 — Bounded worker pool queue

`WorkerPool::dispatch` enforces `queue_bound` via live `queue_depth` atomic. `Shed` mode: fail fast with 429. `Block` mode: cooperative `may::coroutine::sleep(1ms)` up to `backpressure_timeout_ms`, then shed.

> **Code anchors:** `src/worker_pool.rs` — `dispatch()`, `dispatch_with_blocking()`, `dispatch_with_shedding()`, `shed_overflow()`
> **Verification:** pet_store survived 90s smoke test at 3 rounds, 73,716 req/s, 3.5% failures (all unregistered-route 404s), zero aborted connections, log output shrunk from ~1 MB to 240 lines.

### Phase R.1 — Radix terminal method array

`RadixNode::routes` changed from `HashMap<Method, Arc<RouteMeta>>` to a 9-slot `MethodRouteTable` indexed by const match on `http::Method`. Smaller node struct; no HashMap allocator per terminal.

> **Bench finding (honest):** 2000u × 600s returned 60,611 req/s vs 66,484 in baseline — but latency percentiles shifted ~10% uniformly including `/health`. Attributed to thermal/scheduling drift. Current harness cannot reliably resolve sub-~15% changes. Retained on code-quality grounds (simpler, smaller, correctness-tested).

---

## Deferred phases

### Phase 3.1–3.4 — Reply slot (ARCHITECTURALLY INFEASIBLE)

**Phase 3 (2026-04-18)** — attempted to replace `may::sync::mpsc::channel()` with a custom `Arc<ReplySlot>` one-shot (atomic state + `UnsafeCell` + captured `Coroutine::unpark`). Landed on `pre_BFF_work` as `9748dcd`.

> **Bench result (negative):** −7.7% throughput (61,362 vs 66,484 req/s), +8.6% avg latency, +12% p99.

**Why it failed:**

1. The slot was still wrapped in `Arc<ReplySlot>` — same single heap allocation per request as `may::sync::mpsc::channel()`'s internal `Arc<Inner>`. Did not eliminate the allocation.
2. `may`'s coroutine park/unpark is comparable (or slightly worse) to its internal mpsc signal pair for this workload.
3. `Arc::strong_count` check inside `recv` adds a relaxed load per park/re-check iteration.

**3.1–3.4 were to explore:**
- Slab-based pre-allocated slot pool to eliminate per-request heap alloc entirely
- Thread-local slot storage to remove the `Arc` wrapper
- In-band signaling through the request struct (synchronous handoff)
- `may` runtime hooks for zero-overhead coroutine signaling

**Decision (2026-04-21):** **Deferring 3.1–3.4 indefinitely.**

Reasons:

1. **Fundamental architectural constraint.** Any scheme coupling the dispatcher to a handler coroutine needs async communication. To avoid per-request heap alloc you need either:
   - A pre-allocated slot pool (slab) — adds slab lock/contention on top of what `may` already provides
   - Stack-allocated synchronization — not viable with `may`'s coroutine model (dispatcher and handler run in different coroutine contexts)
   - In-band signaling — requires synchronous handoff, breaks fire-and-forget model

2. **Strong current performance.** 66,484 req/s / 29.21ms avg. Previous baseline was ~20k req/s — we've improved 232%. Diminishing returns are steep.

3. **Benchmark harness limitation.** Cannot reliably resolve sub-~15% changes. Even if 3.1–3.4 worked perfectly, we couldn't credibly measure whether they beat the current setup. PRD Phase 6 notes this and proposes a tighter harness (fixed CPU clock, back-to-back A/B in a single session).

4. **Higher-leverage work exists.** Phase 4 (validator fast path) and Phase 6's tightened harness offer clearer paths to measurable gains.

5. **Complexity debt.** The `may::sync::mpsc::channel()` approach is well-tested, well-understood, and works correctly. Replacing it with custom synchronization introduces risk (park/unpark bugs, lifetime issues, coroutine safety) for unmeasurable return.

> **Code anchors for Phase 3 prototype (reverted):** `pre_BFF_work` branch, commit `9748dcd`
> **Current reply channel:** `src/dispatcher/core.rs:736` (`mpsc::channel()`), `src/dispatcher/core.rs:109` (`reply_tx: mpsc::Sender<HandlerResponse>`), `src/worker_pool.rs`

---

### Phase 4 — Validator fast path (TODO)

Optimize the schema validation pipeline for the common-case path. See `llmwiki/topics/schema-validation-pipeline.md` for the full pipeline description and code anchors.

### Phase 6 — Bench harness tightening + Goose v2

- Tighten the 2000u × 600s stress harness: fixed CPU clock, back-to-back A/B runs in a single session to eliminate thermal/scheduling drift.
- Criterion microbenches for router-internal timings (replaces or augments the macro stress test for sub-15% signal).
- Goose v2 scenario matrix with JSON baselines.

---

## Current baseline (2026-04-18)

**2000 concurrent users × 600s** (post-Phase 0.3 + 2.1):

| Metric | Value |
|--------|-------|
| Throughput | 66,484 req/s (+9.7% vs Phase 1, +232% vs Dec 2025) |
| Avg latency | 29.21 ms (−9%) |
| p50 | 26 ms |
| p95 | 64 ms |
| p99 | 98 ms |
| Max | 794 ms |
| Real failures | 0 |
| Server status | HTTP 200 at end |

Baseline file: `benches/baselines/2000u-600s-phase-0-3-2-1.json`

---

## Open items

- **Bench harness reliability:** Need tighter harness before pursuing further micro-optimizations. Phase 6.
- **Phase 4 (validator fast path):** Not yet explored. See `llmwiki/topics/schema-validation-pipeline.md`.
- **Phase 3.1–3.4:** Deferred indefinitely. See rationale above.
