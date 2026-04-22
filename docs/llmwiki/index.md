# BRRTRouter llmwiki — index

Read [`SCHEMA.md`](./SCHEMA.md) first. Tail [`log.md`](./log.md) for recent work.

## Topics

| Topic | Summary |
|-------|---------|
| [`topics/schema-validation-pipeline.md`](./topics/schema-validation-pipeline.md) | Runtime JSON Schema path: parse → 415/400 gates → `ValidatorCache` → dispatch — **Phase 4** (`PRD_HOT_PATH_V2`) |
| [`topics/bench-harness-phase-6.md`](./topics/bench-harness-phase-6.md) | Reproducible stress + Criterion baselines — **Phase 6** (includes `schema_validation_hot_path` bench) |
| [`../../llmwiki/topics/auto-research-perf-loop.md`](../../llmwiki/topics/auto-research-perf-loop.md) | How to run **auto-research** perf iterations (cron); charter in `auto-research/docs/` |
| [`../../auto-research/docs/PERF_CONTROL_SURFACE_AND_LOG.md`](../../auto-research/docs/PERF_CONTROL_SURFACE_AND_LOG.md) | Control surface + experiment log (canonical) |

## Related

- Performance PRD: [`../PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](../PRD_HOT_PATH_V2_STABILITY_AND_PERF.md)
- [`../PERFORMANCE.md`](../PERFORMANCE.md) — headline numbers and methodology
