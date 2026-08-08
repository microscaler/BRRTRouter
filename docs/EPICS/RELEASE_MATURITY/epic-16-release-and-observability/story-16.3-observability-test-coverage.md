# Story 16.3 — Observability test coverage (fake OTEL)

**GitHub issue:** [#432](https://github.com/microscaler/BRRTRouter/issues/432)  
**Epic:** [Epic 16](README.md)  
**Wave:** 2  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Close ROADMAP gap: broaden fake OTEL collector usage across remaining critical
tests; address high-value gaps from telemetry WIP notes that block release confidence.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Inventory of tests still on real/no OTEL vs fake collector. |
| FR-2 | Migrate N highest-value tests to fake collector. |
| FR-3 | Document how product crates should test telemetry. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | Tests hermetic/offline. |
| NFR-2 | No flaky wall-clock asserts. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Inventory doc/table | present |
| P2 | ≥N tests use fake collector | count |
| P3 | Example assertion on span/metric | yes |
| P4 | Guide snippet | yes |
| P5 | CI still green | yes |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Require live collector in unit tests | forbidden |
| N2 | Flaky sleep-only assert | forbidden |
| N3 | Panic without collector | forbidden |
| N4 | Secret attributes leaked in fixtures | forbidden |
| N5 | Silent skip telemetry asserts | forbidden |

### Acceptance criteria (tests)
- [ ] P1/P2 and N1 mandatory.

## Acceptance criteria
- [ ] Coverage improved + documented; FR/NFR complete.

