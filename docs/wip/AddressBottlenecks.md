## Address Bottlenecks PRD

### Background

- **Observation**: `goose-report2.html` shows no request errors, but sustained load above ~5,000 concurrent users on a 4‑core Docker environment exhibits instability (throughput drop, rising latency, and/or timeouts).
- **Goal**: Restore headroom for sustained high concurrency by eliminating recent regressions in hot paths and introducing bounded backpressure.

### Current Architecture (relevant to performance)

- **HTTP runtime**: `may_minihttp` with May coroutines. `AppService` implements `HttpService` and orchestrates routing, security, validation, dispatch, and response.
- **Dispatch model**: One coroutine per handler/operation; requests are sent via unbounded MPSC channels; dispatcher waits synchronously for a reply.
- **Validation**: OpenAPI request and response schemas validated in `src/server/service.rs` using `jsonschema`.
- **Logging**: `tracing` with sampling and optional async buffering; extensive INFO logs at request/handler boundaries.
- **Metrics**: Prometheus-like metrics via `MetricsMiddleware`, with per-path aggregates behind `RwLock` maps and atomic counters.

### Findings: Bottlenecks and Instability Sources

1) Per-request JSON Schema compilation (CPU hotspot)
- Code path: `src/server/service.rs` compiles JSON Schema on every request and for every response validation.
- Impact: Excess CPU and allocations proportional to RPS; amplifies under sustained load.

2) Per-request body serialization for size accounting
- Code path: `src/server/service.rs` computes `total_size_bytes` by serializing the JSON body (`Value::to_string()`).
- Impact: Unnecessary CPU and allocations per request even when logs are sampled out.

3) Single-coroutine handlers + unbounded channels (no backpressure)
- Code path: `src/typed/core.rs` spawns one coroutine per operation; MPSC channel is unbounded; dispatcher does a blocking `recv()`.
- Impact: Hot endpoints serialize execution into one worker; queues grow without bounds → memory pressure, latency spikes, timeouts.

4) Logging sampling division-by-zero edge case
- Code path: `src/otel.rs` sampling layer computes `1.0 / sampling_rate` without guarding zero.
- Impact: With `BRRTR_LOG_SAMPLING_RATE=0` (or parsed to 0), can cause panics or undefined behavior under load.

5) Metrics per-path map lock contention
- Code path: `src/middleware/metrics.rs` uses a read-then-write `RwLock` upgrade pattern for per-path metrics.
- Impact: Under 5k+ concurrency, increased lock contention and latency (smaller than 1–3 but measurable).

6) High-volume INFO logs in hot paths
- Code path: dispatcher and service emit INFO logs at request start/complete and handler boundaries.
- Impact: Extra CPU and I/O pressure if not sufficiently sampled/async-buffered in production.

### Requirements

- **R1**: Eliminate per-request schema compilation overhead by caching compiled validators.
- **R2**: Remove unnecessary per-request body serialization in hot path.
- **R3**: Introduce bounded backpressure and parallelism per handler to avoid unbounded queues and single-worker bottlenecks.
- **R4**: Harden logging sampling to be robust to zero/invalid rates; keep logging overhead predictable under load.
- **R5**: Reduce lock contention in metrics path or make it negligible at 5k+ concurrency.
- **R6**: Maintain functional parity (validation, security, metrics) and keep existing tests green; add new tests for performance invariants.
- **R7**: Compute per-handler coroutine stack size from the OpenAPI path/spec and set it during template generation (handlers/controllers) to balance memory vs. depth/complexity.

### Proposed Design Changes

1) JSON Schema Validator Caching (R1)
- Precompile JSON Schema validators per route and response status at startup or first-use with `OnceLock`/lazy map.
- Store on route metadata or in `AppService` caches keyed by `(handler_name, kind=request|response, status?)`.
- Use `Arc<JSONSchema>` to share across coroutines; validation becomes lock-free.
- Config: `BRRTR_SCHEMA_CACHE=on|off` (default on) for rollback.

