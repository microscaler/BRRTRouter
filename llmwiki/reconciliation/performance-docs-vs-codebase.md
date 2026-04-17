# Performance Docs vs Codebase Reconciliation

- Status: partially-verified
- Source docs: `docs/PERFORMANCE.md`, `docs/GOOSE_LOAD_TESTING.md`, `docs/flamegraph.md`

## Verified implementation anchors

1. Criterion benchmarks are present and wired in workspace config:
   - `/home/runner/work/BRRTRouter/BRRTRouter/benches/throughput.rs`
   - `/home/runner/work/BRRTRouter/BRRTRouter/benches/jwt_cache_performance.rs`
   - `/home/runner/work/BRRTRouter/BRRTRouter/Cargo.toml`
2. Goose load-test example exists and is exercised in CI:
   - `/home/runner/work/BRRTRouter/BRRTRouter/examples/api_load_test.rs`
   - `/home/runner/work/BRRTRouter/BRRTRouter/.github/workflows/ci.yml`
3. Router/request hot-path structures use `SmallVec` and radix routing as documented in performance narratives:
   - `/home/runner/work/BRRTRouter/BRRTRouter/src/router/core.rs`
   - `/home/runner/work/BRRTRouter/BRRTRouter/src/server/request.rs`
4. Backpressure/shed behavior is implemented with explicit overload responses:
   - `/home/runner/work/BRRTRouter/BRRTRouter/src/dispatcher/core.rs`
5. Global stack-size runtime override exists via `BRRTR_STACK_SIZE` parsing:
   - `/home/runner/work/BRRTRouter/BRRTRouter/src/worker_pool.rs`

## Reconciled conclusions

- The repo supports benchmark and load-test workflows (`cargo bench`, Goose example, CI artifact uploads), so the docs are directionally correct on available performance tooling.
- The codebase does include the optimization pillars highlighted in docs (SmallVec hot-path use, radix routing, load shedding under pressure).

## Gaps / drift

1. `docs/PERFORMANCE.md` contains many empirical throughput/latency tables and cross-framework comparisons that are not directly reproducible from versioned benchmark artifacts in-repo; treat these as historical snapshots unless refreshed from current CI artifacts.
2. `docs/PERFORMANCE.md` still references Goose ramp terminology as “hatch rate”, while the current example/CI usage uses Goose `--increase-rate`:
   - `/home/runner/work/BRRTRouter/BRRTRouter/examples/api_load_test.rs`
   - `/home/runner/work/BRRTRouter/BRRTRouter/.github/workflows/ci.yml`
3. Stack-size baseline statements are inconsistent across docs vs runtime defaults. Current runtime default in `WorkerPoolConfig` is `0x8000` (32 KiB):
   - `/home/runner/work/BRRTRouter/BRRTRouter/src/worker_pool.rs`
   - `/home/runner/work/BRRTRouter/BRRTRouter/docs/DEVELOPMENT.md`
4. `just bench` / `just flamegraph` are documented in `docs/PERFORMANCE.md`, but these just recipes are not currently present in `/home/runner/work/BRRTRouter/BRRTRouter/justfile`; prefer direct cargo commands in operational docs unless recipes are reintroduced.
