# Story 13.2 — Rate limiting middleware

**GitHub issue:** [#402](https://github.com/microscaler/BRRTRouter/issues/402)  
**Epic:** [Epic 13](README.md)  
**Wave:** 1  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Ship a real **rate-limit middleware** (token bucket or sliding window) so public
APIs can shed load with **429** and metrics — replacing aspirational docs claims.

## Delivery

- `src/middleware/rate_limit.rs` (or equivalent) wired into the middleware chain.
- Config: global default + optional per-route override (OpenAPI vendor ext and/or `AppConfig`).
- Keying: client IP and/or auth subject (document precedence).
- On exceed: **429**, `Retry-After` when applicable, stable error JSON (or Problem Details if 13.3 landed — dual-support OK).
- Prometheus/OTEL counter for sheds.
- Update docs only after implementation (completes 13.1 strike).

## Functional requirements

| ID | Requirement |
|----|-------------|
| FR-1 | When enabled, requests over the limit receive **429** before handler dispatch. |
| FR-2 | Under-limit requests proceed unchanged (status/body from route). |
| FR-3 | Config can set requests-per-window (or equivalent) globally. |
| FR-4 | Optional per-route limit tighter than global is honored. |
| FR-5 | Shed events increment a metrics counter. |
| FR-6 | Disabled / unset config → middleware is a no-op (default safe). |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| NFR-1 | No panic on missing peer addr / missing auth subject (fallback key documented). |
| NFR-2 | Hot path avoids global `Mutex` for every request (shard or lock-free structure). |
| NFR-3 | Does not reintroduce `RwLock` on `SharedRouter` match path. |
| NFR-4 | Memory of key map is bounded or TTL-evicted (DoS via unique keys mitigated). |
| NFR-5 | Error body shape stable across releases for the chosen format. |

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Under limit | proceeds (not 429) |
| P2 | Exactly at limit then one more | last → **429** |
| P3 | Window elapses | allows again |
| P4 | Per-route tighter limit | 429 at route threshold |
| P5 | Metrics counter increments on shed | observed |
| P6 | Disabled config | never 429 from limiter |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Burst over limit | **429**; no handler side effects |
| N2 | Hostile flood of unique keys | no OOM / panic; eviction or reject |
| N3 | Missing peer address | fallback key; no panic |
| N4 | Panic inside limiter | forbidden |
| N5 | Silent drop without 429 | forbidden |
| N6 | Limit bypass via HEAD/OPTIONS if configured to count | documented; consistent |
| N7 | Credential/PII in rate-limit logs | forbidden |

### Acceptance criteria (tests)

- [x] P1/P2 and N1/N4 mandatory.

## Acceptance criteria

- [x] Middleware ships and is configurable.
- [x] Docs updated to claim rate limiting only after green tests.
- [x] FR/NFR + unit tests complete.

## References

- `src/middleware/mod.rs`, `src/middleware/metrics.rs`
- `docs/RequestLifecycle.md` (aspirational table)
