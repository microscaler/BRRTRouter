# Story 12.4 — Pre-handler query/header validation

**GitHub issue:** [#395](https://github.com/microscaler/BRRTRouter/issues/395)  
**Epic:** [Epic 12](README.md)  
**Wave:** 1  
**Effort:** M  
**Blocked by:** benefits from 12.3 (resolved schemas); not hard-blocked  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Enforce OpenAPI **required** query/header (and path) parameters in the service
pipeline **before** the handler runs — not only inside generated `TryFrom`.

## Delivery

- After route match: validate required params + basic type/format policy (document which formats).
- 400 with stable error JSON listing missing/invalid fields.
- Align with existing body schema validation path.
- Untyped/proxy routes: document behaviour (validate when `RouteMeta.parameters` present).

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | All required query present | reaches handler |
| P2 | Optional query omitted | OK |
| P3 | Required header present | OK |
| P4 | Path params from radix match | OK |
| P5 | Valid integer query vs schema | OK when formats enabled |
| P6 | Regression: secured route still auth-first | order documented |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Missing required query | **400**; handler not called |
| N2 | Missing required header | **400** |
| N3 | Wrong type for query (string vs int) | **400** |
| N4 | Empty string for required | **400** or documented |
| N5 | Unknown query when additionalProperties false (if enforced) | documented |
| N6 | Hostile oversized query value | limit / 400; no panic |
| N7 | Panic on validation | forbidden |
| N8 | Silent coerce corrupting semantics | forbidden |

### Acceptance criteria (tests)

- [ ] N1/N2 mandatory; P1 mandatory.

## Acceptance criteria

- [ ] Required query/header missing → 400 before handler.
- [ ] Error body lists fields.
- [ ] Unit tests section complete.

## References

- `src/server/service.rs`, `llmwiki/topics/schema-validation-pipeline.md`
