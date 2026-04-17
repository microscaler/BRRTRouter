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
