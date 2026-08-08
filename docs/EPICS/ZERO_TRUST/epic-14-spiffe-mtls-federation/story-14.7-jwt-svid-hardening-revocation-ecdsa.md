# Story 14.7 — JWT SVID hardening (external revocation hook + ECDSA)

**GitHub issue:** [#420](https://github.com/microscaler/BRRTRouter/issues/420)  
**Epic:** [Epic 14](README.md)  
**Wave:** 5  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Harden JWT **verification** (SPIFFE JWT SVID and user JWTs via JWKS): ECDSA
algorithms and an optional **`jti` revocation hook** that calls out to
Sesame-IDAM or another external authority.

BRRTRouter remains a **consumer/enforcer** — see
[`JWT_AND_IDENTITY_BOUNDARY.md`](../../../JWT_AND_IDENTITY_BOUNDARY.md).
It must **not** grow an IdP-owned revocation database, session store, or token
issuer.

## Delivery

- Enable ES256 / ES384 / ES512 when JWKS keys require them.
- Trait e.g. `JtiRevocationChecker` with async/sync check → Allow / Deny / Unavailable.
- Reference adapters: in-memory for tests; HTTP checker docs pointing at Sesame/IdP.
- **No** Redis/session “revocation product” inside BRRTRouter.
- Metrics for revoke hits / checker failures.
- Default: revocation **off**.

## Functional requirements

| ID | Requirement |
|----|-------------|
| FR-1 | When checker enabled and reports revoked → deny. |
| FR-2 | Unknown `jti` → allow unless checker policy says otherwise. |
| FR-3 | ES256-signed JWT verifies with EC JWKS. |
| FR-4 | RS256 path regression still works. |
| FR-5 | Docs state checker is external; Sesame/IdP owns revocation data. |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| NFR-1 | Checker timeout / fail-closed vs fail-open documented and configurable. |
| NFR-2 | No panic on malformed `jti`. |
| NFR-3 | Default revocation **off**. |
| NFR-4 | No in-tree IdP revocation schema or mandatory Redis. |

## Unit tests

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Non-revoked jti (mock checker) | allow |
| P2 | ES256 token | allow |
| P3 | RS256 regression | allow |
| P4 | Revocation disabled | allow even if mock would deny |
| P5 | Metric on revoke hit | yes |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Revoked jti | deny |
| N2 | Bad ES signature | deny |
| N3 | Panic | forbidden |
| N4 | Checker unavailable + fail-closed | deny |
| N5 | Secret in logs | forbidden |
| N6 | Shipping Redis revocation DB as required component | forbidden |

### Acceptance criteria (tests)

- [ ] P2/P3 and N1/N6 mandatory.

## Acceptance criteria

- [ ] Feature flagged; boundary docs linked; FR/NFR complete.
