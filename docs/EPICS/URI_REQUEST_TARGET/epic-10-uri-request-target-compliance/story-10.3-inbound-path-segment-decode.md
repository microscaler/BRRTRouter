# Story 10.3 — Inbound path segment decode

**GitHub issue:** _(create)_  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.1, 10.11  
**Blocks:** 10.4, 10.9, 10.10

## Overview

Path template matching must decode percent-encoded segments per RFC 3986 before
handlers see values, and must not treat encoded `/` (`%2F`) as a segment
separator. Symmetric to query work in 10.2.

## Delivery

- Trace router path capture (`src/router/`) from matched template → `path_params`.
- Ensure `%20`, UTF-8 multi-byte, and `%2F` in a single segment behave correctly.
- Align with OpenAPI path parameter semantics (single segment unless style says otherwise).
- Add goldens + unit tests for encoded path params (e.g. `/regions/Western%20Cape`).

## Acceptance criteria

- [ ] Encoded spaces/accents in path params decode to Unicode strings in `HandlerRequest`.
- [ ] `%2F` does not create an extra path segment.
- [ ] Illegal encodings fail closed (no panic); status consistent with 10.7.
- [ ] Matrix rows for inbound path marked Pass.

## References

- `src/router/` path matching
- RFC 3986 §3.3 (`segment`, `pchar`)
- OpenAPI 3 path parameters
