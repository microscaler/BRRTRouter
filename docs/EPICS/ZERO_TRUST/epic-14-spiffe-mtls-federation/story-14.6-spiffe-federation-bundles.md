# Story 14.6 — SPIFFE Federation (bundles)

**GitHub issue:** [#419](https://github.com/microscaler/BRRTRouter/issues/419)  
**Epic:** [Epic 14](README.md)  
**Wave:** 4  
**Effort:** L  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Accept SPIFFE Federation trust bundles so foreign trust domains can be validated
for X.509 (and JWT if applicable) per configured federation policy.

## Delivery
- Federated bundle store keyed by trust domain.
- Config allowlist of federated domains.
- Document refresh from Federation API or static files (v1 can be file-based).

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | SVID from federated domain validates when bundle present. |
| FR-2 | Unknown foreign domain denied. |
| FR-3 | Bundle update for foreign domain hot-reloads. |
| FR-4 | Local domain still works without federation. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | Fail-closed for unknown domains. |
| NFR-2 | No panic on bad federated bundle. |
| NFR-3 | Domain allowlist required (no open federation). |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Federated domain + bundle | Ok |
| P2 | Local domain | Ok |
| P3 | Reload foreign bundle | Ok |
| P4 | Allowlist entry required | documented |
| P5 | JWT federated (if in scope) | per design |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Foreign domain not allowlisted | Err/deny |
| N2 | Missing foreign bundle | deny |
| N3 | Corrupt foreign bundle | keep last-good / deny; no panic |
| N4 | Open federation default | forbidden |
| N5 | Panic | forbidden |

### Acceptance criteria (tests)
- [ ] P1 and N1/N4 mandatory.

## Acceptance criteria
- [ ] Federation file-based v1; FR/NFR complete.

