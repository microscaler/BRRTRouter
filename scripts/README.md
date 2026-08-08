# Scripts

Operational helpers for load testing, curls, and local builds. Run from the
**repo root** unless noted.

| Path | Purpose |
|------|---------|
| `run_goose_tests.py` | Goose JSF / baseline runs (`just goose-jsf`) |
| `generate_benchmark_report.py` | Benchmark HTML/report generation |
| `compare_metrics.py` | Diff Goose / metrics JSON |
| `curls.sh` | Smoke curls against Pet Store |
| `test_stability.py` / `test_stability_detailed.py` | Repeat `cargo test` stability sweeps |
| `host-aware-build.sh` / `ensure-jemalloc.sh` / `build-test.sh` | Build helpers |
| `post.lua` | wrk-style POST payload (legacy) |
| [`artifacts/`](artifacts/) | Generated JSON reports (stability / Goose outputs) |
| [`archive/`](archive/) | Obsolete one-shot migration scripts (do not run) |

Cargo integration / unit tests live under [`../tests/`](../tests/), not here.
