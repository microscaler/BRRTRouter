# Goose benchmark baselines

This folder holds pinned Goose JSON reports that we diff against for regression detection (PRD [`PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](../../docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md) Phase 6). Each file is the exact `--report-file foo.json` output Goose emitted for a named scenario, committed so PRs can be compared against a known-good baseline.

## Files

| File | Scenario | Notes |
|---|---|---|
| [`2000u-600s.json`](./2000u-600s.json) | `api_load_test` example, `--users 2000 --increase-rate 200 --run-time 600s --no-reset-metrics` against `pet_store` on 8081 | Post Phase 0.1 + 2.2 + 5.1 baseline (2026-04-18): 33.8 M requests, 55,001 req/s sustained, 0 real 5xx, p99 130 ms. See [`docs/PERFORMANCE.md`](../../docs/PERFORMANCE.md). |

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
