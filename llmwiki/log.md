# LLM Wiki Log

## [2026-04-21] decision | Phase 3.1–3.4 deferred indefinitely — reply slot infeasible

Investigated deferred follow-ups to Phase 3 (custom reply slot replacing `may::sync::mpsc::channel()`). PRD file had been cleared; reconstructed from log.md and codebase analysis.

**Why Phase 3 failed (from 2026-04-18 log):** Custom `Arc<ReplySlot>` still used `Arc<ReplySlot>` wrapper — same single heap allocation per request as `may::sync::mpsc::channel()`'s `Arc<Inner>`. `Arc::strong_count` check inside `recv` added overhead per park/re-check. Bench result: −7.7% throughput, +8.6% latency.

**Why 3.1–3.4 are deferred indefinitely:**
1. Any async comm scheme between dispatcher and handler coroutine needs *some* heap-backed state; slab-based pre-allocation adds contention on top of what `may` provides. Stack-alloc synchronization is not viable with `may`'s coroutine model. In-band signaling breaks fire-and-forget.
2. 66,484 req/s / 29.21ms avg — already 232% improvement over baseline. Diminishing returns are steep.
3. Benchmark harness can't resolve sub-~15% changes. Tightening (Phase 6) is prerequisite.
4. Complexity risk of custom sync vs well-tested `may::sync::mpsc::channel()` is not justified.

> **Code anchors:** `src/dispatcher/core.rs:736` (current `mpsc::channel()`), `src/dispatcher/core.rs:109` (`reply_tx`), `src/worker_pool.rs` (worker pool with backpressure), `pre_BFF_work` branch commit `9748dcd` (reverted Phase 3 prototype)
> **PRD:** `docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md` — fully restored with all shipped phases, decisions, and open items.

## [2026-04-18] ship | Phase R.1 — radix terminal method array

- `RadixNode::routes` changed from `HashMap<Method, Arc<RouteMeta>>` to a 9-slot `MethodRouteTable` indexed by a const match on `http::Method`. Same semantics (extension methods yield no route, matching the pre-R.1 supported-method filter); smaller node struct; no HashMap allocator per terminal.
- Commit `perf(router): radix terminal lookup via method array (Phase R.1)`. 49 router tests + 299 lib tests pass.
- **Bench finding (honest):** 2000u × 600s returned 60,611 req/s vs 66,484 in the Phase 0.3+2.1 baseline captured earlier the same day — but the latency percentiles shifted **~10 % uniformly across every endpoint**, including `/health` which never touches the terminal table. A real router slowdown would be localised. Attributed to laptop thermal / scheduling drift between runs hours apart. **The current bench harness cannot reliably resolve sub-~15 % changes.** Captured as an action in PRD Phase 6: either tighten the harness (fixed CPU clock, back-to-back A/B in a single session) or add criterion microbenches for router-internal timings.
- Retained on code-quality grounds (simpler, smaller, correctness-tested) per PRD decision framework; if a tighter bench later shows a regression we'll revisit.

## [2026-04-18] experiment | Phase 3 (parker reply slot) — attempted, reverted

Tried to replace the per-request `may::sync::mpsc::channel()` with a custom `Arc<ReplySlot>` one-shot (atomic state + UnsafeCell + captured `Coroutine::unpark`). Landed on `pre_BFF_work` as `9748dcd`, then measured on the 2000u × 600s bench:

| Metric | Phase 0.3 + 2.1 | Phase 3 prototype | Δ |
|---|---:|---:|---:|
| Throughput | 66,484 req/s | 61,362 req/s | **−7.7 %** |
| Avg latency | 29.21 ms | 31.73 ms | +8.6 % |
| p99 | 98 ms | 110 ms | +12 % |

