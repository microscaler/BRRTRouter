# Bench harness (Phase 6)

**PRD:** [`PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](../../PRD_HOT_PATH_V2_STABILITY_AND_PERF.md) — Phase **R.1** showed **~10% uniform** latency shift **including `/health`**, which is a strong sign of **thermal / scheduling drift**, not router regression. Phase 6 tightens measurement so sub-~15% deltas become meaningful.

## Goals

1. **Macro stress (2000 users × 600 s)** — comparable A/B in one session: same binary, same host, back-to-back, cool-down between runs if needed.
2. **Micro (Criterion)** — router internals and validation hot spots without 10-minute thermal drift; use `--save-baseline` / `--baseline` for regression gates.

## Criterion (in-repo)

| Bench | Purpose |
|-------|---------|
| [`benches/throughput.rs`](../../../benches/throughput.rs) | Radix / `Router` routing throughput (Verb Zoo spec) |
| [`benches/jwt_cache_performance.rs`](../../../benches/jwt_cache_performance.rs) | JWT / security provider path |
| [`benches/schema_validation_hot_path.rs`](../../../benches/schema_validation_hot_path.rs) | `ValidatorCache` hit + `jsonschema::Validator::iter_errors` (Phase 4 hot path) |

Run:

```bash
cargo bench -p brrtrouter --bench throughput
cargo bench -p brrtrouter --bench schema_validation_hot_path
```

**Compare vs a saved baseline:** Criterion stores data under `target/criterion/`. You must **`--save-baseline <tag>`** before **`--baseline <tag>`** on this machine (new clone / `cargo clean` → save again).

### MS02 — `just` workflows (SHA + date in the tag)

The **`justfile`** expands **`ms02-<git-short-sha>-<YYYYMMDD>`** so you never type SHA/date by hand. It writes the tag to **`benches/baselines/.ms02-criterion-baseline`** (gitignored) for compares.

| Recipe | What it does |
|--------|----------------|
| `just bench-baseline-ms02` | Save baseline for `schema_validation_hot_path` only |
| `just bench-baseline-ms02-all` | Save the **same** tag for schema + throughput + JWT benches |
| `just bench-against-ms02` | Compare `schema_validation_hot_path` to the **last** saved tag |
| `just bench-against-ms02-all` | Compare all three benches to the **last** saved tag |

**Raw cargo** (equivalent tag — replace `TAG` with printed output from a save run, e.g. `ms02-a1b2c3d-20260418`):

```bash
cargo bench -p brrtrouter --bench schema_validation_hot_path -- --save-baseline TAG
cargo bench -p brrtrouter --bench schema_validation_hot_path -- --baseline TAG
```

Record **git SHA**, **date**, **`rustc -Vv`**, **`uname -a`** in [`log.md`](../log.md) when you establish a new host baseline.

**Note:** The tag encodes **commit + calendar day** (local timezone). Re-save after **`cargo clean`** (Criterion data lives under `target/`).

## Macro stress checklist

- **Release** + **jemalloc** where the team standardizes (see [`PERFORMANCE.md`](../../PERFORMANCE.md)).
- **Logging:** `RUST_LOG=brrtrouter=warn` (or stricter) so I/O does not dominate.
- **Port:** avoid collisions with local Tilt (historically **8091** vs **8081**).
- **Fixed hardware state:** note CPU governor, laptop plugged in, cooldown between A/B; record ambient if troubleshooting thermals.
- **Artefacts:** keep JSON + markdown summary under `benches/baselines/` with **git commit hash** and **exact CLI**.

## Goose / load driver

When migrating to **Goose v2** JSON reports, store outputs next to baselines and reference them from [`PERFORMANCE.md`](../../PERFORMANCE.md) and [`log.md`](../log.md).

## Open

- Automated “same-session A/B” script (optional; keep in `justfile` or docs only until stable).

## Autonomous perf loop (charter + log)

For **≥ 30 minute** iterations (Tilt + lint + full tests + benches), the **control surface**, **experiment log**, and **commit-forward / no-PR** policy live under **`auto-research/docs/`**:

- [`auto-research/docs/PERF_CONTROL_SURFACE_AND_LOG.md`](../../../auto-research/docs/PERF_CONTROL_SURFACE_AND_LOG.md)
- How to conduct: [`llmwiki/topics/auto-research-perf-loop.md`](../../../llmwiki/topics/auto-research-perf-loop.md) (repo-root wiki)
