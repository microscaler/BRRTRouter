# Story 13.6 — Handler / request deadlines → 504

**GitHub issue:** [#406](https://github.com/microscaler/BRRTRouter/issues/406)  
**Epic:** [Epic 13](README.md)  
**Wave:** 3  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Complete the Kubernetes ops story next to graceful shutdown: configurable
**per-request / handler deadlines** so a stuck handler yields **504** (and metrics)
instead of holding a worker indefinitely.

## Delivery

- Global deadline (env/`AppConfig`) + optional per-route vendor override.
- Dispatcher or service enforces deadline around handler wait (`recv` timeout or equivalent).
- On timeout: **504**, stable error/problem JSON, metric increment, no panic.
- Document interaction with proxy upstream timeouts (existing 504 paths).
- Explicit non-goal: cancelling arbitrary user threads mid-CPU (best-effort stop wait).

## Functional requirements

| ID | Requirement |
|----|-------------|
| FR-1 | When deadline enabled, handler exceeding limit → client **504**. |
| FR-2 | Fast handlers under deadline unaffected. |
| FR-3 | Timeout increments a metrics counter. |
| FR-4 | Per-route override can shorten/lengthen within global ceiling policy (document). |
| FR-5 | Disabled deadline → legacy wait behavior (documented). |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| NFR-1 | No panic if handler finishes just after timeout race. |
| NFR-2 | Timeout path does not leak reply channels unbounded (document cleanup). |
| NFR-3 | Hot path cost when disabled is near-zero. |
| NFR-4 | Error body does not include internal stack traces by default. |

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Handler returns before deadline | 2xx/expected |
| P2 | Slow handler past deadline | **504** |
| P3 | Metric increments on timeout | observed |
| P4 | Disabled deadline | slow handler still returns (test with short sleep) |
| P5 | Proxy timeout still 504 where applicable | regression note |
| P6 | Error JSON/problem shape stable | `status`/reason |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Handler panics under deadline | still recovered (existing panic path); no double panic |
| N2 | Zero/negative deadline config | reject or treat as disabled; no panic |
| N3 | Silent hang forever when deadline enabled | forbidden |
| N4 | Timeout returns 200 | forbidden |
| N5 | Credential leak in timeout body | forbidden |
| N6 | Panic in timeout writer | forbidden |

### Acceptance criteria (tests)

- [x] P1/P2 and N3/N4 mandatory.

## Acceptance criteria

- [x] Config documented; default safe for existing apps (off or high).
- [x] FR/NFR + unit tests complete.

## References

- `src/dispatcher/core.rs`, `src/server/http_server.rs`, `src/server/service.rs`
- `docs/PERFORMANCE.md` next-bottleneck (dispatch)
