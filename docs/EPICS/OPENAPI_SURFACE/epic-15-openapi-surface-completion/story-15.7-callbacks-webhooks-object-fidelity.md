# Story 15.7 — callbacks/webhooks object fidelity

**GitHub issue:** [#428](https://github.com/microscaler/BRRTRouter/issues/428)  
**Epic:** [Epic 15](README.md)  
**Wave:** 4  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Stop silent ignorance of OAS `callbacks` / root `webhooks`: parse into metadata
and/or lint warnings. **Auto-fire runtime stays parked**; outbound kit remains 12.5.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Spec with `callbacks` does not silently lose the field without warn/metadata. |
| FR-2 | Root `webhooks` similarly surfaced or warned. |
| FR-3 | Docs state auto-fire not implemented; point to `deliver_webhook`. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | No panic on complex callback graphs. |
| NFR-2 | Load cost acceptable (dev-time lint OK). |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | callbacks present → warn or metadata | yes |
| P2 | webhooks present → warn or metadata | yes |
| P3 | Docs point to 12.5 kit | yes |
| P4 | Routes still register | yes |
| P5 | Lint CLI surfaces | yes |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Silent drop with zero signal | forbidden |
| N2 | Claim auto-fire | forbidden |
| N3 | Panic on callback $ref | forbidden |
| N4 | Break unrelated paths | forbidden |
| N5 | Execute outbound on parse | forbidden |

### Acceptance criteria (tests)
- [ ] P1 and N1/N2 mandatory.

## Acceptance criteria
- [ ] Fidelity without auto-fire; FR/NFR complete.