2) Per-Handler Coroutine Stack Sizing (R7)
- Compute a recommended stack size per operation during code generation based on OpenAPI-derived signals:
  - Path complexity (count of path/query/header params)
  - Request/response schema depth and max object nesting
  - SSE/streaming endpoints and controllers with deeper call stacks
  - Custom vendor extension override: `x-brrtrouter-stack-size` (bytes)
- Heuristic (initial):
  - Base = 16 KiB; +4 KiB per 5 params; +4–16 KiB for deep schemas (depth tiers: >6, >12); +8 KiB if SSE/streaming
  - Clamp to `[MIN, MAX]` where `MIN=16 KiB`, `MAX=256 KiB` (env-tunable)
- Generator changes:
  - Compute per-handler `stack_size_bytes` in `src/generator` while processing routes
  - Emit into `templates/controller.rs.txt`/`templates/registry.rs.txt` so each spawned coroutine uses `may::coroutine::Builder::new().stack_size(stack_size_bytes)`
  - Keep global fallback via `may::config().get_stack_size()` if no per-handler size is specified
- Configuration and overrides:
  - Env per-handler: `BRRTR_STACK_SIZE__<HANDLER_NAME>=bytes`
  - Global caps: `BRRTR_STACK_MIN_BYTES`, `BRRTR_STACK_MAX_BYTES`
  - OpenAPI vendor extension takes precedence over heuristic; env overrides take final precedence

3) Body Size Without Serialization (R2)
- Prefer `Content-Length` header if present; never serialize body to compute size.
- If header is absent, use a spec-derived estimate computed at generation time:
  - Estimate from OpenAPI schema: sum of `maxLength` for strings (fallback heuristics), `maxItems * itemSize` for arrays, bounded object property sizes; clamp to sane bounds.
  - Consider content types; only estimate `application/json` initially; fallback for others.
  - Support vendor override: `x-brrtrouter-body-size-bytes` at operation level to explicitly set estimate.
- Generator changes:
  - Compute `estimated_body_bytes` per operation in `src/generator` while processing schemas.
  - Emit the estimate into generated controllers/registry so `AppService` logging can use it when `Content-Length` is unavailable.
- Runtime behavior:
  - Use `Content-Length` if present; otherwise use the generated estimate; avoid `Value::to_string()` in hot path.
- Config: vendor extension takes precedence; optional env override per handler if needed.

4) Handler Worker Pools + Bounded Queues (R3)
- For each handler/operation, spawn `N` worker coroutines (default 4 or `min(cores, 8)`), fronted by a bounded MPSC queue.
- Dispatch strategy: round-robin sender selection or single bounded queue feeding workers.
- Backpressure: when queue is full, either shed (429) or block with timeout (configurable) before sending.
- Config:
  - `BRRTR_HANDLER_WORKERS=<int>` default 4
  - `BRRTR_HANDLER_QUEUE_BOUND=<int>` default 1024
  - `BRRTR_BACKPRESSURE_MODE=block|shed` default block
  - `BRRTR_BACKPRESSURE_TIMEOUT_MS=<int>` default 50
- Metrics: expose per-handler queue depth and shed count.

5) Logging Sampling Hardening (R4)
- Clamp rate into [0.0, 1.0]. Treat `<= 0.0` as “error-only” for non-error events; avoid division-by-zero.
- Optional: move some INFO logs to DEBUG or ensure production defaults use `sampled` with sane non-zero rate.
- Config: existing `BRRTR_LOG_SAMPLING_MODE`/`BRRTR_LOG_SAMPLING_RATE` honored.

6) Metrics Contention Reduction (R5)
- Option A: Switch per-path maps to a sharded structure (e.g., `dashmap`) to avoid read→write upgrade cost.
- Option B: Pre-register known paths at startup to avoid write path entirely in steady state.
- Keep atomic counters relaxed ordering as-is.

