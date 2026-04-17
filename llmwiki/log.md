# LLM Wiki Log

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
