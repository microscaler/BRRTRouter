# Story 12.3 — OpenAPI `$ref` for requestBodies / responses / pathItems

**GitHub issue:** [#394](https://github.com/microscaler/BRRTRouter/issues/394)  
**Epic:** [Epic 12](README.md)  
**Wave:** 1  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Resolve `components.requestBodies`, `components.responses`, and
`components.pathItems` / path `$ref` so enterprise specs do not **silently drop**
schemas or routes.

## Delivery

- Spec load / `build_routes`: follow `$ref` for requestBody and response objects.
- Path Item `$ref` / `components.pathItems` become real routes (or fail closed with clear error).
- Generator consumes resolved schemas (no empty Request when ref existed).
- Document unsupported ref shapes.

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | `requestBody: $ref: #/components/requestBodies/X` | `request_schema` present |
| P2 | Response `$ref` components.responses | response schema present |
| P3 | Nested schema `$ref` inside resolved body | expanded / usable |
| P4 | `components.pathItems` + path `$ref` | route registered |
| P5 | Mixed inline + ref ops same spec | both work |
| P6 | Existing pet_store inline bodies | regression |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Dangling requestBody `$ref` | load/validation error; **no silent empty** |
| N2 | Dangling response `$ref` | clear error or documented skip with issue |
| N3 | Circular `$ref` | fail closed; no panic / infinite loop |
| N4 | External HTTP `$ref` (if unsupported) | clear error |
| N5 | Wrong component type `$ref` | error |
| N6 | Path `$ref` missing | error; route not half-registered |
| N7 | Generator emit with unresolved body | fails build/test |
| N8 | Panic on resolve | forbidden |

### Acceptance criteria (tests)

- [ ] P1/P2 and N1 mandatory.

## Acceptance criteria

- [ ] Fixture specs with component refs load with schemas.
- [ ] No silent schema drop for supported local refs.
- [ ] Unit tests section complete.

## References

- `OPENAPI_3.1.0_COMPLIANCE_GAP.md` §3, §10
- `src/spec/build.rs`
