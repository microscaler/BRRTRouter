# LLM Wiki Log

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
