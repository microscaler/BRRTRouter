# Epic 14 — SPIFFE X.509 / mTLS / Federation

**GitHub issue:** [#411](https://github.com/microscaler/BRRTRouter/issues/411)  
**Theme labels:** `zero-trust`, `epic`  
**Testing:** [`../TESTING_STANDARD.md`](../TESTING_STANDARD.md)  
**Board:** [`../BUILD_BOARD.md`](../BUILD_BOARD.md)

## Overview

Close the **critical** zero-trust gap for fintech/MedTech: BRRTRouter already
validates **SPIFFE JWT SVIDs** (~98% JWT path). Production mTLS requires
**X.509 SVID** peer identity, rotation/readiness, and **SPIFFE Federation** for
multi-cloud / cross-org trust. Also harden JWT SVID (revocation check, ECDSA).

**Does not include:** becoming a SPIFFE issuer (SPIRE remains the issuer);
WebSocket; OAS callback auto-fire.

Authority: [`docs/wip/SPIFFE_COMPLIANCE_ASSESSMENT.md`](../../../wip/SPIFFE_COMPLIANCE_ASSESSMENT.md).

## Success criteria (epic-level)

- [ ] X.509 SVID peer identity can authorize a route the same way JWT SVID does.
- [ ] mTLS material missing/expired → **fail-closed** (ready=false and/or 503/401 per policy).
- [ ] Federation can accept a foreign trust domain via SPIFFE Bundle.
- [ ] JWT SVID: optional `jti` revocation check + ECDSA algorithms.
- [ ] Stories meet TESTING_STANDARD; Sesame/platform guide documents SPIRE integration.

## Wave plan

```text
Wave 0 ──► 14.1 inventory & threat model
Wave 1 ──► 14.2 X.509 validate ‖ 14.3 mTLS request path
Wave 2 ──► 14.4 SecurityProvider integration
Wave 3 ──► 14.5 rotation & readiness
Wave 4 ──► 14.6 federation
Wave 5 ──► 14.7 JWT hardening (revocation + ECDSA)
Wave 6 ──► 14.8 reference integration + docs
```

## Stories

| Story | Title | Issue | Effort | Blocked by |
|-------|--------|-------|--------|------------|
| 14.1 | Zero-trust inventory & threat model | [#414](https://github.com/microscaler/BRRTRouter/issues/414) | S | — |
| 14.2 | X.509 SVID parse & validate | [#415](https://github.com/microscaler/BRRTRouter/issues/415) | L | 14.1 |
| 14.3 | mTLS peer identity on request path | [#416](https://github.com/microscaler/BRRTRouter/issues/416) | L | 14.2; may_minihttp |
| 14.4 | X.509 SecurityProvider → authz | [#417](https://github.com/microscaler/BRRTRouter/issues/417) | M | 14.3 |
| 14.5 | SVID/bundle rotation & fail-closed ready | [#418](https://github.com/microscaler/BRRTRouter/issues/418) | M | 14.2 |
| 14.6 | SPIFFE Federation (bundles) | [#419](https://github.com/microscaler/BRRTRouter/issues/419) | L | 14.2, 14.5 |
| 14.7 | JWT SVID hardening (revocation + ECDSA) | [#420](https://github.com/microscaler/BRRTRouter/issues/420) | M | — |
| 14.8 | Reference integration guide & e2e fixtures | [#421](https://github.com/microscaler/BRRTRouter/issues/421) | M | 14.4–14.6 |

## Functional requirements (epic)

| ID | Requirement |
|----|-------------|
| E-FR-1 | Extract SPIFFE ID from validated X.509 SVID peer certificate. |
| E-FR-2 | Enforce trust domain / URI SAN rules per SPIFFE X.509-SVID. |
| E-FR-3 | Map peer SPIFFE ID into the same security/authz pipeline as JWT SVID. |
| E-FR-4 | Rotate leaf/intermediate/bundle without process restart (or documented hot-reload). |
| E-FR-5 | Accept federated trust bundles for configured foreign domains. |
| E-FR-6 | Optional JWT `jti` revocation lookup before accept. |
| E-FR-7 | Verify JWT SVIDs signed with ES256/ES384/ES512 when keys present. |

## Non-functional requirements (epic)

| ID | Requirement |
|----|-------------|
| E-NFR-1 | Fail-closed on invalid/expired/missing trust material. |
| E-NFR-2 | No panic on malformed certs/bundles. |
| E-NFR-3 | Secrets/key material never logged. |
| E-NFR-4 | Hot path after identity extract stays lock-friendly (ArcSwap-style snapshots). |
| E-NFR-5 | Clear split: BRRTRouter is SVID **consumer**, not issuer. |

## References

- `src/security/spiffe/`
- `docs/wip/SPIFFE_COMPLIANCE_ASSESSMENT.md`, `SPIFFE_ROADMAP_FINTECH.md`
- SPIFFE X.509-SVID / Federation specs
