# Story 13.9 — Multi-status response codegen

**GitHub issue:** [#409](https://github.com/microscaler/BRRTRouter/issues/409)  
**Epic:** [Epic 13](README.md)  
**Wave:** 5  
**Effort:** M  
**Blocked by:** Story 12.7 runtime (`HttpJson` / `HttpNoContent`)  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Finish the typed multi-status story on the **codegen** side: generate per-status
response types / `Result`-style aliases from OpenAPI `responses` so handlers are
not stuck with ad-hoc status selection or `panic!` stubs.

## Delivery

- Generator emits enums or aliases for operations with multiple JSON response statuses.
- Stubs return a documented default success variant (not `panic!` for status).
- Integrate with `HttpJson::created` / `HttpNoContent` patterns from 12.7.
- Document migration for existing controllers.
- Non-goal: full content-negotiation across all media types in v1 of this story.

## Functional requirements

| ID | Requirement |
|----|-------------|
| FR-1 | Operation with 200+201 JSON responses generates a typed multi-status return. |
| FR-2 | Generated stub compiles without `panic!` for status selection. |
| FR-3 | Handler can return 201 via typed API matching OpenAPI. |
| FR-4 | 204/`HttpNoContent` operations generate appropriate empty success type. |
| FR-5 | Single-200 operations remain simple (`HttpJson<T>` or equivalent). |
| FR-6 | `brrtrouter-gen` / CLI regenerate is idempotent for unchanged specs. |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| NFR-1 | Generated code passes `cargo check` in pet_store or fixture crate. |
| NFR-2 | No unstable type names across regenerations without schema change. |
| NFR-3 | Generator never panics on empty responses map (error or skip with warning). |

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Spec with 200+201 | enum/alias contains both |
| P2 | Generated stub builds | `cargo check` |
| P3 | 204 operation | NoContent-style success |
| P4 | Single 200 | simple HttpJson path |
| P5 | Re-generate twice | stable output |
| P6 | Pet-store or fixture operation sample | golden/compile |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Stub uses `panic!` for status | forbidden |
| N2 | Missing responses map | no panic; clear error/warn |
| N3 | Invalid status key in spec | skip/error; no corrupt Rust |
| N4 | Silent drop of 4xx response type when declared | forbidden if in scope |
| N5 | Generated code unused_mut / broken imports | forbidden |
| N6 | Panic in generator | forbidden |

### Acceptance criteria (tests)

- [x] P1/P2 and N1/N2 mandatory.

## Acceptance criteria

- [x] Codegen docs updated (`PRD_TYPED_HANDLER_HTTP_STATUS` or successor).
- [x] FR/NFR + unit tests complete.

## References

- `src/generator/`, `src/typed/core.rs`, story 12.7
- `docs/PRD_TYPED_HANDLER_HTTP_STATUS.md`
