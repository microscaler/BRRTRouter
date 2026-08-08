# Story 15.6 — API versioning pattern

**GitHub issue:** [#427](https://github.com/microscaler/BRRTRouter/issues/427)  
**Epic:** [Epic 15](README.md)  
**Wave:** 3  
**Effort:** S–M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Document supported versioning patterns (path prefix via servers, optional header)
and provide a minimal helper or codegen note — not a full negotiation engine.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Guide shows `/v1` via servers/basePath. |
| FR-2 | Optional `Accept-Version` / header pattern documented as app-level or helper. |
| FR-3 | Example spec in pet_store or docs. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | No mandatory breaking change to unversioned apps. |
| NFR-2 | Honest about what is not automatic. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Guide exists | yes |
| P2 | servers example works with 15.3 | yes |
| P3 | Unversioned app unaffected | yes |
| P4 | Linked from BUILDING doc | yes |
| P5 | Fixture | yes |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Claim automatic Accept negotiation if not built | forbidden |
| N2 | Force version on all routes silently | forbidden |
| N3 | Broken example | forbidden |
| N4 | Panic in helper | forbidden |
| N5 | Require header globally by default | forbidden |

### Acceptance criteria (tests)
- [ ] P1/P3 and N1 mandatory.

## Acceptance criteria
- [ ] Pattern documented; FR/NFR complete.

