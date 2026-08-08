# Story 15.8 — OAS 3.2 dual-support readiness

**GitHub issue:** [#429](https://github.com/microscaler/BRRTRouter/issues/429)  
**Epic:** [Epic 15](README.md)  
**Wave:** 5  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Plan and spike dual-support for OpenAPI 3.2 (PathItem.query native) without
forcing fleet cutover; track oas3 crate gaps.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Readiness doc lists 3.2 features vs BRRTRouter/oas3 status. |
| FR-2 | QUERY continue to work on 3.1 promote path. |
| FR-3 | Decision record: when to bump default fleet version. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | No forced 3.2 in product specs this epic. |
| NFR-2 | Spike code behind flag or docs-only if blocked on oas3. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Readiness doc | present |
| P2 | 3.1 QUERY regression | ok |
| P3 | ADR/decision | present |
| P4 | Gap links oas3 issues | yes |
| P5 | Catalog points here | yes |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Silent fleet cutover | forbidden |
| N2 | Break 3.1 load | forbidden |
| N3 | Claim full 3.2 parse if untrue | forbidden |
| N4 | Panic on openapi:3.2.0 | forbidden or handled |
| N5 | Drop Epic 11 promote path without replacement | forbidden |

### Acceptance criteria (tests)
- [ ] P1/P2 and N2 mandatory.

## Acceptance criteria
- [ ] Readiness + decision published; FR/NFR complete.

