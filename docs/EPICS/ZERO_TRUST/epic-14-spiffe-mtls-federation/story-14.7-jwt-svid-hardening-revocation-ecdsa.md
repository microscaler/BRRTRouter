# Story 14.7 — JWT SVID hardening (revocation + ECDSA)

**GitHub issue:** [#420](https://github.com/microscaler/BRRTRouter/issues/420)  
**Epic:** [Epic 14](README.md)  
**Wave:** 5  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Complete JWT SVID production gaps from the compliance assessment: optional
`jti` revocation checking and ECDSA (ES256/384/512) verification.

## Delivery
- Pluggable revocation store trait (memory/Redis later); in-memory for tests.
- Enable ES* algs in JWT SVID verification path.
- Metrics for revocation hits.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Revoked `jti` → deny when checker enabled. |
| FR-2 | Unknown `jti` → allow (unless deny-unknown mode). |
| FR-3 | ES256-signed JWT SVID verifies with EC JWKS. |
| FR-4 | RS256 path regression still works. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | Revocation check timeout/fail policy documented (fail-closed vs open). |
| NFR-2 | No panic on malformed jti. |
| NFR-3 | Default revocation **off** until configured. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Non-revoked jti | allow |
| P2 | ES256 token | allow |
| P3 | RS256 regression | allow |
| P4 | Revocation disabled | allow even if listed |
| P5 | Metric on revoke hit | yes |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Revoked jti | deny |
| N2 | Bad ES signature | deny |
| N3 | Panic | forbidden |
| N4 | Revocation store down + fail-closed | deny |
| N5 | Secret in logs | forbidden |

### Acceptance criteria (tests)
- [ ] P2/P3 and N1 mandatory.

## Acceptance criteria
- [ ] Feature flagged; docs; FR/NFR complete.

