# Story 16.1 — Public API stability policy

**GitHub issue:** [#430](https://github.com/microscaler/BRRTRouter/issues/430)  
**Epic:** [Epic 16](README.md)  
**Wave:** 0  
**Effort:** S  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Define and document the public API surface and stability guarantees for 0.x/1.0.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Policy doc lists stable modules vs experimental. |
| FR-2 | `pub use` surface audited; hidden items marked. |
| FR-3 | Breaking-change definition for 0.x. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | Linked from README/PUBLISHING. |
| NFR-2 | Fixture test that policy file exists. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Policy doc | present |
| P2 | Lists typed/http/server exports | yes |
| P3 | README link | yes |
| P4 | Experimental callout | yes |
| P5 | Epic 16 board link | yes |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Claim 1.0 stable if still alpha | forbidden |
| N2 | Unmarked pub experimental as stable | forbidden |
| N3 | Broken links | forbidden |
| N4 | Secrets in policy | forbidden |
| N5 | Silence on semver | forbidden |

### Acceptance criteria (tests)
- [ ] P1 and N1 mandatory.

## Acceptance criteria
- [ ] Policy published; FR/NFR complete.

