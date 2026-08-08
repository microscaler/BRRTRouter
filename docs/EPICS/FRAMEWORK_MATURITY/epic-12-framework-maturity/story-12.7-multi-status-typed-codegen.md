# Story 12.7 — Multi-status typed / codegen

**GitHub issue:** [#398](https://github.com/microscaler/BRRTRouter/issues/398)  
**Epic:** [Epic 12](README.md)  
**Wave:** 3  
**Effort:** L  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Finish the typed handler HTTP status story: codegen + OpenAPI alignment for
multiple response statuses, **204**, and **HEAD**, building on runtime `HttpJson`.

## Delivery

- Follow [`docs/PRD_TYPED_HANDLER_HTTP_STATUS.md`](../../../PRD_TYPED_HANDLER_HTTP_STATUS.md) remaining items.
- Generator emits types/handlers that can return distinct statuses without panic-as-control-flow.
- 204 empty body; HEAD no body.
- Resolve `components.responses` refs (coordinate with 12.3).

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Handler returns 200 typed body | status + JSON |
| P2 | Handler returns alternate 201/404 per OpenAPI | correct status |
| P3 | 204 No Content | empty body |
| P4 | HEAD | headers OK; no body |
| P5 | `HttpJson` ok path | regression |
| P6 | Generated stub compiles for multi-status op | cargo check fixture |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Undeclared status (if policy reject) | 500 or documented |
| N2 | 204 with body attempted | rejected / stripped per policy |
| N3 | Missing response schema ref | fail codegen/load (12.3) |
| N4 | Panic used for non-200 control flow in new templates | forbidden |
| N5 | HEAD leaking body | forbidden |
| N6 | Wrong content-type for status | validated or documented |
| N7 | Generator half-writes handler | forbidden |
| N8 | Panic on empty response map | forbidden |

### Acceptance criteria (tests)

- [x] P2/P3 and N4/N5 mandatory.

## Acceptance criteria

- [x] Multi-status via `HttpJson` / `HttpNoContent`; HEAD omits body on the wire.
- [x] PRD: full L1 per-status enum codegen deferred (Wave 4 / follow-up); runtime + stub `HttpJson` shipped.
- [x] Unit tests section complete (typed + response omit_body + generator N4).

## References

- `docs/PRD_TYPED_HANDLER_HTTP_STATUS.md`
- `src/typed/`, `src/generator/`
