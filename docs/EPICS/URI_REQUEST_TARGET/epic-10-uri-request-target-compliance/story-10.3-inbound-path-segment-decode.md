# Story 10.3 — Inbound path segment decode

**GitHub issue:** [#377](https://github.com/microscaler/BRRTRouter/issues/377)  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.1, 10.11  
**Blocks:** 10.4, 10.9, 10.10  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Path template matching must decode percent-encoded segments per RFC 3986 before
handlers see values, and must not treat encoded `/` (`%2F`) as a segment
separator. Symmetric to query work in 10.2.

## Delivery

- Trace router path capture (`src/router/`) from matched template → `path_params`.
- Ensure `%20`, UTF-8 multi-byte, and `%2F` in a single segment behave correctly.
- Align with OpenAPI path parameter semantics (single segment unless style says otherwise).
- Add goldens + unit tests for encoded path params (e.g. `/regions/Western%20Cape`).

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | `/regions/Western%20Cape` | param = `Western Cape` |
| P2 | Accented segment (`C%C3%B4te`) | UTF-8 decode |
| P3 | Unreserved segment | identity |
| P4 | `%2F` inside one segment | single param containing `/`; no extra segment |
| P5 | Multi-byte CJK segment | correct Unicode |
| P6 | Trailing-slash policy fixture | documented match behaviour |
| P7 | Multiple path params encoded | each decoded independently |
| P8 | Empty optional segment (if routed) | documented |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | `+` in path (`Western+Cape`) | `+` **not** treated as space |
| N2 | Truncated `%` in segment | no panic; fail closed |
| N3 | `%GG` in segment | no panic; documented |
| N4 | Encoded `..` (`%2E%2E`) | no unintended route / traversal |
| N5 | Double-encoded traps | decode-once policy locked |
| N6 | `%00` in segment | reject or safe; no panic |
| N7 | Overlong UTF-8 pct sequences | reject/safe |
| N8 | Control octets pct-encoded | no panic |

### Acceptance criteria (tests)

- [x] All P*/N* as named unit tests; N1 and P4 mandatory.
- [x] Illegal encodings share status policy with 10.7.

## Acceptance criteria

- [x] Encoded spaces/accents in path params decode to Unicode strings in `HandlerRequest`.
- [x] `%2F` does not create an extra path segment.
- [x] Illegal encodings fail closed (no panic); status consistent with 10.7.
- [x] Matrix rows for inbound path marked Pass.
- [x] Unit tests section complete (positive + negative).

## Shipped (2026-08-07)

- `src/router/path_segment.rs` — `decode_path_segment` (`+` ≠ space; decode-once; lossy UTF-8)
- Radix capture applies decode on param push
- Trailing `/` ignored (empty segments skipped) — documented in `path_decode_positive_p6_*`

## References

- `src/router/` path matching
- RFC 3986 §3.3 (`segment`, `pchar`)
- OpenAPI 3 path parameters
