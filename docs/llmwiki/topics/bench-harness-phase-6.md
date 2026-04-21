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

Run:

```bash
cargo bench -p brrtrouter --bench throughput
```

Document saved baselines beside `benches/baselines/` JSON from Goose stress tests.

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