7) Production Logging Volume (R4/R6)
- Ensure production preset uses async buffered logging and sampling; audit INFO logs in hottest paths; demote to DEBUG if non-essential.

8) Routing Optimization – Phase 1 (Hybrid: Static Map + Segment Matcher)
- Per-method static map for fully static paths (exact match O(1)).
- Fallback segment-wise matcher for parameterized routes (no regex), with static-before-dynamic precedence.
- Zero regex in the common path; keep regex only for exotic or legacy patterns behind a feature flag.
- Config: `BRRTR_ROUTER_MODE=regex|hybrid|trie` (default: hybrid once proven); `BRRTR_ROUTER_LOG=basic|trace` for debug.

9) Routing Optimization – Phase 2 (Trie/Radix)
- Build a trie/radix tree at startup from the OpenAPI spec (static-over-dynamic priority, catch-alls at leaves).
- Atomic swap on hot reload (build off-thread, replace pointer). No route lookup locks at runtime.
- Deterministic matching with param index/types recorded for zero-copy extraction.

10) Allocation Cuts in Hot Path
- Avoid per-request `HashMap`/`String` allocations for path params; use `SmallVec`/stack locals for small N, and borrow slices from the request path.
- Precompute param indices/types from OpenAPI; parse with `str::parse` by type where applicable.
- Reuse small buffers where safe; minimize intermediate JSON/value cloning.

11) Middleware Hygiene (Metrics/Tracing/Auth)
- Metrics: atomics only; bounded label cardinality; avoid locks. Keep histograms via pre-sized structures.
- Tracing: default to sampled + async; keep span fields minimal in hot paths.
- Auth: fast-path unauthenticated routes; avoid heavy token work when not required; cache JWKS securely.

### Non-Goals / Out of Scope

- Swapping out `may_minihttp` runtime or changing HTTP protocol stack.
- Implementing end-to-end distributed tracing export (kept for a later phase).
- Redesigning validation semantics (remain functionally equivalent).

### Acceptance Criteria & Tests (TDD)

- AC1: Validator caching
  - Unit tests: cached compile path is hit after first call; validates same as pre-change.
  - Benchmark: request validation CPU time reduced by ≥80% vs. per-request compile.

- AC2: Per-handler stack sizing
  - Unit tests: generator computes `stack_size_bytes` from sample specs (param counts, depth tiers, SSE flag)
  - Golden-files: generated controllers/registry use `Builder::stack_size(expected_bytes)`
  - Env override tests: `BRRTR_STACK_SIZE__HANDLER` wins over heuristic; caps respected
  - Runtime assertion (optional dev-only): spawned coroutine `stack_size()` equals computed value

- AC3: Body serialization removal
  - Unit/integration tests: logs still include headers and other fields; body-size computation does not serialize body at INFO level.
  - Unit tests for estimator: derive reasonable estimates from representative OpenAPI schemas (strings with maxLength, arrays with maxItems, nested objects).
  - Vendor override tests: `x-brrtrouter-body-size-bytes` replaces heuristic; env override (if set) supersedes both.
  - Golden-files: generated code contains `estimated_body_bytes` for operations.
  - Benchmark: per-request CPU reduced measurably (microbenchmark around size computation removed).

- AC4: Worker pools and bounded queues
  - Unit tests: N workers spawned per handler; bounded capacity respected; shed/block behavior correct.
  - Integration tests: under synthetic hot endpoint, latency remains bounded; no unbounded memory growth.
  - Metrics: queue depth and shed counters exposed and increasing under pressure.

- AC5: Logging sampling robustness
  - Unit tests: `sampling_rate=0` does not panic; non-error events are dropped; errors still logged.

- AC6: Metrics contention
  - Microbenchmarks: `record_path_metrics` shows reduced contention vs. baseline at 5k parallel updates.

- AC7: Regression safety
  - All existing tests pass; response/request validation behavior unchanged.

