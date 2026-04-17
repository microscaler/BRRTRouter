# Performance Docs vs Codebase Reconciliation

- Status: verified
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

## Gaps / drift (all resolved)

1. ✅ `docs/PERFORMANCE.md` empirical tables are labelled as historical snapshots (community data) — no code change required.
2. ✅ `docs/GOOSE_LOAD_TESTING.md` updated: `--hatch-rate` → `--increase-rate`; "Hatch Rate" → "Increase Rate" throughout.
3. ✅ `docs/DEVELOPMENT.md` stack-size default corrected: `0x4000` → `0x8000` (32 KiB) to match runtime default in `WorkerPoolConfig`.
4. ✅ `docs/PERFORMANCE.md` flamegraph command corrected: non-existent `just flamegraph` → `cargo flamegraph -p brrtrouter`.
