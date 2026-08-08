# Story 14.4 — X.509 SecurityProvider → authz

**GitHub issue:** [#417](https://github.com/microscaler/BRRTRouter/issues/417)  
**Epic:** [Epic 14](README.md)  
**Wave:** 2  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
`SecurityProvider` (or equivalent) that consumes peer X.509 SVID identity and
applies the same route security requirements as JWT SVID (audiences/trust domains).

## Delivery
- Provider registration in `AppConfig` / security setup.
- Map SPIFFE ID → principal for RBAC hooks if present.
- Align errors with problem+json if Epic 13.3 shipped (dual-support OK).

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Route requiring SPIFFE X.509 accepts valid peer ID. |
| FR-2 | Wrong audience/trust domain → deny. |
| FR-3 | Handler can read authenticated SPIFFE ID from context. |
| FR-4 | Config can require JWT **or** X.509 **or** either. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | Fail-closed default. |
| NFR-2 | No panic on provider misconfig. |
| NFR-3 | Metrics for accept/deny. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Valid X.509 peer | 2xx |
| P2 | Context has SPIFFE ID | present |
| P3 | Either-JWT-or-X509 mode | both paths |
| P4 | Deny metric | increments |
| P5 | Accept metric | increments |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Wrong domain | 401/403 |
| N2 | Missing identity | deny |
| N3 | Panic | forbidden |
| N4 | Auth bypass | forbidden |
| N5 | Secret in error | forbidden |

### Acceptance criteria (tests)
- [ ] P1 and N1/N4 mandatory.

## Acceptance criteria
- [ ] Provider wired; docs; FR/NFR complete.

