# JWT & identity boundary

**Decision (2026-08):** BRRTRouter is a **consumer and enforcer** of JWTs and
SPIFFE identities. It does **not** reproduce an identity provider.

## In scope (BRRTRouter)

| Capability | Notes |
|------------|--------|
| Validate Bearer JWT via JWKS | `JwksBearerProvider` — keys from Sesame-IDAM, Auth0, Cognito, Keycloak, PropelAuth, etc. |
| Validate SPIFFE JWT SVID | Existing SPIFFE JWT path |
| Enforce scopes / audiences / trust domains | Fail-closed authz on routes |
| Optional `jti` check against an **external** revocation signal | Pluggable hook only (Epic 14.7) — no IdP-owned Redis session store |
| mTLS peer X.509 SVID validate (Epic 14) | Consumer of SPIRE-issued material |

## Out of scope (use Sesame-IDAM or another IdP)

| Capability | Owner |
|------------|--------|
| Issue access / refresh tokens | [Sesame-IDAM](https://github.com/microscaler/sesame-idam) / external IdP |
| User signup, password, OTP, magic link | Sesame-IDAM / GoTrue-shaped IDAM |
| Session cookies as the IdP session store | Sesame-IDAM — BRRTRouter is Option B (Bearer/JWKS); see [`BROWSER_SECURITY_POSTURE.md`](./BROWSER_SECURITY_POSTURE.md) |
| OAuth authorization server / consent UI | External IdP |
| Long-lived revocation database as product feature | IdP / platform — BRRTRouter may call it |

## Deprecated / non-production paths in-tree

| Type | Status |
|------|--------|
| `OAuth2Provider` (simplified local JWT) | **Dev/stub only** — do not use in production; prefer `JwksBearerProvider` |
| `BearerJwtProvider` with shared HMAC secret | Dev / tests; production → JWKS |

## Product reference

- Public reference consumer: **Sesame-IDAM**
- BFF claim enrichment (Epics 3–5) calls IDAM for extra claims — still **not** token issuance inside the router

## Related

- [SecurityAuthentication.md](./SecurityAuthentication.md)
- Epic 14: [ZERO_TRUST](./EPICS/ZERO_TRUST/README.md)
- Epic 13.5: [`BROWSER_SECURITY_POSTURE.md`](./BROWSER_SECURITY_POSTURE.md) (Option B + `SetCookieBuilder`)
