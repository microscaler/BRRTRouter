# Story 15.1 — Remaining OpenAPI gap inventory

**GitHub issue:** [#422](https://github.com/microscaler/BRRTRouter/issues/422)  
**Epic:** [Epic 15](README.md)  
**Wave:** 0  
**Effort:** S  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Reconcile `OPENAPI_3.1.0_COMPLIANCE_GAP.md` for surfaces **beyond** Epic 12/13
(shipped `$ref`, params, multipart MVP) into a backlog owned by Epic 15.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Gap doc rows for response headers, servers, encoding, callbacks, links updated. |
| FR-2 | Each open row maps to a 15.x story or parked note. |
| FR-3 | Epic 12.3 `$ref` rows marked supported. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | Deterministic doc fixtures. |
| NFR-2 | No marketing overclaim. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Gap doc links Epic 15 | yes |
| P2 | $ref marked supported | yes |
| P3 | Response headers row present | yes |
| P4 | callbacks row explicit | yes |
| P5 | servers row explicit | yes |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | $ref still ❌ | forbidden |
| N2 | Unmapped open gap | forbidden |
| N3 | Claim auto-fire shipped | forbidden |
| N4 | Broken links | forbidden |
| N5 | Duplicate conflicting rows | forbidden |

### Acceptance criteria (tests)
- [ ] P1–P3 and N1 mandatory.

## Acceptance criteria
- [ ] Inventory reconciled; FR/NFR complete.

