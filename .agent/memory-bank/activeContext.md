# Active Context

## 2026-04-14 — CORS trusted `X-Forwarded-*`, Private Network Access, HTTP conformance tests

- **`trust_forwarded_host`** / **`CorsMiddlewareBuilder::trust_forwarded_host`** — same-origin uses `effective_server_authority` (`X-Forwarded-Host`, optional `X-Forwarded-Port`).
- **`allow_private_network_access`** — `Access-Control-Allow-Private-Network: true` on preflight (when `Access-Control-Request-Private-Network` present) and on cross-origin `after()` responses; `Vary` includes `Access-Control-Request-Private-Network` when enabled.
- **Tests:** `tests/cors_http_conformance_tests.rs` (raw HTTP); `test_cors_trust_forwarded_host_same_origin_skips_cors_headers` in `middleware_tests.rs`. Docs: `CORS_OPERATIONS.md`, `CORS_IMPLEMENTATION_AUDIT.md`.

## 2026-04-13 — CORS per-route disabled metric

- **Added:** `brrtrouter_cors_route_disabled_total` on `/metrics` (`MetricsMiddleware::inc_cors_route_disabled` / `cors_route_disabled()`), incremented once per request when `RouteCorsPolicy::Disabled` (`x-cors: false`) in `CorsMiddleware::before`.
- **Tests:** `test_cors_metrics_sink_route_disabled`; metrics endpoint asserts new series; docs: `CORS_OPERATIONS.md`, `CORS_IMPLEMENTATION_AUDIT.md`.

## 2026-03-25 — `ui_secure_endpoint_bearer` / `BearerJwtProvider` JWT payload decoding

- **Fixed:** Payload segments are decoded with **base64url (no padding)** first, then padded standard base64 for internal test tokens. E2E uses `PET_STORE_BEARER_DEV_TOKEN` (third segment `sig` matching default mock signature).
- **Memory bank:** See `progress.md` entry for 2026-03-25.

## 2025-03-24 — Flake `test_jwks_sub_second_cache_ttl_timing_accuracy`

- **Cause:** Test used `cache_ttl(200ms)` then slept **250ms** before the validation burst. By then `guard.0.elapsed() >= ttl`, so `refresh_jwks_if_needed` treated the cache as expired and issued extra JWKS HTTP fetches (2 vs 3+ depending on scheduling).
- **Fix:** Use **600ms** TTL and **120ms** warmup so the burst stays **well under** TTL; sleep **TTL + 200ms** before the post-expiry validate. Assert `<= 3` for the hot phase (initial + retry tolerance). Extended mock server accept loop to 32.

## 2026-04-18 — may_minihttp PR #24 follow-up: redundant \r\n\r\n pre-scan removed

- **Context:** PR #24 (`feat/response-header-owned-values`) still in review. Hot-path audit of `src/request.rs` `decode()` found a redundant O(n) `buf.windows(4).any(|w| w == b"\r\n\r\n")` pre-scan duplicating `httparse::Request::parse`'s own `Status::Partial` behaviour.
- **Proof:** Empirical probe across 15 input shapes confirmed: truncation (method / path / version / header-name / header-value / missing `\r\n\r\n`) always yields `Partial`; malformed bytes (`G@T`, space in path, etc.) always yield `Token` / `HeaderName` / `Version` — permanent failures where the pre-scan would only stall the connection. No case where the pre-scan converted a `Token` into a `Partial`.
- **Change:** One-line removal in `src/request.rs` decode() + doc comment. Replaces a per-request linear scan with `httparse`'s native Partial handling (identical semantics, one fewer allocation-free scan).
- **Branch:** `fix/remove-redundant-preparse-check` stacked on `feat/response-header-owned-values` (microscaler fork). When #24 merges, rebase onto master drops the two CI-fix commits automatically and this PR's diff becomes a single-file +15 -7.
- **Commit:** `8095cd2 perf(request): remove redundant \r\n\r\n pre-scan before httparse`. Committer: `Charles Sibbald <casibbald@gmail.com>` (no Cursor trailer).
- **CI mirror:** `clippy -- -D warnings` + `clippy --examples -- -D warnings` + `fmt --check` + `cargo test` all green. One pre-existing flake (`request_parsing::performance::benchmark_completion_check`, 2 s tight debug-build threshold on a test-local helper) fires identically on parent under full-suite concurrent load — unrelated to the diff.
- **Next:** Await #24 review → rebase → forward as upstream PR to `Xudong-Huang/may_minihttp`. BRRTRouter roadmap items still pending: Phase 2.3 (double path alloc), header-value intern, `write_json_error` body-formatting, Goose v2 scenario matrix — tracked in `docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`.

