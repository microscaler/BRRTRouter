# SPIFFE / zero-trust inventory & threat model (Story 14.1)

**Status:** Living inventory — Epic 14  
**Identity boundary:** [JWT_AND_IDENTITY_BOUNDARY.md](./JWT_AND_IDENTITY_BOUNDARY.md)  
**Authority (historical):** [wip/SPIFFE_COMPLIANCE_ASSESSMENT.md](./wip/SPIFFE_COMPLIANCE_ASSESSMENT.md)

## Role

BRRTRouter is a SPIFFE / JWT **consumer**, not an issuer. Workload identity is
issued by **SPIRE** (or equivalent). Application user JWTs are issued by
**Sesame-IDAM** or another external IdP. The router validates and enforces.

## Capability matrix

| Capability | Status | Owner story | Notes |
|------------|--------|-------------|--------|
| SPIFFE JWT SVID validate | ✅ Shipped | — | Trust domain, aud, iss/iat/nbf/jti extract |
| User JWT via JWKS (Sesame / external) | ✅ Shipped | — | `JwksBearerProvider` |
| Local HMAC `BearerJwtProvider` | ⚠️ Dev | — | Not production IdP |
| `OAuth2Provider` simplified | ⚠️ Stub | — | Prefer JWKS; see boundary doc |
| X.509 SVID parse & validate | ❌ | **14.2** | Leaf + chain + URI SAN |
| mTLS peer identity → request | ❌ | **14.3** | may_minihttp TLS hooks may block |
| X.509 → SecurityProvider / authz | ❌ | **14.4** | Same pipeline as JWT SVID |
| Bundle/SVID rotation + ready | ❌ | **14.5** | Fail-closed |
| SPIFFE Federation bundles | ❌ | **14.6** | Allowlisted foreign domains |
| JWT `jti` revoke via **external** checker | ❌ | **14.7** | No in-router IdP revocation DB |
| ECDSA ES256/384/512 JWT verify | ❌ | **14.7** | Consumer completeness |
| Hermetic fixtures + operator guide | ❌ | **14.8** | SPIRE/Sesame integration notes |

## Threat model (summary)

| Threat | Mitigation (target) |
|--------|---------------------|
| Spoofed peer without valid SVID | Fail-closed mTLS + SAN SPIFFE ID check (14.2–14.4) |
| Expired / wrong trust domain | Reject; ready=false when material unusable (14.5) |
| Bundle downgrade / corrupt reload | Keep last-good snapshot; metric (14.5) |
| Open federation | Allowlist only (14.6) |
| Stolen user JWT | IdP revocation; optional external `jti` hook (14.7) — **not** BRRTRouter-issued sessions |
| Treating router as IdP | Forbidden — [JWT_AND_IDENTITY_BOUNDARY.md](./JWT_AND_IDENTITY_BOUNDARY.md) |

## may_minihttp / TLS assumptions

- Production mTLS likely requires TLS termination that exposes the **peer certificate**
  (or SPIFFE ID) into the accept path.
- Gaps in may_minihttp are **blockers for 14.3**, tracked as part of that story —
  not solved by inventing an IdP inside BRRTRouter.
- Dev may use plaintext + JWT-only; document clearly.

## Explicit non-goals

- WebSocket identity upgrade
- Becoming a SPIFFE Workload API **server**
- Replacing Sesame-IDAM token issuance
- OAS callback auto-fire

## Related epics

- Epic 14 board: [EPICS/ZERO_TRUST/BUILD_BOARD.md](./EPICS/ZERO_TRUST/BUILD_BOARD.md)
- Framework JWT/JWKS docs: [SecurityAuthentication.md](./SecurityAuthentication.md)
- Sesame-IDAM: https://github.com/microscaler/sesame-idam
