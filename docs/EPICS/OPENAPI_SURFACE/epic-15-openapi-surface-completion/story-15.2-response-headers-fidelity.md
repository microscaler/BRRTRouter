# Story 15.2 — Response headers fidelity

**GitHub issue:** [#423](https://github.com/microscaler/BRRTRouter/issues/423)  
**Epic:** [Epic 15](README.md)  
**Wave:** 1  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Honor OpenAPI response `headers` — validate and/or codegen typed header setters.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Declared required response header missing → validation error or codegen guard. |
| FR-2 | Known headers (e.g. RateLimit) can be set from handlers via helper. |
| FR-3 | Undeclared headers policy documented (allow by default). |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | No panic on empty headers map. |
| NFR-2 | Hot path skip when no header schemas. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Required header present | ok |
| P2 | Helper sets header | observed |
| P3 | Optional omitted | ok |
| P4 | Codegen/docs mention headers | yes |
| P5 | Regression JSON body | ok |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Required missing | fail/4xx or build error per design |
| N2 | Panic | forbidden |
| N3 | Silent ignore required | forbidden |
| N4 | Wrong type if schema typed | fail |
| N5 | Credential header echoed wrongly | forbidden |

### Acceptance criteria (tests)
- [ ] P1 and N1/N3 mandatory.

## Acceptance criteria
- [ ] Runtime and/or codegen path shipped; FR/NFR complete.

