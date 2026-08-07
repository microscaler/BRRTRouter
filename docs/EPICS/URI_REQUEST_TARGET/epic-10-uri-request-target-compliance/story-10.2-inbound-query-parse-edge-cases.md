# Story 10.2 — Inbound query parse edge cases

**GitHub issue:** _(create)_  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.1  
**Blocks:** 10.4, 10.5, 10.9, 10.10

## Overview

Bring `parse_query_params` to full correctness for browser-shaped and hostile
query strings. Today it delegates to `url::form_urlencoded::parse` after the
first `?`; edge cases (invalid escapes, `+`, duplicates, empty parts) must be
explicitly specified and tested — not assumed.

## Delivery

- Audit `src/server/request.rs` `parse_query_params` against golden corpus (10.1).
- Define fail-closed behaviour for illegal percent-encoding (reject request vs
  replacement character — pick one, document, test).
- Preserve duplicate keys as multiple `ParamVec` entries (already intended).
- Clarify handling when path contains `#` (should not reach us; if it does,
  document).
- Expand unit tests beyond `/p?x=1&y=2`.

## Acceptance criteria

- [ ] Goldens for `+` / `%20` spaces both decode to the same string value.
- [ ] Truncated/`%GG` escapes: behaviour documented + tested (no panic).
- [ ] `a=1&a=2` → two entries; order preserved.
- [ ] Empty value `k=` and valueless `k` (if accepted by form_urlencoded) covered.
- [ ] Matrix rows for inbound query marked Pass.

## References

- `src/server/request.rs` (`parse_query_params`)
- WHATWG URL — `application/x-www-form-urlencoded` parsing
