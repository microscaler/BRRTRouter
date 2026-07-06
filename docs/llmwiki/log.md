# BRRTRouter llmwiki — log

Append-only. Newest entries at the **bottom**.

---

## [2026-04-18] perf | Phase 4 — validation hot path

- `AppService::call`: cap JSON Schema `iter_errors` at **64** per request/response (`MAX_JSON_SCHEMA_ERRORS`); DEBUG log uses `?schema.get("required")` instead of allocating `Vec<String>` for required keys each time.
- See [`topics/schema-validation-pipeline.md`](./topics/schema-validation-pipeline.md).

## [2026-04-18] docs | llmwiki — Phase 4 + 6 landing

- Added `docs/llmwiki/` with [`index.md`](./index.md), [`SCHEMA.md`](./SCHEMA.md), [`topics/schema-validation-pipeline.md`](./topics/schema-validation-pipeline.md), [`topics/bench-harness-phase-6.md`](./topics/bench-harness-phase-6.md).
- **Phase 4:** Maps runtime validation to `validator_cache`, `AppService::call` (V1a/V2/V1&V3), `parse_request_body`; lists optimization ideas (measure first).
- **Phase 6:** Documents Criterion entry points + macro stress checklist (thermal drift, baselines).
- PRD cross-links updated to point here instead of a missing-only path.

---

## [2026-04-18] bench | Criterion `schema_validation_hot_path`

- Added [`benches/schema_validation_hot_path.rs`](../../benches/schema_validation_hot_path.rs): `schema_iter_errors_valid_body`, `schema_iter_errors_invalid_body`, `schema_cache_get_or_compile_hit`.
- **Purpose:** Phase 4 / Phase 6 — measure JSON Schema validation without 10-minute thermal drift; save/compare with `--save-baseline ms02` / `--baseline ms02` on the ms02 host (see [`topics/bench-harness-phase-6.md`](./topics/bench-harness-phase-6.md)).

**Sample output (2026-04-18, one machine — record your own):**

| Benchmark | Time (est.) |
|-----------|----------------|
| `schema_iter_errors_valid_body` | ~102 ns |
| `schema_iter_errors_invalid_body` | ~155 ns |
| `schema_cache_get_or_compile_hit` | ~639 ns |

Invalid-body path is slower (more `iter_errors` work). Cache hit is dominated by `RwLock` read + `HashMap` lookup per `get_or_compile` (see `validator_cache` rustdoc).

---

## [template] MS02 baseline refresh

When benches move to a **new ms02** or new CPU, reset Criterion baselines on that host only. Run **`just bench-baseline-ms02`** or **`just bench-baseline-ms02-all`** — the printed tag is **`ms02-<short-sha>-<YYYYMMDD>`** (see [`topics/bench-harness-phase-6.md`](./topics/bench-harness-phase-6.md)). Copy this block and fill in:

- **Date:** (wall time when you ran the bench)
- **Host:** `uname -a` →
- **Rust:** `rustc -Vv` →
- **Commit:** `git rev-parse HEAD` →
- **Criterion baseline tag:** (paste from `just` output, e.g. `ms02-a1b2c3d-20260418`)

---

## [2026-07-06] docs | Sesame-IDAM workaround cleanup backlog

- Added [`topics/sesame-idam-workarounds-cleanup.md`](./topics/sesame-idam-workarounds-cleanup.md) — implementation tasks **BR-1** through **BR-7** to remove sesame-idam / hauliage workarounds.
- **Next staged:** **BR-1** — `security: []` must mean public (fix `src/spec/build.rs` + oas3 presence tracking). Symptom: sesame in-cluster login 401 until global security removed from OpenAPI specs.
- Cross-linked from sesame [`topic-brrtrouter-refactor-backlog.md`](../../seasame-idam/docs/llmwiki/topics/topic-brrtrouter-refactor-backlog.md) and hauliage [`sesame-idam-brrtrouter-integration.md`](../../hauliage/docs/llmwiki/topics/sesame-idam-brrtrouter-integration.md).
- Phase 1 `brrtrouter::http` migration documented as complete; BR-5..BR-7 track platform hygiene (may::go refresh, reqwest shed, JWT sub-spans).

## [2026-07-06] fix | BR-1 — security:[] semantics

- Implemented `OperationSecurityPresence` + `resolve_operation_security` in `src/spec/security_presence.rs`.
- `load_spec` / `load_spec_full` pass presence into `build_routes_with_security_presence`.
- Added `tests/spec_security_tests.rs` (3 tests). Sesame restored global security on login/session specs.
- See sesame [`topic-brrtrouter-refactor-backlog.md`](../../seasame-idam/docs/llmwiki/topics/topic-brrtrouter-refactor-backlog.md) pm13 entry.

## [2026-07-06] fix | BR-1b — in-cluster HTTP JWKS URLs

- Allow `http://*.svc.cluster.local` JWKS URLs (`085e67e`). Unblocks hauliage fleet pod JWKS fetch in Kind.

## [2026-07-07] fix | BR-1c — HTTP fetch path-only URI

- `fetch_get_http` / `fetch_post_http` send path+query only to `may_http` (`73744df`). Fixes in-cluster JWKS GET from hauliage pods.
- Tests: `tests/http_fetch_tests.rs`. Hauliage `sesame_jwks_smoke` 3/3 on ms02 after redeploy.

## [2026-07-07] docs | Wave 2 staged

- Wiki sync: BR-1b/BR-1c done; consumer Wave 2 = HI-7 pin + company/bff redeploy. Cross-ref hauliage integration wiki.
