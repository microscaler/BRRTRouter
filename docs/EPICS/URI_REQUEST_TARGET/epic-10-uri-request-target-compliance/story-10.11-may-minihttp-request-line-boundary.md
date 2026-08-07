# Story 10.11 — may_minihttp request-line boundary

**GitHub issue:** [#385](https://github.com/microscaler/BRRTRouter/issues/385)  
**Epic:** [Epic 10](README.md)  
**Blocked by:** 10.1  
**Blocks:** 10.3, 10.5  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Compliance depends on knowing the **exact** path/query string may_minihttp
passes into `parse_request`. Fragment stripping, absolute-form vs origin-form,
and normalization must be documented and tested at the boundary — otherwise
BRRTRouter unit tests can pass while production bytes differ.

## Delivery

- Document request-line → `raw_path` contract in may_minihttp and BRRTRouter
  (`docs/` + code comments at `parse_request`).
- Integration or contract tests: origin-form with query; assert `parse_query_params`
  input matches expected octets.
- Note absolute-URI form / OPTIONS `*` if supported.
- If may_minihttp must change to expose raw query bytes for 10.5, open a linked
  issue on that repo and track it here.

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Origin-form `/p?q=1` | `raw_path` / parse input matches expected octets |
| P2 | Encoded space in query | reaches `parse_query_params` correctly |
| P3 | Long-but-under-limit target | accepted per contract |
| P4 | Absolute-form if supported | documented + tested |
| P5 | Normal multi-segment path | segments intact |
| P6 | Query with `+` and `%20` | octets preserved into app |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Raw space in request-target | rejected at front or app; no panic |
| N2 | CTL characters in target | rejected |
| N3 | Oversize target | 414 / close per policy |
| N4 | Null byte in target | rejected |
| N5 | Fragment `#` in request-target | rejected or stripped per HTTP; contract states which |
| N6 | Malformed absolute-form | rejected |
| N7 | Ambiguous `//` authority | documented reject/safe |
| N8 | Front accepts / app rejects | stable app error (10.7); no panic |

### Acceptance criteria (tests)

- [x] Boundary doc lists which layer owns N1–N7.
- [x] At least one integration/contract test may_minihttp → `parse_query_params`.

## Acceptance criteria

- [x] Written contract: what characters can appear in `raw_path` (query? fragment?).
- [x] At least one integration test spanning may_minihttp → `parse_query_params`.
- [x] Gaps that require may_minihttp changes are filed and linked.
- [x] Matrix row for request-line boundary marked Pass.
- [x] Unit tests section complete (positive + negative).

## Shipped (2026-08-07)

- [`request-line-boundary.md`](../request-line-boundary.md)
- `src/server/request_target.rs` — absolute-form → origin path+query; wired in `parse_request`
- `tests/request_line_boundary_tests.rs` — httparse contract (P*/N*)
- may_minihttp follow-up: [#390](https://github.com/microscaler/BRRTRouter/issues/390) (Issues disabled on may_minihttp fork)

## References

- may_minihttp server request parsing
- `src/server/request.rs` `parse_request`
- RFC 9110 §7.1 request target forms
