# Auto-research perf loop

**Status:** `verified` (process doc; gates depend on your Tilt/CI setup)

## Purpose

Define **how** to run the BRRTRouter **autonomous / scheduled perf research** loop: time budget, charter location, wiki updates, and commit discipline.

## Source docs

- Charter + tables: [`auto-research/docs/PERF_CONTROL_SURFACE_AND_LOG.md`](../../auto-research/docs/PERF_CONTROL_SURFACE_AND_LOG.md)
- Scripts: [`auto-research/scripts/README.md`](../../auto-research/scripts/README.md)
- Bench harness / MS02: [`docs/llmwiki/topics/bench-harness-phase-6.md`](../../docs/llmwiki/topics/bench-harness-phase-6.md)
- Agent rules: [`AGENTS.md`](../../AGENTS.md)

## Code anchors

- Hot path: `src/server/service.rs`, `src/validator_cache.rs`
- Benches: `benches/schema_validation_hot_path.rs`, `benches/throughput.rs`, `benches/jwt_cache_performance.rs`

## How to conduct one iteration

1. **Read** [`auto-research/docs/PERF_CONTROL_SURFACE_AND_LOG.md`](../../auto-research/docs/PERF_CONTROL_SURFACE_AND_LOG.md) — confirm your change is **in charter** (control surface table). Do not edit rows out of charter without human review.
2. **Branch** — work on the **current** integration branch; this track **does not use PRs** — **commit forward** on that branch when gates pass.
3. **Budget** — assume **≥ 30 minutes** for A–D in one go (Tilt build, fmt/clippy, full tests, benches). Do not schedule shorter autonomous cadence unless the charter explicitly allows skipping a phase.
4. **Checklist** — from repo root: `python auto-research/scripts/perf_iteration.py` (prints phases). Optional: `--verify-root` before cron; `--run-local-gates` for fmt + clippy + workspace tests (still not a full Tilt substitute).
5. **Measure** — refresh or compare Criterion baselines per [`docs/llmwiki/topics/bench-harness-phase-6.md`](../../docs/llmwiki/topics/bench-harness-phase-6.md) (`just bench-baseline-ms02*`, `just bench-against-ms02*`).
6. **Decide** — if metrics improve **and** Tilt + lint + tests pass: `git commit` with Conventional Commit + short bench delta in the body.
7. **Record** — append a row to **What we tried** in the charter; add to **What we will not try again** if an approach is permanently rejected.
8. **Wiki** — append [`../log.md`](../log.md) with date, hypothesis, and baseline tag; if methodology changed, update this page or the charter.

## Gaps / drift

- If your team uses **only** `just nt` and never full `cargo test --workspace`, document that exception in the charter file, not only here.