## 2026-04-18 (addendum) — may_minihttp flaky perf test fixed

- **Symptom:** `tests/request_parsing.rs` `performance::benchmark_completion_check` oscillated either side of its 2 s wall-clock bound (0.8 s isolated, 2.04 s under full-suite concurrent load). Sibling `benchmark_header_counting` had the same structure with a 100 ms bound. Both were benchmarking *test-local helpers* (`has_complete_headers`, `count_headers`), not production parser code.
- **Fix:** Marked both `#[ignore]` with module-level doc comment explaining the intent + invocation (`cargo test -- --ignored performance::`). Replaced the fragile wall-clock asserts with stable correctness asserts (sum-of-counts / hit-count), guarded by `std::hint::black_box` to prevent the optimiser hollowing out the loop.
- **Result:** default `cargo test` now reports **19 passed / 2 ignored** (was 21 passed, occasionally 1 FAIL); `cargo test -- --ignored performance::` runs both in ~0.5 s and passes cleanly.
- **Commit:** `9a96d18 test(request_parsing): ignore wall-clock perf smokes + assert correctness` on `fix/remove-redundant-preparse-check`. Clippy + fmt green.

## 2026-04-18 (integration branch) — may_minihttp rollup for BRRTRouter consumption

- **Branch created:** `integration/microscaler-fork` on `github.com:microscaler/may_minihttp` — contains **all 5 commits** from both open PRs:
  1. `f9daffe feat(response): allow owned header values (no Box::leak)`
  2. `7c9c9e7 ci: fix clippy + Dockerfile Rust version`
  3. `a442e32 ci: bump Dockerfile.test to Rust 1.88 for cookie_store/time/icu deps`
  4. `8095cd2 perf(request): remove redundant \r\n\r\n pre-scan before httparse`
  5. `9a96d18 test(request_parsing): ignore wall-clock perf smokes + assert correctness`
- **Purpose:** single stable tip for downstream consumers (BRRTRouter) while the two separate PRs wait upstream review. **No PR is open** for this branch — it's a rollup only.
- **Lifecycle:** delete once both upstream PRs land on `master`; BRRTRouter pins back to `master` / tag / crates.io release at that point.
- **BRRTRouter action item:** `Cargo.toml` currently pins `may_minihttp = { git = "…", branch = "feat/response-header-owned-values" }`. Consider switching the `branch = …` to `"integration/microscaler-fork"` to pick up the `decode()` pre-scan removal too. (Held pending user confirmation — not done in this session.)
- **CI at combined tip:** `clippy` (default + `--examples`) + `fmt --check` + `cargo test` all green.

## 2026-04-18 (correction) — may_minihttp `decode()` pre-scan removal REVERTED

- **Finding:** The `8095cd2 perf(request): remove redundant \r\n\r\n pre-scan before httparse` change was **not safe**. BRRTRouter's `server::response::tests::{test_write_handler_response_403_uses_forbidden_not_ok, test_cors_handler_response_error_round_trip_not_null}` started failing against the integration branch with `"failed to parse http request: Token"` on the keep-alive re-decode.
- **Root cause:** Re-ran the empirical probe against `httparse` 1.10 with a wider input set (including leftover body bytes from a prior keep-alive request). Results:
  - `whitespace only` → `Err(Token)` (not `Partial`)
  - `leading =1` (stale body bytes) → `Err(Token)`
  - `leading garbage then POST` → `Err(Token)`
  - `only body bytes no method` → `Err(Token)`
  All truncation cases of a *well-formed* request still yield `Partial`, but **any buffer whose first byte is not a valid method-token character fails immediately with `Token`**. The test sends `Content-Length: 11` with a 13-byte body, leaving `=1` in `req_buf` for the next keep-alive decode. The old pre-scan masked this by returning `Ok(None)` (no `\r\n\r\n` in `=1`), letting the server wait for a real next request or EOF. The new code surfaces a 500 and closes the connection before the 403 response is written.
- **Action taken:**
  1. Rebased `integration/microscaler-fork` to drop commit `8095cd2` (clean rebase, no revert commit). New tip `9175f9a`. Force-pushed.
  2. Refreshed BRRTRouter Cargo.lock → pins `9175f9a`. All 299 lib tests pass.
  3. Committed the pin change + dropped commit as `36379d0 chore(deps): pin may_minihttp to integration/microscaler-fork rollup branch` on `pre_BFF_work`.
- **Still broken:** `fix/remove-redundant-preparse-check` on may_minihttp remote still carries `8095cd2` and should either be (a) rebased to drop the commit, or (b) renamed / deleted entirely. The `PR_fix-remove-redundant-preparse-check.md` description is now incorrect — the pre-scan removal cannot ship.
- **Lesson for the PRD:** My empirical probe (15 inputs, all well-formed-truncation cases) didn't cover the two real-world bad-framing cases (`=1`, `whitespace-only`) that actually matter in a keep-alive loop. Any future `decode()` fast-path change must run against a *connection-level* harness, not just single-buffer inputs.

