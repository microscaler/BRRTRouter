# Story 14.2 — X.509 SVID parse & validate

**GitHub issue:** [#415](https://github.com/microscaler/BRRTRouter/issues/415)  
**Epic:** [Epic 14](README.md)  
**Wave:** 1  
**Effort:** L  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview
Library path to parse X.509 SVID certificates, validate chain against trust
bundle, enforce SPIFFE ID in URI SAN, expiry, and basic key usage.

## Delivery
- Module under `src/security/spiffe/` (x509).
- Trust bundle load from PEM/JWKS-equivalent SPIFFE bundle format (document).
- Return structured `SpiffeId` + validation error taxonomy.

## Functional requirements
| ID | Requirement |
|----|-------------|
| FR-1 | Valid leaf+chain against bundle → Ok(SPIFFE ID). |
| FR-2 | URI SAN `spiffe://` required; extract path/trust domain. |
| FR-3 | Expired leaf → Err. |
| FR-4 | Wrong trust domain → Err. |
| FR-5 | Untrusted intermediate/root → Err. |

## Non-functional requirements
| ID | Requirement |
|----|-------------|
| NFR-1 | No panic on truncated/malformed DER/PEM. |
| NFR-2 | Constant-ish failure messages (no key material). |
| NFR-3 | Bundle parse errors are typed. |

## Unit tests
### Positive
| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Golden valid SVID fixture | Ok + ID |
| P2 | Trust domain match | Ok |
| P3 | Intermediate chain | Ok |
| P4 | Clock leeway documented/tested | Ok within leeway |
| P5 | Multiple URI SAN pick SPIFFE | Ok |
### Negative
| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Expired | Err |
| N2 | Wrong domain | Err |
| N3 | Missing URI SAN | Err |
| N4 | Truncated PEM | Err; no panic |
| N5 | Empty bundle | Err |
| N6 | Panic | forbidden |

### Acceptance criteria (tests)
- [ ] P1 and N1/N4 mandatory.

## Acceptance criteria
- [ ] Library API + fixtures; FR/NFR complete.