- AC8: Routing Phase 1 (Hybrid)
  - Static paths matched via O(1) map; dynamic paths matched without regex.
  - Benchmarks with 50/100/200 routes show ≥5–20× match speedup vs regex scan; no correctness regressions.
  - Feature flag allows toggling between regex and hybrid for A/B.

- AC9: Routing Phase 2 (Trie)
  - Trie enabled behind flag; atomic swap on hot reload; deterministic static-over-dynamic precedence.
  - Benchmarks show additional ≥2× improvement over hybrid on large dynamic route-sets; parity on small sets.

- AC10: Allocation Cuts
  - Allocations/request reduced measurably (heap profiles); param extraction zero-copy for typical routes.
  - No regressions in param correctness; types parsed per OpenAPI metadata.

- AC11: Middleware Hygiene
  - Metrics hot path free of locks; label cardinality bounded; tracing overhead ≤ configured sampling target.
  - Auth fast-paths verified on public routes; secure behavior maintained on protected routes.


### Rollout Plan

1. Implement validator caching and logging sampling guard (low-risk, immediate CPU wins).
2. Integrate per-handler stack sizing in generator/templates; validate memory and stability under load.
3. Remove body serialization from hot path (minor refactor).
4. Introduce worker pools with bounded queues behind feature flags/env.
5. Switch metrics per-path map to sharded/pre-registered approach.
6. Tune defaults in production presets; verify with load tests.
7. Spike branches (isolated) for:
   - router-phase1-hybrid (static map + segment matcher)
   - router-phase2-trie (trie/radix with atomic swap)
   - alloc-cuts-hotpath (param SmallVec/stack, no per-request HashMap)
   - middleware-hygiene (metrics/tracing/auth refinements)
   Each branch benchmarked independently against main/hybrid; no cross-edits.


### Configuration Summary

- `BRRTR_SCHEMA_CACHE=on|off` (default on)
- `BRRTR_HANDLER_WORKERS=<int>` (default 4)
- `BRRTR_HANDLER_QUEUE_BOUND=<int>` (default 1024)
- `BRRTR_BACKPRESSURE_MODE=block|shed` (default block)
- `BRRTR_BACKPRESSURE_TIMEOUT_MS=<int>` (default 50)
- Existing logging envs (`BRRTR_LOG_*`) continue to apply; guard zero sampling.
- Per-handler stack sizing:
- `BRRTR_STACK_SIZE__<HANDLER_NAME>=<bytes>` (override heuristic)
- `BRRTR_STACK_MIN_BYTES` (default 16384), `BRRTR_STACK_MAX_BYTES` (default 262144)
- Body size estimate:
- OpenAPI vendor extension `x-brrtrouter-body-size-bytes` at operation level
- Optional env override `BRRTR_BODY_SIZE__<HANDLER_NAME>=<bytes>` (if present, overrides vendor/heuristic)
 - Routing mode:
 - `BRRTR_ROUTER_MODE=regex|hybrid|trie` (feature flag for A/B)
 - `BRRTR_ROUTER_LOG=basic|trace` (debug verbosity for matching)

### Risks & Mitigations

- Risk: Caching incorrect validators if routes mutate at runtime → Hook cache invalidation into hot-reload path; or tie cache to route version hash.
- Risk: Bounded queues cause request shedding → expose metrics and clear 429s; make blocking mode default with short timeout.
- Risk: More coroutines per handler increase memory → keep small stack size (16–32 KB) and tune workers by env.
- Risk: Mis-sized stacks cause overflows or wasted memory → clamp within `[MIN, MAX]`, provide env/vendor overrides, and add monitoring for stack overflow signals.
- Risk: Spec-derived body size estimate may be inaccurate → allow vendor/env overrides; when `Content-Length` is present, emit metric comparing actual vs estimate to guide tuning.

### Success Metrics

- Sustain ≥5,000 concurrent users for ≥30 minutes on 4‑core Docker with:
  - p95 latency within SLO (project-specific), error rate < 0.5%.
  - Stable RSS (no linear growth), no timeouts from handler queue saturation.
  - CPU: ≥20–40% headroom vs. baseline at equal throughput.

