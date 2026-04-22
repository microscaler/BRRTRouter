# BRRTRouter LLM Wiki Schema

## Purpose
This wiki is a persistent, code-anchored knowledge layer between `docs/` and the Rust codebase.

## Source of Truth Order
1. Runtime behavior in `src/**` and `tests/**`
2. Generated behavior in `templates/**` and `src/generator/**`
3. Existing prose docs in `docs/**`
4. This wiki (`llmwiki/**`) as reconciled synthesis

## Page Conventions
- Every substantive page includes:
  - **Status** (`verified`, `partially-verified`, `unverified`)
  - **Source docs** (`docs/...` links)
  - **Code anchors** (absolute repository paths)
  - **Gaps / drift** (doc claim vs code reality)
- Prefer explicit file paths and function names over high-level claims.
- Keep operational instructions executable and minimal.

## Operational Workflows
- **Ingest**: add/refresh entries from `docs/**` into `llmwiki/docs-catalog.md`, then reconcile with code.
- **Query**: answer from `llmwiki/index.md` + linked pages first, then verify in code when uncertain.
- **Lint**: regularly check for stale claims and unresolved gaps in `llmwiki/reconciliation/*.md`.
- **Auto-research perf**: scheduled or background perf work follows [`topics/auto-research-perf-loop.md`](./topics/auto-research-perf-loop.md); charter tables live in `auto-research/docs/`; use `python auto-research/scripts/perf_iteration.py` from repo root for the printable checklist.

## Logging
- Append session updates to `llmwiki/log.md`.
- Keep entries chronological and append-only.
