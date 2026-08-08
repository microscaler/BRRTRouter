# Story 16.5 — Consumer migration guide (alpha → 0.1)

**GitHub issue:** [#434](https://github.com/microscaler/BRRTRouter/issues/434)  
**Epic:** [Epic 16](README.md)  
**Wave:** 4  
**Effort:** S  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Write Sesame-oriented migration notes for alpha → 0.1 (error shapes, features,
SPIFFE, TestApp).

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Migration guide covers breaking changes list. |
| FR-2 | Points at problem+json / rate-limit / TestApp when shipped. |
| FR-3 | SPIFFE mTLS called out as Epic 14 dependency. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | Honest about unfinished epics. |
| NFR-2 | Linked from BUILDING_WITH_BRRTRouter. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Guide exists | yes |
| P2 | Links BUILDING doc | yes |
| P3 | Lists breakages | yes |
| P4 | Mentions Epic 14 | yes |
| P5 | Mentions Epic 13 kits | yes |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Claim all epics done | forbidden |
| N2 | Broken links | forbidden |
| N3 | Hauliage as public reference | forbidden |
| N4 | Skip Sesame | forbidden |
| N5 | Secret examples | forbidden |

### Acceptance criteria (tests)
- [ ] P1 and N1/N3 mandatory.

## Acceptance criteria
- [ ] Guide published; FR/NFR complete.