Reverted. Why it failed: the slot is still wrapped in `Arc<ReplySlot>` — same single heap allocation per request as `may::sync::mpsc::channel()`'s `Arc<Inner>`, so we didn't eliminate the alloc. `may`'s coroutine park/unpark is comparable (or slightly worse) to its internal mpsc signal pair for this workload. The `Arc::strong_count` check inside `recv` adds a relaxed load per park/re-check iteration. PRD [`PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](../docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md) §Phase 3 now documents the negative result; 3.1–3.4 are deferred. `cargo test --lib` 299/299 restored post-revert.

## [2026-04-18] ship | Phase 0.3 + 2.1 — bounded metrics maps + header intern

- **Upstream**: `may_minihttp` PR [`#24`](https://github.com/Xudong-Huang/may_minihttp/pull/24) (owned-header values, PRD Phase 0.1 upstreaming) raised.
- **Phase 0.3**: soft cap on `MetricsMiddleware::{path_metrics, status_metrics}` keys via `BRRTR_METRICS_PATH_MAX` (default 4096). Novel keys beyond cap collapse to `"__other"` bucket with `brrtrouter_metrics_path_overflow_total` Prometheus counter. **First shipped version used `contains_key + entry` per call and cost 62 % throughput at 2 k users**; refactored to `DashMap::get()` (read lock) for the hit path and `entry()` + `AtomicUsize` counter only on the miss path. Commit `perf(metrics): bound path/status DashMap keys with __other overflow (Phase 0.3)` + refactor `perf(metrics): read-first hot path for cap-bounded path maps (Phase 0.3 refactor)`.
- **Phase 2.1**: new [`src/server/header_intern.rs`](../src/server/header_intern.rs) module with 24-entry pre-allocated `Arc<str>` table for common HTTP header names. Hit path (>95 % of real traffic) is an `Arc::clone`, zero allocation; miss path unchanged. Replaces 2 heap allocations per header (`to_ascii_lowercase` + `Arc::from(&str)`) with a refcount bump on the hit. Commit `perf(request): intern common HTTP header names (Phase 2.1)`.
- **2000u × 600s re-bench** (post-refactor, on pet_store:8091): **40.82 M requests → 66,484 req/s (+9.7 % vs Phase 1, +232 % vs Dec 2025)**. Avg latency **29.21 ms (−9 %)**, p50 **26 ms**, p95 **64 ms**, p99 **98 ms**, max 794 ms. 0 real failures. Server still HTTP-200. Committed baseline [`benches/baselines/2000u-600s-phase-0-3-2-1.json`](../benches/baselines/2000u-600s-phase-0-3-2-1.json).
- `cargo test --lib` 299/299 (4 new header_intern tests + 2 new path-cap tests).

## [2026-04-18] ship | Phase 1 — lock-free Router / Dispatcher via ArcSwap

- Added `arc-swap = "1.7"` to root `Cargo.toml` + `examples/pet_store/Cargo.toml`. New public type aliases `SharedRouter = Arc<ArcSwap<Router>>` / `SharedDispatcher = Arc<ArcSwap<Dispatcher>>` in [`src/server/service.rs`](../src/server/service.rs).
- Hot-path reads collapse from `RwLock::read().expect(...)` to `self.router.load().route(...)` and `self.dispatcher.load()`. No lock poisoning.
- [`src/hot_reload.rs`](../src/hot_reload.rs) publishes via `router.store(Arc::new(new_router))` and (for dispatcher) `load_full` → clone → `on_reload` → `store` (RCU pattern; readers holding the old `Arc` keep serving).
- Mass-updated ~15 call sites (tests + `cli/commands` + pet_store + generator templates) via scripted `Arc::new(RwLock::new(X))` → `Arc::new(arc_swap::ArcSwap::from_pointee(X))` pass.
- Commit `perf(router): lock-free Router + Dispatcher via ArcSwap (Phase 1)` + follow-up `perf(petstore): adopt ArcSwap dump_routes / drop RwLock (Phase 1 follow-up)`.
- `cargo test --lib` 293/293. 11 of 13 integration test binaries pass (162 tests); `multi_response_tests` + `generator_templates_tests` have pre-existing `RouteMeta { request_content_types: ... }` fixture gaps from the earlier 415 work — unrelated to ArcSwap.

## [2026-04-18] bench | 2000 users × 600 s — Phase 1 ArcSwap

Re-run of the headline 2k/600s stress against pet_store on port 8091 (Tilt squatted on 8081 again; confirmed Phase 1 build behaves identically on a free port).

- **37,253,602 requests** (+10.1 % vs pre-Phase-1) — 36,001,118 [200] + 1,252,484 [404] (same `GET /` test scenario).
- **60,575 req/s** sustained (+10.1 %).
- Latency: avg **32.09 ms (−9.4 %)** / p50 **28 ms** / p95 **70 ms (−11.4 %)** / p99 **110 ms (−15.4 %)** / max 906 ms (outlier noise over 37 M reqs).
- **Zero real failures**; server still HTTP-200 at end.
- Baseline committed: [`benches/baselines/2000u-600s-arcswap.json`](../benches/baselines/2000u-600s-arcswap.json).

## [2026-04-18] bench | 2000 users × 600 s — post-Phase-2.2/5.1 ceiling

Stress test against `pet_store` on `127.0.0.1:8081`, direct `api_load_test` binary (no averaging): **`--users 2000 --increase-rate 200 --run-time 600s --no-reset-metrics`**. `RUST_LOG=brrtrouter=warn,pet_store=warn`.

- **33,825,644 requests** handled (32,688,348 [200] + 1,137,296 [404]).
- The 1.1 M "failures" are all the `test_index` Goose transaction hitting `GET /` — an unregistered root route. **Zero real failures, zero 5xx, zero aborted connections.**
- **55,001 req/s sustained** at 2000 concurrent users for 10 minutes.
- **Latency**: avg 35.40 ms / median 30 ms / p95 79 ms / p99 130 ms / max 769 ms.
- Server **still serving HTTP 200** after the run; peak RSS ~252 MB with decreasing trend.
- vs [`docs/PERFORMANCE.md`](../docs/PERFORMANCE.md) 2000-user baseline (20 k req/s, ~400 ms latency, scale-out triggered): **+175 % throughput, −92 % avg latency, 0 % shed**.
- Artefacts: `/tmp/goose_2k/{report.html,report.json,report.md}` — JSON is the Phase 6 baseline candidate.
- Side observation: Tilt was already listening on IPv6 `[::1]:8081`; our IPv4 `127.0.0.1:8081` bind coexisted cleanly.

## [2026-04-18] ship | Phase 2.2 + 5.1 — hot-path logs + bounded queue

- **Trigger**: the port-change Goose smoke (pet_store on 8081, 20 users × 30 s × 3 runs) reproduced the Hauliage reboot pattern in 30 s: **pet_store SIGABRT (exit 134)** under ~58 k req/s, preceded by **~2,800 synchronous `WARN "No route matched"` log writes/sec** flooding stderr (~1 MB of log output).
- **Phase 2.2 (per-request log demotion)**: `warn!("No route matched")` → `debug!` + per-request `info!` in `server::service` (RequestLogger Drop, auth success/completed), `server::request` (parsed / body read), and `dispatcher::core` (5 handler-lifecycle events) → `debug!`. Unused `info` imports removed. Commit `perf(log): demote per-request tracing to debug (Phase 2.2)`.
- **Phase 5.1 (bounded queue)**: `WorkerPool::dispatch` now enforces `queue_bound` via the live `queue_depth` atomic. `Shed` mode: fail fast with 429 + `record_shed()`. `Block` mode: cooperative `may::coroutine::sleep(1ms)` up to `backpressure_timeout_ms`, then shed. Added `shed_mode_rejects_when_queue_full` unit test. Commit `feat(worker-pool): real bounded queue with shed/block semantics (Phase 5.1)`.
- **Verification after both**: re-run of the same 90 s smoke — pet_store **survived all 3 rounds** and remained serving HTTP 200 at end. **73,716 req/s (+26 %), 3.5 % failures (all `GET /` 404s against an unregistered route), zero aborted connections**, log output shrunk from ~1 MB to **240 lines**. `cargo test --lib` 293/293.
- PRD [`docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](../docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md) updated to v1.2.

## [2026-04-17] ship | Phase 0.1 — Box::leak removed from the request path

- `may_minihttp` fork branch `feat/response-header-owned-values` (commit `f9daffe`) adds `IntoResponseHeader` + `ResponseHeader { Static, Owned }`; `Response::header` now generic over static/owned inputs. Owned values drop with the response.
- BRRTRouter `Cargo.toml` repointed to that branch; `AppService::intern_keep_alive` deleted; 3 `Box::leak` call sites gone. `cargo test --lib` 292/292.
- `write_json_error` switched to `serde_json::to_vec` while we were in the file (PRD Phase 2.6 bonus).
- PRD [`docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](../docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md) updated to mark Phase 0.1 shipped; 0.2 subsumed; 0.3 (metrics path bound) is next.

## [2026-04-17] PRD | hot-path v2 — stability & perf

- Authored [`docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](../docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md). Scope: root-cause the Hauliage dev-env "microservice needs reboot" pattern and collapse previous hot-path findings into a phased plan.
- Phase 0 (stop the bleeding) removes per-response `Box::leak` (unbounded heap growth), bounds the metrics path `DashMap`, retires the `feat/configurable-max-headers` fork pin (our upstream PR merged: [`Xudong-Huang/may_minihttp#21`](https://github.com/Xudong-Huang/may_minihttp/pull/21)).
- Phases 1–6: `ArcSwap` router/dispatcher; header-name intern; defer radix `to_string`; parker-based reply channel; validator fast path; bounded worker pool; Goose v2 scenario matrix with JSON baselines.
- Updated [`index.md`](./index.md) to surface the PRD under a new "PRDs (active)" section.

## [2026-04-17] ingest | runtime map + generator + sibling wikis

- Added **`topics/runtime-stack-map.md`** — links `spec/` → `router/` → `dispatcher/` → `server/service.rs` with code anchors.
- Added **`topics/generator-cli-and-askama.md`** — `brrtrouter_gen`, `src/generator/`, `templates/*.txt`, consumer pointers to Hauliage scaffolding PRD/wiki.
- Added **`topics/sibling-repos-and-wikis.md`** — how BRRTRouter / Lifeguard / Hauliage wikis divide responsibility.
- Updated **`index.md`** to list the new topic pages.

## [2026-04-17] ingest | bootstrap llmwiki from docs + code
- Created initial `llmwiki/` structure.
- Imported full `docs/**/*.md` inventory into a catalog.
- Reconciled key operational docs against current code entrypoints.
- Added first functional pages for runtime request flow and generator flow.
- Recorded known baseline validation failures observed before doc changes.

## [2026-04-17] reconcile | CORS operations docs vs middleware/runtime
- Added dedicated reconciliation page: `llmwiki/reconciliation/cors-operations-vs-codebase.md`.
- Verified key CORS claims against middleware implementation and HTTP-level tests.
- Updated index and core reconciliation status to mark `docs/CORS_OPERATIONS.md` as verified.

## [2026-04-17] reconcile | performance docs vs current benchmarking/runtime anchors
- Added dedicated reconciliation page: `llmwiki/reconciliation/performance-docs-vs-codebase.md`.
- Verified benchmark/load-test anchors in `benches/**`, `examples/api_load_test.rs`, and CI workflow artifact handling.
- Captured drift for historical numeric claims, Goose flag terminology (`--increase-rate`), and stack-size default inconsistencies.

## [2026-04-17] fix | root out and correct doc inconsistencies identified in llmwiki analysis
- **`docs/DEVELOPMENT.md`**: Corrected `BRRTR_STACK_SIZE` default from `0x4000` to `0x8000` (32 KiB) to match `WorkerPoolConfig` runtime default.
- **`docs/ARCHITECTURE.md`**: Fixed `load_spec` return type in mermaid diagram (`(Spec, Vec<RouteMeta>)` → `(Vec<RouteMeta>, String slug)`) and step description ("Returns parsed `Spec` object" → correct return signature).
- **`docs/ARCHITECTURE.md`**: Updated router description from regex/O(n) to radix tree/O(k) in mermaid diagram, request-processing steps, key-components section, and performance-considerations section.
- **`docs/PERFORMANCE.md`**: Replaced non-existent `just flamegraph` recipe with `cargo flamegraph -p brrtrouter`.
- **`docs/GOOSE_LOAD_TESTING.md`**: Replaced all occurrences of obsolete `--hatch-rate` with `--increase-rate` and "Hatch Rate" with "Increase Rate".
- **`llmwiki/reconciliation/performance-docs-vs-codebase.md`**: Updated to `verified`; gaps marked as resolved.
- **`llmwiki/reconciliation/core-docs-vs-codebase.md`**: Marked architecture and performance drift items as resolved.

## [2026-04-17] contribute | post-415-fix wiki additions + path normalization
- Scoped to the companion `feat(server): reject undeclared Content-Type with HTTP 415` commit. Goal: leave the wiki one step more useful than before, focused on the concepts the 415 fix surfaced.
- Added **`llmwiki/entities/request-body-parsing.md`** — full Content-Type × body-shape matrix for `parse_request_body`, including the **pre-2026-04-17 multipart bypass history** (`Some(json!({}))` fabrication that silently made multipart requests pass §V1 schema validation against an empty object). Cross-linked to hauliage ADR 0016.
- Added **`llmwiki/topics/schema-validation-pipeline.md`** — end-to-end V1a / V1 / V2 / V6 / V7 pipeline with exact file anchors, pre-compilation via `validator_cache` (12 validators at fleet startup for 8 routes), and explicit catalog of things the pipeline does **not** currently do (no multipart parsing, no query-param validation against operation schema, no format-assertion enforcement).
- Added **`llmwiki/entities/route-meta.md`** — full 21-field catalog with `populated-from` and `consumed-by` per field; includes the new `request_content_types` field and the "adding a field touches 7 files" diff guide (test fixtures in six places).
- Added **`llmwiki/reference/openapi-extensions.md`** — audit of every `x-*` extension BRRTRouter recognises (`x-handler`, `x-brrtrouter-body-size-bytes`, `x-brrtrouter-stack-size` / `x-stack-size`, `x-sse`, `x-cors`, `x-brrtrouter-cors`, `x-ref-name`) plus hauliage-tooling injected ones (`x-service`, `x-service-base-path`, `x-brrtrouter-downstream-path`) and the latent `x-brrtrouter-impl` convention awaiting Fix A.
- **Normalised CI-runner absolute paths** (`/home/runner/work/BRRTRouter/BRRTRouter/...`) to repo-relative across five pre-existing wiki pages: `reconciliation/performance-docs-vs-codebase.md`, `reconciliation/cors-operations-vs-codebase.md`, `flows/code-generation-flow.md`, `flows/runtime-request-flow.md`, `reference/codebase-entry-points.md`. 43 path occurrences updated total. These paths came from the GitHub Actions Copilot workflow that bootstrapped the wiki.
- Updated `llmwiki/index.md` with new `Reference`, `Entities`, and `Topics` sections.
- **Convention going forward** (per user's ask): every BRRTRouter PRD / feature commit adds or extends 1–3 wiki pages tied to that work. Same schema (`Status` / `Source docs` / `Code anchors` / `Gaps`). Cross-link between hauliage ADRs and BRRTRouter wiki topics where concepts span both repos.

## [2026-04-17] ingest | canonical vs wip docs policy

- Added [`topics/canonical-docs-vs-wip.md`](./topics/canonical-docs-vs-wip.md); updated [`index.md`](./index.md) and [`docs-catalog.md`](./docs-catalog.md) synthesis table.
