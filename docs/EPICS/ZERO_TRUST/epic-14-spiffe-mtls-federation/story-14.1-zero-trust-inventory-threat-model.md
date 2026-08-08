# Story 14.1 — Zero-trust inventory & threat model

**GitHub issue:** [#414](https://github.com/microscaler/BRRTRouter/issues/414)  
**Epic:** [Epic 14](README.md)  
**Wave:** 0  
**Effort:** S  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Document JWT-SVID vs X.509-SVID vs Federation scope, may_minihttp TLS constraints,
threat model (spoofed SAN, expired leaf, wrong trust domain), and epic boundary
(consumer not issuer).

## Delivery
- Living doc: [`docs/SPIFFE_ZERO_TRUST_INVENTORY.md`](../../../SPIFFE_ZERO_TRUST_INVENTORY.md).
- Identity boundary: [`docs/JWT_AND_IDENTITY_BOUNDARY.md`](../../../JWT_AND_IDENTITY_BOUNDARY.md) (Sesame/external IdP issues JWTs; router enforces).
- Matrix: capability × status × story owner.
- Explicit may_minihttp / TLS termination assumptions.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Matrix lists JWT SVID, X.509 SVID, Federation, revocation, ECDSA with status. |
| FR-2 | Threat model covers peer spoof, expired cert, bundle downgrade. |
| FR-3 | Documents BRRTRouter as SPIFFE consumer only. |
| FR-4 | Links each gap to a 14.x story. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | No false “mTLS shipped” claims. |
| NFR-2 | Links resolve. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Matrix present | yes |
| P2 | Consumer-not-issuer stated | yes |
| P3 | Stories cross-linked | yes |
| P4 | Threat model section | yes |
| P5 | may_minihttp TLS notes | yes |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Claim X.509 shipped before 14.2–14.4 | forbidden |
| N2 | Claim issuer/Workload API server | forbidden |
| N3 | Broken links | forbidden |
| N4 | WS required for mTLS | forbidden |
| N5 | Silent omit of Federation | forbidden |

### Acceptance criteria (tests)
- [x] Doc fixtures cover P1–P3 and N1–N2 (`tests/epic14_1_zero_trust_inventory_tests.rs`).

## Acceptance criteria
- [x] Inventory published; FR/NFR complete.

