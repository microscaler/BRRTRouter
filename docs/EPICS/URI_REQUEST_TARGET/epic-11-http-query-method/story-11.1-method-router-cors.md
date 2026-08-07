# Story 11.1 — Method + router + CORS

**GitHub issue:** [#386](https://github.com/microscaler/BRRTRouter/issues/386)  
**Epic:** [Epic 11 — HTTP QUERY](README.md)  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Accept `QUERY` as a first-class method in routing and CORS preflight responses.

## Delivery

- Ensure method parsing accepts `QUERY` (http 1.x / legacy bridge).
- Router matches OpenAPI/routes registered for QUERY.
- CORS: include `QUERY` in `Access-Control-Allow-Methods` when enabled; preflight succeeds.
- Reject unknown methods with 405 as today.

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | `QUERY` to registered route | handler invoked |
| P2 | Same-origin QUERY with body | reaches handler |
| P3 | CORS on: preflight lists `QUERY` | `Access-Control-Allow-Methods` contains QUERY |
| P4 | CORS on: actual QUERY after preflight | not blocked by method check |
| P5 | Uppercase `QUERY` method bytes | accepted |
| P6 | Existing GET/POST routes | unaffected regression |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | QUERY to unregistered path | 404/405 per existing router policy |
| N2 | Unknown method | 405 |
| N3 | Lowercase `query` method (if HTTP forbids) | reject per stack rules; documented |
| N4 | CORS off: no spurious Allow-Methods QUERY | not advertised |
| N5 | Preflight without QUERY in route CORS config | no false allow |
| N6 | Method parse garbage bytes | no panic; 400/405 |
| N7 | QUERY with illegal request-target | Epic 10 composition path; no panic |
| N8 | Panic on CORS header build | forbidden |

### Acceptance criteria (tests)

- [ ] P1/P3 and N2 mandatory.
- [ ] All P*/N* named unit/integration tests as appropriate; unit-level method/CORS helpers required.

## Acceptance criteria

- [ ] Same-origin QUERY reaches a registered handler.
- [ ] CORS preflight lists QUERY when CORS is on.
- [ ] Tests for allow and 405 paths.
- [ ] Unit tests section complete (positive + negative).

## References

- RFC 10008 §2, §4 (Security / CORS)
- `src/server/cors_setup.rs`, `src/router/`
