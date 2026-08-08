# Story 12.6 — Multipart form-data truth

**GitHub issue:** [#397](https://github.com/microscaler/BRRTRouter/issues/397)  
**Epic:** [Epic 12](README.md)  
**Wave:** 3  
**Effort:** L  
**Blocked by:** 12.2 helpful (body caps)  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Stop silent multipart → empty JSON object bypass. Either parse
`multipart/form-data` (fields + files per OpenAPI `encoding`) or **fail closed**
with 415/501 when declared but unsupported.

## Delivery

- Decide MVP: (A) structured field parse into JSON-compatible map for validation, or (B) hard reject with clear error until full file API exists.
- Prefer (A) for text fields; file parts may be size-capped / deferred — document.
- Honor `request_content_types` 415 path already in service.
- Encoding object: minimal support or documented skip list.

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Declared multipart + valid parts | handler gets fields / 2xx |
| P2 | `application/json` sibling content-type | still works |
| P3 | Multipart with required field present | OK |
| P4 | Content-Type boundary parse | OK |
| P5 | Size under 12.2 cap | OK |
| P6 | Documented file-part policy | fixture matches docs |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Multipart when only JSON declared | **415** |
| N2 | Missing required multipart field | **400** |
| N3 | Malformed multipart | **400**; no panic |
| N4 | No silent `{}` body fabrication | assert body not empty object bypass |
| N5 | File over size cap | 413/400 |
| N6 | Missing boundary | 400 |
| N7 | Panic on parse | forbidden |
| N8 | Half-parsed state applied | forbidden |

### Acceptance criteria (tests)

- [x] N4 mandatory (no silent bypass); P1 or hard-reject path documented + tested.

## Acceptance criteria

- [x] No empty-object bypass for multipart.
- [x] Policy documented (parse vs reject) — `docs/multipart.md` (MVP A).
- [x] Unit tests section complete (`src/server/multipart.rs` + request parse tests).

## References

- `src/server/request.rs`, schema-validation llmwiki gaps
