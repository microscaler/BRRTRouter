# Story 11.4 — Accept-Query + POST fallback docs

**GitHub issue:** _(create)_  
**Epic:** [Epic 11 — HTTP QUERY](README.md)  
**Blocked by:** 11.2

## Overview

Document and optionally implement `Accept-Query` advertisement, cache-key notes,
and a **POST fallback** pattern for browsers/edges that do not yet honour QUERY.

## Delivery

- Support or document response/request `Accept-Query` (RFC 10008 §3).
- Consumer guide: when to use GET vs QUERY vs POST; uppercase `QUERY` in fetch;
  CORS preflight; no HTML form support; cache limitations.
- Optional: convention for `POST` + `Query-Method` / method-override fallback
  (document only unless product asks for implementation).

## Acceptance criteria

- [ ] Docs page linked from Epic 11 README and audit §5.
- [ ] Example fetch + preflight CORS snippet.
- [ ] Explicit “Epic 10 still required for GET query strings.”

## References

- RFC 10008 §2.7 (caching), §3 (`Accept-Query`), §4
- `docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md` §5