## 2026-04-18 — Phase R rerun: +15.6 % throughput, thermal-drift theory disproven

- **Setup:** 3 × 2000u × 600s Goose bench against pet_store on :8091. Current tip: R.1 + R.2 + `may_minihttp` pinned to `integration/microscaler-fork` (owned headers + log-hygiene fixes).
- **First attempt failed:** run 1 crashed pet_store with `SIGABRT` (exit 134) ~4 min in; runs 2 & 3 pushed 2000u at a dead socket, reporting 100 % connection errors. **The "thermal drift" hypothesis in the PRD §Phase R.1 was wrong.** The real cause was hot-path log-pipeline saturation:
  1. `router/core.rs` — `warn!("Slow route matching detected")` per-request whenever radix lookup > 1 ms (trivially true for multi-segment dynamic paths under real load).
  2. `middleware/memory.rs` — `warn!("High memory growth detected")` every 10 s during any legitimate 2000u ramp (coroutine stacks + connection buffers crossed +100 MB trivially).
  3. `runtime_config.rs` — forced odd stack size unconditionally, triggering `may`'s per-coroutine `println!` on every spawn.
- **Fix:** demoted (1) + (2) to `debug!` (raised threshold from 1 ms → 10 ms on the slow-match check), gated (3) behind `BRRTR_TRACK_STACK_USAGE=1`. Committed as `b1fc30b perf(logging): demote hot-path warn!s blocking SIGABRT under 2000u load` on `pre_BFF_work`.
- **Rerun results (post-log-cleanup):**
  - Run 1: 76,626 req/s, avg 25.31 ms, median 23 ms, max 759 ms
  - Run 2: 75,525 req/s, avg 25.66 ms, median 23 ms, max 655 ms
  - Run 3: 78,430 req/s, avg 24.71 ms, median 22 ms, max 653 ms
  - **Mean: 76,860 req/s ±1.9 %, 0 % connection errors.** The 6.7 % "fail" rate is exclusively `GET /` + `GET /docs` returning 404 (by design — pet_store doesn't register those routes, the Goose scenario hits them deliberately).
- **Δ vs Phase 0.3+2.1 arcswap baseline (66,484 req/s, 29.21 ms avg):** **+15.6 %** throughput, **−13.6 %** avg latency, **−11.5 %** p50. R.1 + R.2 are a clear, unambiguous win. The earlier "−8.8 %" regression was entirely SIGABRT+retry artefact.
- **Harness noise-floor correction:** ±10 % run-to-run variance previously attributed to "unmanaged macOS scheduler noise" was mostly SIGABRT+retry. The same harness now produces **±1.9 %** variance on the same hardware. The Phase 6 Goose v2 harness items (JSON output, per-scenario baselines, server-side quantiles) are still worthwhile but the "laptop too noisy for <15 % deltas" claim was overstated.
- **Files updated:**
  - `src/router/core.rs`, `src/middleware/memory.rs`, `src/runtime_config.rs` — log-level + gating fixes.
  - `docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md` — version bump to 1.7; §Phase R.1 "Initial interpretation" preserved for honesty, "Correct diagnosis" block added underneath, new §Phase R rerun section with the 76,860 / ±1.9 % numbers and comparison table.

## 2026-04-18 (correction) — BRRTRouter telemetry architecture clarification

- **Architecture:** runtime events flow via `tracing` → stdout (JSON) → Promtail → Loki, plus OTEL spans to the collector. Stdout is *not* a human console — it's the Promtail source. "Only startup logs to console" is an observability invariant (human-readable startup vs Promtail-scraped runtime stream), *not* a hot-path-silence constraint.
- **Reclassifying the three fixes from `b1fc30b` under the correct architecture:**
  - `router/core.rs` "Slow route matching" (per-request) — **fix kept**. Per-request alerts in Loki are noise in any sink; millions of duplicate events/min saturate both the tracing dispatcher and the stdout→Promtail pipeline. `warn!`→`debug!` + 1 ms → 10 ms threshold is correct. Comment rewritten to lead with the Loki-pollution argument.
  - `middleware/memory.rs` "High memory growth" (10 s cadence) — **fix partially reverted**. 10 s cadence is the correct rate for Loki; this is exactly what ops wants. The real bug was the +100 MB threshold firing on every legitimate 2000u ramp. **Restored `warn!`**, raised threshold **100 MB → 500 MB**. Companion info! line also restored (had been demoted to debug! in b1fc30b).
  - `runtime_config.rs` odd-stack opt-in — **fix kept**. `may::println!` bypasses `tracing` entirely, lands as raw unstructured text on stdout, never reaches Promtail/Loki. Thousands of direct stdout writes per second under 2000u is pure bench debris. Gating behind `BRRTR_TRACK_STACK_USAGE=1` is strictly correct. Comment updated to make the "bypasses tracing" point explicit.
- **Bench numbers unchanged:** the 76,860 req/s ±1.9 % result from earlier still stands — memory middleware fires at 10 s cadence, it contributes negligibly to throughput regardless of whether its single `warn!` call per poll is `warn!` or `debug!`.
- **PRD §Phase R.1 "Correct diagnosis" block rewritten** to describe the actual architecture and classify each source by pipeline position: (i) two tracing events that went through the correct pipeline but at wrong level/threshold, (ii) one raw println that bypassed the pipeline entirely. `§Phase R rerun` "Fixes committed" list updated to reflect the restored `warn!` + raised threshold on memory.
- **Commit:** `2b54c66 fix(logging): restore memory-growth warn!, align comments with OTEL architecture` on `pre_BFF_work`, pushed. Tests 299/299 green.

## 2026-04-18 (methodology correction) — bench scope control + canonical OpenAPI-only numbers

- **Methodology bug identified:** previous Phase R rerun (76,860 req/s ±1.9 %) mixed three unrelated code paths under one aggregate:
  1. OpenAPI-dispatched endpoints (what BRRTRouter is about) — Pet/User/Advanced APIs + `GET /labels/{color}`.
  2. `AppService` short-circuits — `/health`, `/metrics`, `/openapi.yaml` (documented at `src/server/service.rs:772-794` as "bypass the dispatcher for performance").
  3. Radix-miss 404s — `GET /`, `GET /docs` exit on first trie walk, never reach handler / schema / serde.
  Categories (2)+(3) together were ~20 % of traffic → inflated headline number, masked per-endpoint regressions.
- **Fix:** `examples/api_load_test.rs` now reads `BRRTR_BENCH_SCOPE` env var:
  - **`openapi` (default)** — only scenarios that traverse the full OpenAPI pipeline (radix → params → dispatcher → handler coroutine → schema validate → typed serde → response).
  - **`full`** — adds Built-in Endpoints + Static Files scenarios (preserved for smoke / end-to-end coverage; *not* for perf reporting).
  Commit `721da13 feat(bench): BRRTR_BENCH_SCOPE env var gates non-OpenAPI scenarios`.
- **Canonical OpenAPI-only numbers (3 × 2000u × 600s, post-log-cleanup, `may_minihttp` pinned to `integration/microscaler-fork`):**
  - Run 1: 77,710 req/s, avg 24.90 ms, median 22 ms, max 717 ms
  - Run 2: 83,036 req/s, avg 23.28 ms, median 21 ms, max 584 ms
  - Run 3: 83,933 req/s, avg 23.04 ms, median 20 ms, max 758 ms
  - **Mean: 81,560 req/s ±3.8 %, 0 % connection errors, 100 % [200]s.**
- **vs Phase 0.3+2.1 arcswap baseline (66,484 req/s, mixed-scope):** +22.7 % throughput, −18.7 % avg latency, −19.2 % p50. Cross-scope caveat: baseline was also mixed-scope, so both inflated by ~20 % short-circuit traffic — the ratio is honest, the absolute baseline overstates true OpenAPI throughput.
- **Surprise result — OpenAPI-only is FASTER than Full-scope (81,560 vs 76,860):** `/metrics` is not a cheap endpoint at scale (iterates `path_metrics` + `status_metrics` DashMaps, thousands of entries under 2000u load); `/` + `/docs` hit error-response formatting. These were server-side tax, not free short-circuits. Removing them released Goose client bandwidth that was being spent on them, which then pushed a smaller set of handler paths harder → more total req/s through the OpenAPI pipeline.
- **Run-to-run spread widened 1.9 % → 3.8 %** because per-scenario throughput is now dominated by genuine handler work (kernel scheduling + tracing-serialization jitter visible) rather than mostly-fixed-cost short-circuits. Still inside the 5 % target.
- **Client-bottleneck evidence:** 500u × 60s OpenAPI smoke showed avg 5.37 ms, vs 24 ms at 2000u. The client is the ceiling at 2000u. A proper L0–L5 scenario matrix (keep-alive on/off, pipelined 2/4/8) is needed to find the server's ceiling — captured as Phase 6.2.
- **PRD version bumped to 1.8.** §Phase R rerun table now shows both Full-scope and OpenAPI-only columns with methodology note; status line reflects new canonical number.
