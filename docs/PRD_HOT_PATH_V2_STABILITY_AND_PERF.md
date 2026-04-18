# PRD: BRRTRouter hot-path v2 — stability, memory, and performance

**Project:** BRRTRouter
**Document version:** 1.3
**Date:** 2026-04-18 (Phases 0.1 / 2.2 / 5.1 / 1 shipped)
**Status:** Active — Phases 0.1 / 2.2 / 5.1 / 1 shipped; Phase 0.3 next; upstream `may_minihttp` PR pending
**Owner:** BRRTRouter core
**Target branch:** `pre_BFF_work` (lands in phases; merges through small PRs)
**Primary driver:** Hauliage dev-env stability — eliminate the "microservice keeps needing reboots" class of failure.

---

## Table of contents

1. [Executive summary](#1-executive-summary)
2. [Problem statement (why this PRD exists)](#2-problem-statement-why-this-prd-exists)
3. [Upstream state of `may_minihttp` (PR #21)](#3-upstream-state-of-may_minihttp-pr-21)
4. [Root-cause catalog](#4-root-cause-catalog)
5. [Goals and non-goals](#5-goals-and-non-goals)
6. [Phased plan](#6-phased-plan)
    - [Phase 0 — Stop the bleeding (stability)](#phase-0--stop-the-bleeding-stability)
    - [Phase 1 — Lock-free hot path](#phase-1--lock-free-hot-path)
    - [Phase 2 — Allocation discipline](#phase-2--allocation-discipline)
    - [Phase 3 — Zero per-request channel](#phase-3--zero-per-request-channel)
    - [Phase 4 — Schema / validation hardening](#phase-4--schema--validation-hardening)
    - [Phase 5 — Metrics & backpressure truth](#phase-5--metrics--backpressure-truth)
    - [Phase 6 — Goose v2 harness](#phase-6--goose-v2-harness)
7. [Targets and measurements](#7-targets-and-measurements)
8. [Rollout / CI story](#8-rollout--ci-story)
9. [Risks](#9-risks)
10. [Open questions](#10-open-questions)
11. [References / code anchors](#11-references--code-anchors)

---

## 1. Executive summary

Hauliage microservices — all built on **BRRTRouter** — keep reaching a state during long development sessions where individual service pods need to be manually restarted. Symptoms: slowly growing RSS, increasing p99, eventual socket / handler stalls. The router itself is fast, but it has a handful of **unbounded-growth** and **per-request-allocation** hot spots that, under long-running load (or a busy dev day of Playwright + BDD + curl traffic), behave like a slow leak.

This PRD collapses a recent deep read of the hot path and the Goose harness into a single, **phased** remediation plan. The first phase is **purely corrective** (stop the leak); subsequent phases are performance and observability upgrades that also make the framework cheaper per pod in Kubernetes.

**The highest-value single change** in this document is Phase 0.1: fix the per-response `Box::leak` in [`src/server/response.rs`](../src/server/response.rs) and [`src/server/service.rs`](../src/server/service.rs). On its own it turns "restart the service every few hours" into "runs for days".

---

## 2. Problem statement (why this PRD exists)

### 2.1 Observed behavior in Hauliage dev

- **RSS climbs** on every service (fleet, consignments, bff, etc.) during a day of work, even when request rate is modest.
- **p99 latency** on the `/api/...` surface gradually degrades.
- **Manual `kubectl rollout restart`** (via Tilt) temporarily restores normal behavior.
- No single handler is at fault — the drift is **framework-level**, visible in every BRRTRouter-backed service.
- At the router surface, `brrtrouter_active_requests` stays low, but the process's `VmRSS` rises monotonically.

### 2.2 What this PRD changes

- Removes the leak; removes per-request allocations that contribute to RSS and allocator fragmentation.
- Removes a coarse read-lock from every request, which will cut p99 spikes at concurrency.
- Upgrades the load-testing harness so we can **prove** these fixes work, per-PR, with stable numbers.

### 2.3 What this PRD is NOT

- Not a rewrite. `may` + `may_minihttp` stay; the BFF decision (see [Lifeguard GraphQL PRD](../../lifeguard/docs/llmwiki/topics/graphql-optional-feature.md)) stays; OpenAPI-first with BRRTRouter codegen stays.
- Not a request for a new runtime. We keep coroutines / `may`.
- Not a security PRD. JWT / API-key paths are touched minimally (Phase 4.3) and only to avoid doing the same work twice.

---

## 3. Upstream state of `may_minihttp` (PR #21)

Our contribution PR [`Xudong-Huang/may_minihttp#21`](https://github.com/Xudong-Huang/may_minihttp/pull/21) — "Fix TooManyHeaders Error and Enable Comprehensive Load Testing" — **merged 2026-01-05**. It delivered:

- **`MaxHeaders` enum** + const-generic `decode()` (our `HttpServerWithHeaders<_, 32>` usage in [`src/server/http_server.rs`](../src/server/http_server.rs) is now upstream API).
- Test-infrastructure fixes (MAY runtime race, readiness checks, keep-alive support).
- 6 Goose header scenarios.

**Implication for this PRD:**
- We can **stop pinning the fork branch** (`feat/configurable-max-headers`) in [`Cargo.toml`](../Cargo.toml) once the next `may_minihttp` release is cut — Phase 0.2.
- However, PR #21 **does not** change the response-header API. [`Response::header`](https://github.com/Xudong-Huang/may_minihttp/blob/master/src/response.rs) still takes `&'static str` and stores `[&'static str; MAX_HEADERS]`. Our `Box::leak` pattern still exists and still leaks. Phase 0.1 requires **a second upstream PR** ("non-`'static` response header values") or a narrow fork maintained only for that API.

---

## 4. Root-cause catalog

Each item below is the product of reading the actual source on the current `pre_BFF_work` tip. File paths + line ranges are repository-relative.

### 4.1 **BLEED-1 (CRITICAL): Per-response header `Box::leak`**

**Where:** [`src/server/response.rs:39–43`](../src/server/response.rs), [`src/server/service.rs:1033–1034, 1333–1335, 1497–1503`](../src/server/service.rs).

```39:43:../src/server/response.rs
for (k, v) in headers {
    let header_value = format!("{k}: {v}").into_boxed_str();
    res.header(Box::leak(header_value));
}
```

Because `may_minihttp::Response::header` requires `&'static str`, every non-static response header (including `x-request-id: <ulid>`, `accept-post: <list>`, dynamic `content-type`) is leaked to the program heap **forever**. Under load the intern mitigation (the keep-alive intern map in `service.rs`) protects one specific header — everything else grows without bound.

Under Hauliage traffic this is the dominant memory-growth source and the most likely direct cause of the reboot cadence.

### 4.2 **BLEED-2 (high): Unbounded `DashMap` keys in metrics**

**Where:** [`src/middleware/metrics.rs`](../src/middleware/metrics.rs) — `path_metrics: DashMap<String, Arc<PathMetrics>>` and `status_metrics: DashMap<(String, u16), AtomicUsize>`.

Pre-registration at startup covers known paths; 404s with novel paths (crawlers, mis-aimed Playwright runs, fuzz traffic) insert new `String` keys on every request. Over a day of dev traffic this can grow into thousands of entries.

### 4.3 **BLEED-3 (medium): Keep-alive intern map unbounded**

**Where:** [`src/server/service.rs::intern_keep_alive`](../src/server/service.rs).

The intern is keyed on the **full header value** (`Keep-Alive: timeout=..., max=...`). If anyone ever parameterises timeout per-connection this becomes unbounded. Not a current leak, but a foot-gun.

### 4.4 **PERF-1 (high): `RwLock` on router/dispatcher in the hot path**

**Where:** [`src/server/service.rs`](../src/server/service.rs) around `self.router.read()` / `self.dispatcher.read()`.

Two lock acquisitions per request. Under contention (hot reload, metrics scrape that holds `dispatcher.read()`), p99 spikes. Already called out in [`docs/JSF/PERFORMANCE_OPTIMIZATION_PRD.md`](./JSF/PERFORMANCE_OPTIMIZATION_PRD.md) Phase 3.

### 4.5 **PERF-2 (high): Unconditional per-request work for debug logging**

**Where:** [`src/server/request.rs::parse_request`](../src/server/request.rs).

`format!("{:?}", req.version())`, `headers.iter().map(...).take(20).collect()`, `headers.iter().map(...).sum()` run **every request** regardless of `RUST_LOG`. Same pattern repeats in `server::service::call` (header size sum, total size bytes, span field prep).

### 4.6 **PERF-3 (high): No header-name interning**

**Where:** [`src/server/request.rs::parse_request`](../src/server/request.rs).

```rust
Arc::from(h.name.to_ascii_lowercase().as_str())
```

Every request allocates a fresh `String` (from `to_ascii_lowercase`), then wraps in an `Arc<str>`, for **every** header — even for the ~10 common names (`content-type`, `authorization`, `x-api-key`, `content-length`, `host`, `user-agent`, `accept`, `accept-encoding`, `connection`, `cookie`, `x-request-id`).

### 4.7 **PERF-4 (medium): Radix allocates on failed param branches**

**Where:** [`src/router/radix.rs:218–225`](../src/router/radix.rs).

```rust
params.push((Arc::clone(param_name), segment.to_string()));
```

`segment.to_string()` runs even for branches that later fail. With multiple `param_children` per node and a deep path, one 404 can allocate several strings that are immediately backtracked.

### 4.8 **PERF-5 (high): Per-request reply `mpsc::channel()`**

**Where:** [`src/dispatcher/core.rs::dispatch_with_request_id`](../src/dispatcher/core.rs).

```rust
let (reply_tx, reply_rx) = mpsc::channel();
```

`may::sync::mpsc` is Mutex+Condvar under the hood. Creating one per request is heavy for a **single-reply** handoff. Also `request.clone()` before `tx.send(request.clone())` copies the full `HandlerRequest`.

### 4.9 **PERF-6 (medium): JWT double-verification**

**Where:** [`src/server/service.rs`](../src/server/service.rs) — after `provider.validate(...)` succeeds, the code calls `provider.extract_claims(...)` in a second loop over `route_match.route.security`. For JWKS-backed Bearer flows this can re-parse and re-verify the token.

### 4.10 **PERF-7 (low): `write_json_error` double-allocates body**

**Where:** [`src/server/response.rs::write_json_error`](../src/server/response.rs).

`body.to_string().into_bytes()` — allocates a JSON `String`, then into `Vec<u8>`. `serde_json::to_vec` is a single allocation.

### 4.11 **PERF-8 (low): `iter_errors().collect::<Vec<_>>()` always**

**Where:** [`src/server/service.rs`](../src/server/service.rs) request + response validation blocks.

Even on the hot common case (no errors) we materialise a `Vec`. `is_valid()` first, then `iter_errors()` on the failing path.

### 4.12 **PERF-9 (low): `info!` in request-completion paths**

**Where:** multiple. RequestLogger `Drop` emits `info!`; dispatcher emits `info!("Request dispatched to handler")` and `info!("Handler response received")`. Field formatting is lazy, but callsite dispatch still has cost at ~20k QPS. These are really debug-level.

### 4.13 **HARNESS-1 (medium): Goose suite does not catch slow leaks**

- 2 min × 20 users is too short to see the leak above.
- "Instrumented" harness measures client-side time and labels it router time.
- No 404 churn / big-body / many-headers / pipelining scenario.
- No JSON report consumed; driver regex-parses ASCII.

---

## 5. Goals and non-goals

### 5.1 Goals

1. **Stop unbounded memory growth in steady-state traffic.** Hauliage pods must run for a **dev day (8h)** at load without RSS growth signalling a leak, and for a **week** under moderate continuous traffic.
2. **Remove request-path lock contention.** `router.read()` / `dispatcher.read()` must not appear in flamegraphs of steady-state traffic.
3. **Reduce per-request allocation count to near-zero for the common case** (auth + routed + small JSON body + schema validated).
4. **Make the Goose harness a trustworthy regression gate.** JSON output, stable scenarios, baselines in-repo, PR comments.
5. **Keep backward compatibility** with Hauliage's generated code. No breaking changes to `HandlerRequest` / `HandlerResponse` / `spawn_typed*` signatures in Phase 0–2.

### 5.2 Non-goals

- Replacing `may` with Tokio.
- Redesigning OpenAPI extensions or codegen templates.
- Changing BFF direction (see Lifeguard GraphQL wiki topic — GraphQL is not the BFF path).
- Implementing the `Tier 2` manifest + 3-way merge regeneration model (tracked in the Hauliage BFF Scaffolding Remediation PRD).
- Rewriting the security model. Phase 4.3 is a small local optimisation only.

---

## 6. Phased plan

Each phase ships as its own PR. Each PR must include Goose v2 numbers (once Phase 6 lands) or baseline client-side numbers before/after.

---

### Phase 0 — Stop the bleeding (stability)

**Goal:** Eliminate the memory-growth class of failure that drives the reboot cadence.

- **0.1 — Non-`'static` response header values.** ✅ **SHIPPED (2026-04-17).**
    - `may_minihttp` fork branch [`feat/response-header-owned-values`](https://github.com/microscaler/may_minihttp/tree/feat/response-header-owned-values) (commit `f9daffe`) adds `IntoResponseHeader` + `ResponseHeader { Static(&'static str), Owned(Box<str>) }`; `Response::header<H: IntoResponseHeader>` now accepts `&'static str`, `String`, `Box<str>`, `Cow<'static, str>`. Owned values are freed with the response. 3 new unit tests + existing doc test pass; 23+21 existing integration tests unchanged.
    - BRRTRouter `Cargo.toml` now points at the microscaler fork's `feat/response-header-owned-values` branch (replacing the `feat/configurable-max-headers` pin, which was already merged upstream as PR #21).
    - All 3 `Box::leak` call sites removed from [`src/server/service.rs`](../src/server/service.rs) and [`src/server/response.rs`](../src/server/response.rs). `AppService::intern_keep_alive` and `INTERN` static deleted. `keep_alive_header: Option<&'static str>` → `Option<Box<str>>`, cloned per response.
    - `write_json_error` switched from `body.to_string().into_bytes()` to `serde_json::to_vec` (Phase 2.6 bonus — trivial while here).
    - Verification: `cargo test --lib` 292 / 292 pass, including the CORS 403 round-trip + 415 `Accept-Post` regression guards.
    - **Upstream PR**: [`Xudong-Huang/may_minihttp#24`](https://github.com/Xudong-Huang/may_minihttp/pull/24) raised 2026-04-18. While it is in review, BRRTRouter continues to consume the branch from the microscaler fork.
- **0.2 — Retire the `feat/configurable-max-headers` fork pin.** ✅ **Subsumed by 0.1.**
    - Now pinned at `feat/response-header-owned-values` (which is a descendant of the merged `feat/configurable-max-headers` work). When upstream merges 0.1's PR we can switch to the next `may_minihttp` release tag.
- **0.3 — Bound the metrics path map.** ⏳ **Next.**
    - In [`src/middleware/metrics.rs`](../src/middleware/metrics.rs) add: (a) a single `__unmatched` key for 404s, (b) a configurable soft cap (`BRRTR_METRICS_PATH_MAX`, default 4096) after which inserts go to `__other`.

**Acceptance criteria for Phase 0:**
- Soak test: 1k users, 60 min continuous load, RSS growth ≤ 2 % over the last 45 min window (i.e. flat). No `Box::leak` call in the request path (grep-gated in CI).
- No change in observable behavior for a Hauliage `cargo test --workspace` run.

---

### Phase 1 — Lock-free hot path

**Goal:** Remove router/dispatcher `RwLock` from the request path.

- **1.1 — `arc_swap::ArcSwap<Router>`** in `AppService`. ✅ **SHIPPED (2026-04-18).**
- **1.2 — `arc_swap::ArcSwap<Dispatcher>`** similarly. ✅ **SHIPPED (2026-04-18).**
- **1.3 — Update `src/hot_reload.rs`** to use `ArcSwap::store` / copy-on-write semantics for Dispatcher. ✅ **SHIPPED (2026-04-18).**

**Implementation notes:**
- Added `arc-swap = "1.7"` to the root `Cargo.toml` and `examples/pet_store/Cargo.toml`.
- New public type aliases in [`src/server/service.rs`](../src/server/service.rs): `SharedRouter = Arc<ArcSwap<Router>>`, `SharedDispatcher = Arc<ArcSwap<Dispatcher>>`. Consumers migrate from `Arc<RwLock<Router>>` mechanically via `ArcSwap::from_pointee(router)`.
- Hot-path reads collapse from 4-line `RwLock::read().expect(...)` blocks to `self.router.load().route(method, &path)` and `self.dispatcher.load()`; no lock poisoning.
- `hot_reload::watch_spec` publishes via `router.store(Arc::new(new_router))` and, for dispatcher, `load_full()` → clone → mutate → `store`. This is the correct RCU pattern: readers holding the old Arc keep serving until their `Guard` drops.
- Mechanically updated ~15 call sites (tests + `cli/commands` + `pet_store` + generator templates) via a scripted pass.

**Acceptance criteria (measured 2000u × 600s against `pet_store` on 8091):**

| Metric | Pre Phase 1 | Post Phase 1 | Δ |
|---|---:|---:|---:|
| Throughput | 55,001 req/s | **60,575 req/s** | **+10.1 %** |
| Avg latency | 35.40 ms | **32.09 ms** | **−9.4 %** |
| p99 latency | 130 ms | **110 ms** | **−15.4 %** |
| Real 5xx | 0 | **0** | = |

Baselines committed at [`benches/baselines/2000u-600s.json`](../benches/baselines/2000u-600s.json) (pre) and [`benches/baselines/2000u-600s-arcswap.json`](../benches/baselines/2000u-600s-arcswap.json) (post). `cargo test --lib` 293/293 + all 11 building integration test binaries pass (162 integration tests).

---

### Phase 2 — Allocation discipline

**Goal:** Zero allocation for the common request (known headers, routed, valid JSON).

- **2.1 — Header-name intern table.** Static `phf::Map<&'static [u8], Arc<str>>` for the ~15 common header names. In `parse_request`, look up first, fall back to `Arc::from(lowercased)` only on miss. Expected effect: eliminate 90 %+ of `String` allocations in the hot path.
- **2.2 — Demote per-request tracing to `debug!`.** ✅ **SHIPPED (2026-04-18).**
    - **Motivation**: The Goose smoke against pet_store on 8081 triggered a **SIGABRT (exit 134)** in pet_store after ~152 s under 20-user load, preceded by **~2,800 synchronous `WARN "No route matched"` log writes/sec**. The log pipeline was itself the bottleneck.
    - **Change**: demoted `warn!("No route matched")` in [`src/router/core.rs`](../src/router/core.rs) to `debug!`; demoted per-request `info!` in [`src/server/service.rs`](../src/server/service.rs) (`RequestLogger` Drop, `Authentication completed`, `Authentication success`), [`src/server/request.rs`](../src/server/request.rs) (`HTTP request parsed`, `Request body read`), and [`src/dispatcher/core.rs`](../src/dispatcher/core.rs) (5 handler-lifecycle events) to `debug!`. Kept startup-time and conditional slow-path `warn!`s. Removed unused `info` imports.
    - **Verification**: at `RUST_LOG=brrtrouter=warn`, the 90 s / 20-user / ~74 k req/s smoke produced **240 lines of server output total** (vs **1,044,735 bytes** before). `cargo test --lib` 293/293.
    - **Remaining (future PR)**: gate the precomputed debug fields themselves (`http_version = format!(...)`, `header_names`, `size_bytes`) behind `tracing::enabled!(Level::DEBUG)` so the formatting work itself disappears — this one-line fix was deferred to keep the Phase 2.2 PR scoped.
- **2.3 — Avoid double path allocation.** Store `raw_path: String` once; keep the pre-`?` slice as `&str` into the stored path until routing is done.
- **2.4 — Defer radix param `to_string()`** until the terminal branch is known accepted. Walk-to-match first, materialise param values on the accepting branch.
- **2.5 — Body `Vec::with_capacity`** from `Content-Length` when present.
- **2.6 — `write_json_error` → `serde_json::to_vec`.**
- **2.7 — Validator: `is_valid()` fast path** for the no-error case; keep `iter_errors()` for the failure path.

**Acceptance criteria:** at 1k users, `heap allocations / request` (from a short `dhat` run) drops by ≥60 %. `cargo bench throughput` shows ≥15 % improvement on the `router_hot_path` benchmark.

---

### Phase 3 — Zero per-request channel

**Goal:** Eliminate the per-request reply-channel allocation.

- **3.1 — Reuse reply channel per dispatch caller.** Server coroutine holds a channel it reuses; or better:
- **3.2 — Replace reply channel with a parker.** Handler writes into a `UnsafeCell<Option<HandlerResponse>>` protected by an `AtomicU32` state flag, then `may::coroutine::park`/`unpark`. Dispatcher allocates this "slot" once per connection (on first use), reuses it across requests on that connection.
- **3.3 — Stop cloning `HandlerRequest`.** Move the request into `tx.send`, keep `handler_name: Arc<str>` and `path: Arc<str>` separately in the dispatcher state machine to preserve logs without cloning.
- **3.4 — Replace `HandlerResponse::set_header` `String` insert** with an append-only model; `write_handler_response` is the only consumer and does not need remove semantics.

**Acceptance criteria:** at 2k users, allocator count/request drops to within 2× the theoretical minimum (body bytes + JSON output only). `dispatch_with_request_id` no longer appears as a top-10 allocator in a `dhat` trace.

---

### Phase 4 — Schema / validation hardening

**Goal:** Don't do work twice.

- **4.1 — `SecurityProvider::validate` returns `Option<Value>` claims.** Remove the re-scan that calls `extract_claims`. For providers that only validate and do not have claims, `Ok(None)` is fine.
- **4.2 — Validator cache miss metric.** Count misses (Prometheus `brrtrouter_validator_cache_miss_total`) so a mis-configured spec-reload that clears the cache shows up in Grafana.
- **4.3 — Validator precompile on reload.** After `ArcSwap::store` in Phase 1, immediately call `precompile_schemas` for the new spec, before the next request arrives.

**Acceptance criteria:** JWT-auth'd endpoints show a measurable drop (≥20 %) in `security_validate_duration_us` at 1k users. `brrtrouter_validator_cache_miss_total` stays at 0 in steady state.

---

### Phase 5 — Metrics & backpressure truth

**Goal:** Make the metrics surface honest.

- **5.1 — Bounded worker-pool queue.** ✅ **SHIPPED (2026-04-18).**
    - Kept the existing `may::sync::mpmc` channel (coroutine-friendly blocking `recv`) and added a **soft bound** via the live `WorkerPoolMetrics::queue_depth` atomic. `dispatch_with_shedding` now fails fast with `HandlerResponse::error(429, …)` + `record_shed()` when depth ≥ `queue_bound`. `dispatch_with_blocking` cooperatively yields with `may::coroutine::sleep(1ms)` up to `backpressure_timeout_ms`; on timeout it sheds the same 429 (never hangs the caller).
    - Shared path: new `send_to_pool` records the dispatch, undoes the counter if the channel is disconnected (→ 503, distinct from a 429).
    - Behavior preservation: `queue_bound=0` disables the gate (old unbounded behavior); default remains 1024.
    - Tests: new `shed_mode_rejects_when_queue_full` + 3 pre-existing worker-pool tests pass. Full `cargo test --lib` 293/293.
    - Combined with 2.2, this eliminates the SIGABRT: the re-run 90 s smoke (same 20-user / same scenarios) now completes cleanly with the server still serving — **73,716 req/s, 3.5 % failures (all `GET /` 404s against an unregistered root route), zero aborted connections**, vs the pre-fix **58,427 req/s, 41.1 % failures including 684 k status=0 aborts + SIGABRT**.
- **5.2 — Per-handler histograms.** Replace `PathMetrics` avg/min/max with a 10-bucket fixed histogram (same bucket layout as `HISTOGRAM_BUCKETS`) to expose real quantiles per handler.
- **5.3 — `brrtrouter_box_leak_bytes_total` counter (transitional).** Increments whenever the hot path allocates a leaked header (before Phase 0.1 ships). Removed once leak sites are gone. Guards against regression.

**Acceptance criteria:** `curl /metrics` on a service with a slow handler shows `brrtrouter_worker_pool_shed_total{handler="..."}` incrementing under sustained overload. Per-handler p95 visible in Grafana.

---

### Phase 6 — Goose v2 harness

**Goal:** Turn the Goose suite into a **trustworthy regression detector** that runs in CI and catches the classes of bug this PRD fixes.

- **6.1 — Shared scenario module.** Factor transactions from `examples/api_load_test.rs` and `examples/adaptive_load_test.rs` into `examples/load/common.rs`. Weights stay in each binary.
- **6.2 — Scenario matrix (layered).**
    - **L0 — floor:** isolated `GET /__bench/echo` (no auth, no schema). Establishes router ceiling.
    - **L1 — routing:** static, single-param, multi-param, label, deep path, **404 churn (random paths)**.
    - **L2 — I/O shape:** small JSON GET, small POST, 32 KiB POST, 128 KiB POST, query-param explosion (32), many-headers (30).
    - **L3 — auth:** none, API-key, Bearer valid, Bearer expired, Bearer wrong scope.
    - **L4 — validation:** schema pass, schema fail (invalid field), schema fail (missing required), undeclared Content-Type (415).
    - **L5 — concurrency:** keep-alive off, keep-alive on, pipelined 2/4/8.
- **6.3 — JSON output only.** Drop the regex-in-Python parser in [`scripts/run_goose_tests.py`](../scripts/run_goose_tests.py); consume Goose's `--report-file report.json`.
- **6.4 — Server-side quantile consumption.** Driver queries Prometheus `histogram_quantile` windowed to the cycle length; prints server-side vs client-side side-by-side so "network / client overhead" is visible.
- **6.5 — Baselines in-repo.** `benches/baselines/<scenario>.json` committed. CI compares a PR run against `main`'s baseline and posts a PR comment.
- **6.6 — Retire the client-side `MetricsCollector`.** Delete or rewrite `examples/performance_metrics_load_test.rs` — Prometheus + Goose JSON replaces it.
- **6.7 — Soak job.** Optional-label CI workflow (`load:soak`) runs 1k users × 30 min and uploads an RSS-over-time chart. Blocks merge only when explicitly requested.

**Acceptance criteria:** A PR that re-introduces `Box::leak` in the response path would cause the soak job's RSS chart to climb; CI fails. A PR that regresses p99 on L1 by >10 % fails the PR comment diff check.

---

## 7. Targets and measurements

| Metric                                  | Current (estimated from docs + dev observation) | Target after all phases |
|-----------------------------------------|--------------------------------------------------|--------------------------|
| RSS growth over 8h dev day              | Monotonic climb                                  | Flat within 2 %          |
| p99 @ 2k users (L1 routing)             | ~120 ms                                          | ≤ 60 ms                  |
| p99 spike during `/metrics` scrape      | Significant                                      | Invisible                |
| Allocations / request (dhat count)      | Dozens                                           | Single digits common     |
| Leaked bytes / response (typical)       | 30–80 B / header / request, unbounded            | 0                        |
| Goose CI run produces machine-readable  | No (ASCII regex)                                 | Yes (JSON + baselines)   |

---

## 8. Rollout / CI story

- Every phase is **one or more small PRs**. No big-bang.
- Phase 0 ships first, on its own, with a soak-test artefact. This is what buys us dev-env stability.
- `scripts/run_goose_tests.py` stays wired to the existing CI step during 0–5. Phase 6 swaps it.
- Each PR's description must include:
    - Before/after numbers from the smoke load test (u=20, t=60s) — always runs.
    - Before/after numbers from L1 routing + 404 churn (u=200, t=60s) — for Phase 1 onwards.
    - A note on `dhat` allocator count — for Phase 2 onwards.
- Soak job is **label-gated** — we don't run 30 min on every PR.

---

## 9. Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `may_minihttp` upstream rejects the non-`'static` header PR | Medium | Keep the narrow fork branch. We've already shipped one upstream PR; the maintainer is responsive. |
| `ArcSwap` subtle ordering issues with hot reload | Low | `arc_swap` is mature and widely used. Add a stress test that hot-reloads the spec 1000× under load. |
| Header intern misses due to case drift | Low | Build the static map case-insensitive; always look up with `to_ascii_lowercase`'s byte form via a small inline function. |
| Parker-based reply regresses under extreme concurrency | Medium | Benchmark Phase 3 against Phase 2 baseline; keep a fallback `mpsc` path behind a feature flag for one release. |
| Real bounded worker-pool queue changes behavior for existing Hauliage services | Low | Default `queue_bound` stays 1024; mode defaults to `Block` which is compatible with current "unbounded" behavior (just with a latency ceiling). |
| Metrics path bounding makes legitimate novel routes invisible | Low | Cap is soft + log + metric. Operators can raise it. |

---

## 10. Open questions

- **Q1.** Should Phase 3's parker land before Phase 2's allocation work, or after? (Perf data will drive; default order in this PRD is 2 → 3.)
- **Q2.** Do we want a per-pod hard RSS ceiling in BRRTRouter itself (a watchdog that forces `process::exit` at N MB) as a safety net during Phase 0 rollout, or trust the soak-test gate?
- **Q3.** Should `brrtrouter_box_leak_bytes_total` be kept permanently (regression-guard) or removed once Phase 0.1 is merged? (Currently proposes transitional.)
- **Q4.** When we drop the `may_minihttp` fork pin (Phase 0.2), do we also revert the `may` fork patch in `Cargo.toml` `[patch.crates-io]`? (Separate decision; track in the next `may` release cut.)
- **Q5.** Do we publish Phase 6's baseline JSON files under `benches/baselines/` or under `docs/perf/baselines/`?

---

## 11. References / code anchors

### Hot-path files touched by this PRD

- [`src/server/service.rs`](../src/server/service.rs) — Phase 0.1, 1.1, 1.2, 2.2, 4.1, 4.3
- [`src/server/request.rs`](../src/server/request.rs) — Phase 2.1, 2.2, 2.3, 2.5
- [`src/server/response.rs`](../src/server/response.rs) — Phase 0.1, 2.6
- [`src/server/http_server.rs`](../src/server/http_server.rs) — Phase 0.2
- [`src/router/core.rs`](../src/router/core.rs) — Phase 1.1
- [`src/router/radix.rs`](../src/router/radix.rs) — Phase 2.4
- [`src/dispatcher/core.rs`](../src/dispatcher/core.rs) — Phase 3.1, 3.2, 3.3
- [`src/middleware/metrics.rs`](../src/middleware/metrics.rs) — Phase 0.3, 5.2, 5.3
- [`src/worker_pool.rs`](../src/worker_pool.rs) — Phase 5.1
- [`src/validator_cache.rs`](../src/validator_cache.rs) — Phase 4.2, 4.3

### Existing docs superseded / extended

- [`docs/JSF/PERFORMANCE_OPTIMIZATION_PRD.md`](./JSF/PERFORMANCE_OPTIMIZATION_PRD.md) — covers Phase 1–3 conceptually; **this PRD extends it** with Phase 0 (the stability gap) and Phase 6 (harness).
- [`docs/GOOSE_LOAD_TESTING.md`](./GOOSE_LOAD_TESTING.md) — current Goose usage doc; Phase 6.2 will add a "Scenario matrix" section.
- [`docs/PERFORMANCE.md`](./PERFORMANCE.md) — headline numbers; update when Phase 2 / 3 land.

### Upstream references

- `may_minihttp` PR #21 (merged 2026-01-05): [Xudong-Huang/may_minihttp#21](https://github.com/Xudong-Huang/may_minihttp/pull/21) — `MaxHeaders` landed; response-header lifetime did **not**.
- `may_minihttp/src/response.rs` (current upstream) — confirms `Response::header(&'static str)` API unchanged.

### Related cross-repo context

- Hauliage BFF scaffolding + lifecycle concerns — [`../../hauliage/docs/PRD_BFF_SCAFFOLDING_REMEDIATION.md`](../../hauliage/docs/PRD_BFF_SCAFFOLDING_REMEDIATION.md).
- Lifeguard `raw SQL` / ORM policy — governs what Hauliage services build on top of BRRTRouter ([`../../lifeguard/docs/llmwiki/topics/raw-sql-vs-selectquery-policy.md`](../../lifeguard/docs/llmwiki/topics/raw-sql-vs-selectquery-policy.md)).
- Lifeguard `graphql` optional feature — why BFF composition is OpenAPI/BRRTRouter, not GraphQL ([`../../lifeguard/docs/llmwiki/topics/graphql-optional-feature.md`](../../lifeguard/docs/llmwiki/topics/graphql-optional-feature.md)).
