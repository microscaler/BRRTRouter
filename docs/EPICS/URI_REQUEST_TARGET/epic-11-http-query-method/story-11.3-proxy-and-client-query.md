# Story 11.3 — Proxy & client QUERY

**GitHub issue:** _(create)_  
**Epic:** [Epic 11 — HTTP QUERY](README.md)  
**Blocked by:** 11.1, Epic 10.7 (error taxonomy)

## Overview

BFF proxy and outbound HTTP client forward QUERY with body to downstream services.

## Delivery

- `proxy_untyped` / Method mapping: QUERY + body forwarded; hop-by-hop headers unchanged.
- may_minihttp client: prove `Method::from_bytes(b"QUERY")` works; add test.
- Timeouts/retries: treat QUERY as idempotent-safe for retry policy docs (align RFC 10008).

## Acceptance criteria

- [ ] Integration test: BFF QUERY → mock downstream QUERY with same body.
- [ ] Invalid request-target still uses Epic 10 composition errors (not 502).
- [ ] Client unit/integration coverage for QUERY.

## References

- `src/http/proxy.rs`, `src/http/fetch.rs`
- RFC 10008 §2 (safe/idempotent)
