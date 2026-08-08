# Story 15.5 — HTTP method matrix (OPTIONS/TRACE)

**GitHub issue:** [#426](https://github.com/microscaler/BRRTRouter/issues/426)  
**Epic:** [Epic 15](README.md)  
**Wave:** 3  
**Effort:** S–M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Document and implement consistent OPTIONS `Allow` advertisement and TRACE policy.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | OPTIONS on known path returns `Allow` listing implemented methods. |
| FR-2 | TRACE default deny or documented allow-if-declared. |
| FR-3 | CORS preflight still works (regression). |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | No panic on OPTIONS for unknown path (404/405 policy documented). |
| NFR-2 | Method matrix doc published. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | OPTIONS Allow contains GET/POST | yes |
| P2 | CORS preflight | regression |
| P3 | Declared TRACE if allowed | per policy |
| P4 | Matrix doc | present |
| P5 | HEAD still omits body | regression |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | TRACE when default deny | 405/404 |
| N2 | Panic | forbidden |
| N3 | Allow empty when methods exist | forbidden |
| N4 | Break CORS | forbidden |
| N5 | Advertise TRACE when denied | forbidden |

### Acceptance criteria (tests)
- [ ] P1 and N1 mandatory.

## Acceptance criteria
- [ ] Policy shipped; FR/NFR complete.