### Implementation Checklist

- [ ] Cache JSONSchema validators (request/response) keyed by route/status.
- [ ] Compute per-handler `stack_size_bytes` during generation from OpenAPI complexity.
- [ ] Emit `Builder::stack_size(stack_size_bytes)` in generated controllers/registry; add env and vendor overrides.
- [ ] Remove body `to_string()` in size accounting; rely on Content-Length or omit.
 - [ ] Compute `estimated_body_bytes` from OpenAPI schema during generation; emit into generated code.
 - [ ] Add vendor extension `x-brrtrouter-body-size-bytes` support and optional env override handling.
- [x] Introduce handler worker pools with bounded queues and env config.
- [x] Add per-handler queue depth and shed metrics; dashboards.
- [ ] Guard sampling-rate zero/invalid in `SamplingLayer`.
- [ ] Reduce INFO logs in hot path or confirm production sampling/async buffering presets.
- [ ] Optimize metrics per-path storage (dashmap or pre-registration).
- [ ] Load testing: reproduce >5k sustained users; document results and tuning.
- [ ] Implement router Phase 1 (hybrid): per-method static map + segment matcher; flag-gated.
- [ ] Implement router Phase 2 (trie): radix/trie build + atomic swap; flag-gated.
- [ ] Apply allocation cuts in routing/param extraction (SmallVec/stack, zero-copy slices).
- [ ] Middleware hygiene: metrics hot path atomics only; tracing sampling verified; auth fast-paths.




## R3 Implementation: Worker Pools with Bounded Queues

### Overview

R3 has been successfully implemented, introducing bounded worker pools with backpressure handling for all handler operations. This eliminates the single-coroutine bottleneck and prevents unbounded memory growth under high load.

### Implementation Details

#### 1. Worker Pool Module (`src/worker_pool.rs`)

Created a comprehensive worker pool infrastructure with:

- **WorkerPoolConfig**: Configuration struct supporting environment variables
  - `BRRTR_HANDLER_WORKERS` (default: 4) - Number of worker coroutines per handler
  - `BRRTR_HANDLER_QUEUE_BOUND` (default: 1024) - Maximum queue depth
  - `BRRTR_BACKPRESSURE_MODE` (block | shed, default: block) - Backpressure strategy
  - `BRRTR_BACKPRESSURE_TIMEOUT_MS` (default: 50ms) - Timeout for block mode

- **BackpressureMode**: Two strategies for handling queue overflow
  - **Block**: Wait with timeout before retrying (default, safe for most use cases)
  - **Shed**: Return 429 (Too Many Requests) immediately (aggressive load shedding)

- **WorkerPoolMetrics**: Real-time monitoring of worker pool health
  - `queue_depth` - Current number of requests in queue (gauge)
  - `shed_count` - Total requests shed due to backpressure (counter)
  - `dispatched_count` - Total requests dispatched to pool (counter)
  - `completed_count` - Total requests completed by workers (counter)

