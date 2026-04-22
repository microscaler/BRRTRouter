# Auto-research — scripts

Python-only helpers for the perf iteration loop (avoid ad-hoc shell wrappers for the same steps).

## `perf_iteration.py`

Run from the **BRRTRouter repository root** (directory containing the workspace `Cargo.toml`).

| Mode | Command | Purpose |
|------|---------|---------|
| Checklist (default) | `python auto-research/scripts/perf_iteration.py` | Print phases A–D and suggested `just` / `cargo` commands (≥ 30 min budget reminder). |
| Verify repo root | `python auto-research/scripts/perf_iteration.py --verify-root` | Exit `0` only if `Cargo.toml` looks like `brrtrouter`; use in cron before Tilt. |
| Local gates (optional) | `python auto-research/scripts/perf_iteration.py --run-local-gates` | Run `cargo fmt --check`, workspace `clippy`, workspace `cargo test` (long-running; not a substitute for full Tilt). |

**Cron example (conceptual):** after `cd` to repo root, `python auto-research/scripts/perf_iteration.py --verify-root && …` then your Tilt / `just` pipeline; append experiment rows to [`../docs/PERF_CONTROL_SURFACE_AND_LOG.md`](../docs/PERF_CONTROL_SURFACE_AND_LOG.md) (this path is `auto-research/docs/…` from the repo root).

Charter: [`../docs/PERF_CONTROL_SURFACE_AND_LOG.md`](../docs/PERF_CONTROL_SURFACE_AND_LOG.md).
