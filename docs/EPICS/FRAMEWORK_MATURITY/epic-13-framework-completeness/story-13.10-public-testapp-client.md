# Story 13.10 — Public TestApp / RequestBuilder

**GitHub issue:** [#410](https://github.com/microscaler/BRRTRouter/issues/410)  
**Epic:** [Epic 13](README.md)  
**Wave:** 5  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Export a **public in-process test client** (`TestApp` / `RequestBuilder`) so product
crates (Sesame, Pet Store) are not forced to copy `tests/common` TCP helpers.
Wraps existing may_minihttp / AppService test patterns behind a stable API.

## Delivery

- Public module e.g. `brrtrouter::test_support` (feature-gated `testing` optional).
- Builder: method, path, headers, JSON body, cookies.
- Assertions helpers optional; at minimum returns status + headers + body bytes/JSON.
- Docs + example in pet_store or `docs/TESTING.md`.
- Non-goal: full browser automation; Goose load driver.

## Functional requirements

| ID | Requirement |
|----|-------------|
| FR-1 | Product test can `TestApp::from_spec` / `from_service` without private `tests/common`. |
| FR-2 | `get/post` JSON helpers round-trip status + body. |
| FR-3 | Custom headers and cookies can be set on the request. |
| FR-4 | 404/400 framework errors observable in test client response. |
| FR-5 | Feature flag or clear docs so production builds need not pull test deps heavily. |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| NFR-1 | API marked for test use; no secrets logged by default. |
| NFR-2 | No panic on connectionless construction errors — return Result. |
| NFR-3 | Deterministic enough for CI (no wall-clock flakes in helpers). |
| NFR-4 | Does not require real network when in-process path is available. |

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | GET known route | 200 + body |
| P2 | POST JSON | status + echoed/parsed body |
| P3 | Header forwarded | observed by handler/test |
| P4 | Cookie set | observed |
| P5 | Pet-store or fixture smoke via TestApp | ok |
| P6 | Public export compiles from external crate path | doc/example |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Unknown path | 404; no panic |
| N2 | Invalid JSON body to JSON route | 400/problem; no panic |
| N3 | Builder with empty path | Err or 404; no panic |
| N4 | Panic inside TestApp on hostile header | forbidden |
| N5 | Leak of Authorization in Display | forbidden |
| N6 | Silent success on failed start | forbidden |

### Acceptance criteria (tests)

- [ ] P1/P2 and N1/N2 mandatory.

## Acceptance criteria

- [ ] Public API documented; used by at least one in-repo example test.
- [ ] FR/NFR + unit tests complete.

## References

- `tests/common/mod.rs`, may_minihttp `TestClient`, `AGENTS.md` testing notes
