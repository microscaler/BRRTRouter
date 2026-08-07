# Story 10.2 — Inbound query parse edge cases

**GitHub issue:** [#376](https://github.com/microscaler/BRRTRouter/issues/376)  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.1  
**Blocks:** 10.4, 10.5, 10.9, 10.10  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

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

## Unit tests (required)

Module: `src/server/request.rs` (`parse_query_params` and callers).

### Positive

| ID | Input path | Assert |
|----|------------|--------|
| P1 | `/p?x=1&y=2` | `x=1`, `y=2` |
| P2 | `/p?q=South%20Africa` | `q` = `South Africa` |
| P3 | `/p?q=South+Africa` | `q` = `South Africa` |
| P4 | `/p?q=C%C3%B4te` | `q` = `Côte` |
| P5 | `/p?a=1&a=2` | two `a` entries in order |
| P6 | `/p?k=` | `k` → `""` |
| P7 | `/p?q=%2B` | `q` = `+` |
| P8 | `/p` (no query) | empty ParamVec |
| P9 | `/p?` | empty or no pairs (document) |
| P10 | `/p?name=%E6%9D%B1%E4%BA%AC` | `東京` |

### Negative

| ID | Input | Assert |
|----|-------|--------|
| N1 | `/p?q=%` | no panic; documented reject/skip/replacement |
| N2 | `/p?q=%2` | no panic; documented behaviour |
| N3 | `/p?q=%GG` | no panic; documented behaviour |
| N4 | `/p?q=%FF` (invalid UTF-8 alone) | no panic; documented lossy/reject |
| N5 | path with embedded NUL if reachable | no panic |
| N6 | extremely long query (under 414 limit) | parses or fails gracefully |
| N7 | `/p?=` / odd empty-key forms | documented; no panic |
| N8 | only `#frag` after path (if presented) | documented; no panic |

### Acceptance criteria (tests)

- [x] All P*/N* as named unit tests (`parse_query_params_positive_*` / `_negative_*`).
- [x] Illegal-% policy documented in module docs and locked by N1–N3.
- [x] Duplicate-key order locked by P5.

## Acceptance criteria

- [x] Goldens for `+` / `%20` spaces both decode to the same string value.
- [x] Truncated/`%GG` escapes: behaviour documented + tested (no panic).
- [x] `a=1&a=2` → two entries; order preserved.
- [x] Empty value `k=` and valueless `k` (if accepted by form_urlencoded) covered.
- [x] Matrix rows for inbound query marked Pass.
- [x] Unit tests section complete (positive + negative).

## Shipped (2026-08-07)

**Illegal-% policy:** leave-as-is for truncated/illegal hex; lossy `U+FFFD` for invalid
UTF-8 after decode; never panic; no HTTP 400 from this layer.

**Tests:** `src/server/request.rs` — `parse_query_params_positive_*` /
`parse_query_params_negative_*` (20 cases incl. valueless key + space equivalence).

## References

- `src/server/request.rs` (`parse_query_params`)
- WHATWG URL — `application/x-www-form-urlencoded` parsing
