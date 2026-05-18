# Goose benchmark baselines

This folder holds pinned Goose JSON reports that we diff against for regression detection (PRD [`PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](../../docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md) Phase 6). Each file is the exact `--report-file foo.json` output Goose emitted for a named scenario, committed so PRs can be compared against a known-good baseline.

## Files

| File | Scenario | Notes |
|---|---|---|
| [`2000u-600s.json`](./2000u-600s.json) | `api_load_test` example, `--users 2000 --increase-rate 200 --run-time 600s --no-reset-metrics` against `pet_store` on 8081 | Post Phase 0.1 + 2.2 + 5.1 baseline (2026-04-18): 33.8 M requests, **55,001 req/s**, 0 real 5xx, avg 35.40 ms, p99 130 ms. See [`docs/PERFORMANCE.md`](../../docs/PERFORMANCE.md). |
| [`2000u-600s-arcswap.json`](./2000u-600s-arcswap.json) | Same scenario but on 8091 (avoid local Tilt conflict on 8081) | Post Phase 1 baseline (2026-04-18): 37.25 M requests, **60,575 req/s (+10.1 %)**, 0 real 5xx, avg 32.09 ms (−9.4 %), p99 110 ms (−15.4 %). |
| [`2000u-600s-phase-0-3-2-1.json`](./2000u-600s-phase-0-3-2-1.json) | Same on 8091 | Post Phase 0.3 + 2.1 baseline (2026-04-18): 40.82 M requests, **66,484 req/s (+9.7 % vs Phase 1; +21 % vs Phase 5.1 pre-ArcSwap; +232 % vs Dec 2025)**, 0 real 5xx, avg **29.21 ms**, p50 **26 ms**, p95 **64 ms**, p99 **98 ms**, max 794 ms. Adds bounded metrics `DashMap` + header-name intern. **Current headline.** |

## How to regenerate

```bash
# Start pet_store on 8081 (local-dev default)
BRRTR_LOCAL=1 RUST_LOG=brrtrouter=warn,pet_store=warn \
  cargo run --release -p pet_store

# In a second terminal, drive the load-test binary directly (do not use
# scripts/run_goose_tests.py for baselines — that script runs 3 rounds and
# averages, which distorts the JSON shape expected here):
cargo run --release --example api_load_test -- \
  --host http://127.0.0.1:8081 \
  --users 2000 --increase-rate 200 --run-time 600s \
  --no-reset-metrics \
  --report-file benches/baselines/2000u-600s.json
```

## Stability rules

- Do not hand-edit these files. They are artefacts from a real run.
- When the scenario meaningfully changes (e.g. adding an L0 floor or a 404-storm scenario per Phase 6), rename with a new descriptor rather than overwriting.
- Commit the accompanying `report.html` + `report.md` only when the file is small enough that diff-review is practical; otherwise keep only the JSON.