- **WorkerPool**: Core implementation
  - Spawns N worker coroutines sharing a single MPSC channel
  - Implements bounded queue behavior using atomic counters (may's MPSC is unbounded)
  - Provides `dispatch()` method with backpressure handling

#### 2. Dispatcher Integration (`src/dispatcher/core.rs`)

Extended Dispatcher with worker pool support:

- Added `worker_pools: HashMap<String, Arc<WorkerPool>>` field to track worker pools
- New methods:
  - `register_handler_with_pool()` - Register handler with default config from env vars
  - `register_handler_with_pool_config()` - Register handler with custom config
  - `worker_pool_metrics()` - Get metrics for all worker pools
- Updated `dispatch()` method to check for worker pools and apply backpressure
- Maintains backward compatibility with single-coroutine handlers

#### 3. Typed Handler Support (`src/typed/core.rs`)

Added worker pool support for typed handlers:

- New method: `register_typed_with_pool()` for typed handlers with worker pools
- Wraps typed handler logic in a closure compatible with worker pool dispatch
- Maintains type safety and automatic validation

#### 4. Metrics Exposure (`src/server/service.rs`)

Exposed worker pool metrics in `/metrics` endpoint:

```prometheus
# Worker Pool Metrics
brrtrouter_worker_pool_queue_depth{handler="handler_name"} gauge
brrtrouter_worker_pool_shed_total{handler="handler_name"} counter
brrtrouter_worker_pool_dispatched_total{handler="handler_name"} counter
brrtrouter_worker_pool_completed_total{handler="handler_name"} counter
```

#### 5. Comprehensive Testing (`tests/worker_pool_tests.rs`)

Added 5 integration tests:
- `test_worker_pool_creation` - Verifies pool creation and configuration
- `test_worker_pool_shed_mode` - Tests immediate shedding with 429 responses
- `test_worker_pool_block_mode` - Tests blocking with timeout
- `test_worker_pool_metrics` - Validates metrics tracking
- `test_worker_pool_config_from_env` - Tests environment variable configuration

### Usage Examples

#### Basic Usage (Default Configuration)

```rust
let mut dispatcher = Dispatcher::new();

unsafe {
    dispatcher.register_handler_with_pool("my_handler", |req: HandlerRequest| {
        // Handle request with parallel processing
        // ...
    });
}
```

This creates a worker pool with 4 workers, queue bound of 1024, and block mode backpressure.

#### Custom Configuration

```rust
let config = WorkerPoolConfig::new(
    8,      // 8 workers for high throughput
    2048,   // larger queue
    BackpressureMode::Shed,  // aggressive shedding
    100,    // 100ms timeout (not used in shed mode)
    0x10000, // 64KB stack size
);

unsafe {
    dispatcher.register_handler_with_pool_config("high_load_handler", handler_fn, config);
}
```

#### Monitoring

Worker pool metrics are automatically exposed at `/metrics`:

```bash
curl http://localhost:8080/metrics | grep worker_pool
```

Expected metrics:
- Queue depth should stay well below queue_bound under normal load
- Shed count should be zero or very low in block mode
- Dispatched and completed counts should track closely (queue draining)

### Performance Characteristics

#### Benefits

1. **Parallel Processing**: Multiple workers process requests concurrently per handler
2. **Bounded Memory**: Queue depth cap prevents unbounded growth
3. **Graceful Degradation**: Backpressure provides controlled behavior under overload
4. **Observable**: Metrics enable proactive monitoring and capacity planning

#### Tradeoffs

1. **Memory**: N workers * stack_size per handler (default: 4 * 64KB = 256KB per handler)
2. **Latency**: Block mode adds up to timeout_ms latency under extreme load
3. **Complexity**: More coroutines to manage vs. single-coroutine model

#### When to Use Worker Pools

- **Use worker pools for**:
  - Handlers with high request volume
  - Handlers that perform I/O or blocking operations
  - Handlers where parallel processing improves throughput
  
- **Use single coroutine for**:
  - Low-traffic handlers
  - Handlers that must serialize requests (e.g., stateful operations)
  - Handlers with very short execution time (<1ms)

### Validation

All existing tests pass (140 tests), plus 5 new worker pool tests. The implementation:
- Maintains backward compatibility with existing handlers
- Provides smooth upgrade path (handlers can opt-in to worker pools)
- Exposes comprehensive metrics for monitoring
- Supports both aggressive (shed) and conservative (block) backpressure modes

### Next Steps

1. **Load Testing**: Run goose tests with >5k concurrent users to validate stability
2. **Production Tuning**: Gather metrics to tune worker count and queue depth per handler
3. **Documentation**: Update user-facing docs with best practices
4. **Code Generation**: Integrate worker pool config into OpenAPI-driven code generation (future work)

