# Story 11.2 — OpenAPI QUERY operations

**GitHub issue:** [#387](https://github.com/microscaler/BRRTRouter/issues/387)  
**Epic:** [Epic 11 — HTTP QUERY](README.md)  
**Blocked by:** 11.1  
**Testing standard:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Spec load + generator recognize QUERY operations with request bodies (query media types).

## Delivery

- Spec parser: allow `QUERY` in path item operations (as extension or when OpenAPI tooling supports it).
- Generator/Askama: emit handlers for QUERY + body schemas.
- Document how consumers declare QUERY in suite configs until OAS formally lists it everywhere.

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Example OpenAPI with QUERY loads | no parse error |
| P2 | Generated handler for QUERY | receives typed/raw body |
| P3 | QUERY + JSON body schema | validation accepts valid body |
| P4 | QUERY + form media type (if supported) | documented + tested |
| P5 | Mixed path: GET + QUERY same path | both registered |
| P6 | Suite config declaration path | loads per docs |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | QUERY without body when required | 400 / codegen validation error |
| N2 | Invalid body vs schema | 400; no panic |
| N3 | Unsupported tooling path | clear error or documented skip; no silent drop |
| N4 | Malformed OpenAPI method field | load error |
| N5 | QUERY with illegal path template | fail closed |
| N6 | Conflicting duplicate QUERY ops | error at load |
| N7 | Body too large | existing limit policy; no panic |
| N8 | Generator emit failure | fails build/test; no half handler |

### Acceptance criteria (tests)

- [ ] P1/P2 mandatory; N1/N2 mandatory.

## Acceptance criteria

- [ ] Example OpenAPI snippet in docs loads without error.
- [ ] Generated handler receives body for QUERY.
- [ ] Unknown/unsupported tooling path documented.
- [ ] Unit tests section complete (positive + negative).

## References

- RFC 10008 §2.1 (media types)
- `src/spec/`, generator templates
