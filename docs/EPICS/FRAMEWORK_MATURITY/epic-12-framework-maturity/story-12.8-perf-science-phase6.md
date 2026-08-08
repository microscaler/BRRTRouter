# Story 12.8 — Perf science (Phase 6 benches + validator flamegraph)

**GitHub issue:** [#399](https://github.com/microscaler/BRRTRouter/issues/399)  
**Epic:** [Epic 12](README.md)  
**Wave:** 4  
**Effort:** M  
**Blocked by:** prefer after Wave 1 (validation changes affect hot path)  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Make performance claims measurable before further hot-path inventiveness. Fix
noisy Criterion harness (Phase 6) and produce a validator-path flamegraph
(Phase 4 open item) so the next bottleneck is evidence-based — **not** radix.

## Delivery

- Stabilize `benches/` variance (document seed/CPU pinning / sample size).
- Document current bottleneck: validation / dispatch vs route match.
- Optional: criterion groups for body-limit + param-validation paths from 12.2/12.4.
- Explicit non-goal: Phase 3 reply-slot redesign; trie rewrite.

## Unit tests (required)

Perf story: unit tests guard harness helpers; benches are CI-optional but must run locally.

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | `route_match` bench runs | completes |
| P2 | Scalability 10→500 routes | documented curve |
| P3 | Schema validation hot-path bench | runs |
| P4 | Flamegraph/README instructions | reproducible steps |
| P5 | Compare match vs validate wall time | both sub-µs; match ≪ e2e latency → no trie rewrite (doc assert) |
| P6 | Regression: no accidental RwLock reintro on match path | code guard / review |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Bench fails flaky without retry policy | document or fix |
| N2 | Claiming “routing bottleneck” without data | forbidden in docs |
| N3 | Flamegraph steps broken | forbidden |
| N4 | Panic in bench setup | forbidden |
| N5 | Silent skip of validation bench in CI docs | call out |
| N6 | Optimizing radix based on noise | forbidden without P5 evidence |

### Acceptance criteria (tests)

- [x] P1/P5 and N2 mandatory (doc + bench smoke).

## Acceptance criteria

- [x] Phase 6 harness notes in `docs/PERFORMANCE.md`.
- [x] Written “next bottleneck” recommendation.
- [x] Unit tests section complete.

## References

- `docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`
- `docs/PERFORMANCE.md`, `benches/`
