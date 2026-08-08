# Story 11.4 — Accept-Query + POST fallback docs

**GitHub issue:** [#389](https://github.com/microscaler/BRRTRouter/issues/389)  
**Epic:** [Epic 11 — HTTP QUERY](README.md)  
**Blocked by:** 11.2  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Document and optionally implement `Accept-Query` advertisement, cache-key notes,
and a **POST fallback** pattern for browsers/edges that do not yet honour QUERY.

## Delivery

- Support or document response/request `Accept-Query` (RFC 10008 §3).
- Consumer guide: when to use GET vs QUERY vs POST; uppercase `QUERY` in fetch;
  CORS preflight; no HTML form support; cache limitations.
- Optional: convention for `POST` + `Query-Method` / method-override fallback
  (document only unless product asks for implementation).

## Unit tests (required)

If `Accept-Query` (or POST fallback) is **implemented**, the tables below are code
unit tests. If **docs-only**, still add unit tests for any parser/helper shipped,
and treat doc examples as fixtures that must compile/load (P1–P3).

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Docs example OpenAPI/fetch snippet | loads / is syntactically valid fixture |
| P2 | `Accept-Query` advertise (if impl) | response header present when enabled |
| P3 | POST fallback handler (if impl) | accepted body routed as query |
| P4 | Guide states uppercase `QUERY` | fixture/checklist assertion in docs test or review gate |
| P5 | Guide links Epic 10 for GET query strings | present |
| P6 | CORS preflight snippet matches 11.1 behaviour | consistent with tests |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Lowercase `query` in fetch example | docs must not recommend; lint/fixture forbids |
| N2 | HTML form QUERY claim | docs must state unsupported |
| N3 | Accept-Query malformed value (if parser) | reject/ignore per RFC; no panic |
| N4 | POST fallback without override header (if impl) | not treated as QUERY |
| N5 | Cache-key claims overstated | docs state browser cache incomplete |
| N6 | Edge 405 without fallback guidance | docs cover POST fallback |
| N7 | Impl panics on missing Accept-Query | forbidden |
| N8 | Silent QUERY→GET downgrade | forbidden if impl |

### Acceptance criteria (tests)

- [x] At least docs-fixture tests for P1/P5 when code is docs-only.
- [x] Full P*/N* when Accept-Query or POST fallback is implemented.
  (`Accept-Query` helpers implemented; POST fallback **documented** only.)

## Acceptance criteria

- [x] Docs page linked from Epic 11 README and audit §5.
- [x] Example fetch + preflight CORS snippet.
- [x] Explicit “Epic 10 still required for GET query strings.”
- [x] Unit tests section complete (positive + negative).

## Delivery notes

- Guide: [consumer-guide-query-method.md](consumer-guide-query-method.md)
- Helpers: `format_accept_query` / `parse_accept_query`
- Tests: `tests/query_consumer_guide_tests.rs` + `src/http/accept_query.rs`

## References

- RFC 10008 §2.7 (caching), §3 (`Accept-Query`), §4
- `docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md` §5
