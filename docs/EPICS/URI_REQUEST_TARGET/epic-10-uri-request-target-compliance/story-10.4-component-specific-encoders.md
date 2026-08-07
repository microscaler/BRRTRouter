# Story 10.4 — Component-specific encoders

**GitHub issue:** _(create)_  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.1, 10.2  
**Blocks:** 10.5, 10.10

## Overview

Replace the single `urlencoding::encode` call site with explicit **path-segment**
and **query-component** encoders (even if both start as “encode all but
unreserved”). Document the intentional decode/encode asymmetry:
inbound `+` → space; outbound space → `%20` (never `+` on rebuilt request-targets).

## Delivery

- Introduce `brrtrouter::http::uri_encode` (or similar) with:
  - `encode_path_segment(&str) -> String`
  - `encode_query_component(&str) -> String` (keys and values)
- Use them from `resolve_path_template`.
- Module docs cite RFC 3986 unreserved set and forbid `+` for space on rebuild.
- Keep existing positive/negative proxy tests green; add encoder unit tests.

## Acceptance criteria

- [ ] Path and query encoding go through named APIs (no raw `urlencoding::encode` in proxy).
- [ ] Space always `%20` on rebuild; `+` in a value encodes as `%2B`.
- [ ] Docs state form-urlencoded inbound vs URI-component outbound policy.
- [ ] Matrix rows for outbound encoding marked Pass (pending passthrough 10.5).

## References

- `src/http/proxy.rs` `resolve_path_template`
- RFC 3986 §2.3 (unreserved), §2.4
