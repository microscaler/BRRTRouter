# Story 11.3 — Proxy & client QUERY

**GitHub issue:** [#388](https://github.com/microscaler/BRRTRouter/issues/388)  
**Epic:** [Epic 11 — HTTP QUERY](README.md)  
**Blocked by:** 11.1, Epic 10.7 (error taxonomy)  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

BFF proxy and outbound HTTP client forward QUERY with body to downstream services.

## Delivery

- `proxy_untyped` / Method mapping: QUERY + body forwarded; hop-by-hop headers unchanged.
- may_minihttp client: prove `Method::from_bytes(b"QUERY")` works; add test.
- Timeouts/retries: treat QUERY as idempotent-safe for retry policy docs (align RFC 10008).

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | BFF QUERY → mock downstream QUERY | same body bytes |
| P2 | Method `QUERY` on client | `Method::from_bytes(b"QUERY")` OK |
| P3 | Hop-by-hop headers stripped | Connection etc. not forwarded incorrectly |
| P4 | Content-Type preserved | downstream sees same |
| P5 | Valid request-target + QUERY | 2xx from mock |
| P6 | Retry policy treats QUERY as safe/idempotent | docs + unit on policy classifier |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Invalid request-target on QUERY proxy | Epic 10 composition error (**not** 502) |
| N2 | Downstream connect failure | 502/504 per 10.7; not composition |
| N3 | Empty body when required | 400; no panic |
| N4 | Oversized body | limit error; no panic |
| N5 | Method mapping drops body | test fails if body empty downstream |
| N6 | Legacy http 0.2 Method unsupported | fail closed or bridge; documented |
| N7 | Panic on forward | forbidden |
| N8 | Wrong method sent (GET/POST) | test detects mis-map |

### Acceptance criteria (tests)

- [x] P1 and N1 mandatory.
- [x] Client unit coverage for QUERY method construction.

## Acceptance criteria

- [x] Integration test: BFF QUERY → mock downstream QUERY with same body.
- [x] Invalid request-target still uses Epic 10 composition errors (not 502).
- [x] Client unit/integration coverage for QUERY.
- [x] Unit tests section complete (positive + negative).

## Delivery notes

- `fetch_query` + `method_allows_automatic_retry` (see [query-retry-policy.md](query-retry-policy.md))
- Proxy already mapped methods via `Method::from_bytes`; tests prove QUERY + body + hop-by-hop

## References

- `src/http/proxy.rs`, `src/http/fetch.rs`
- RFC 10008 §2 (safe/idempotent)
