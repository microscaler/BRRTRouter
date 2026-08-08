# Browser security posture (Epic 13.5)

**Decision: Option B — Bearer / JWKS only (no server-session framework).**

BRRTRouter does **not** ship cookie-session middleware or CSRF protection for
browser login sessions. Identity and sessions belong to
[Sesame-IDAM](https://github.com/microscaler/sesame-idam) (or another IdP).
This router **validates and enforces** tokens; it does not issue them.

See also: [`JWT_AND_IDENTITY_BOUNDARY.md`](./JWT_AND_IDENTITY_BOUNDARY.md).

## What is in scope

| Capability | Notes |
|------------|--------|
| Bearer JWT via `Authorization` | Production: `JwksBearerProvider` |
| JWT / API key **read from a cookie** | Existing `cookie_name()` on providers — transport only |
| `SetCookieBuilder` | Helpers to emit `Set-Cookie` with `Secure` / `HttpOnly` / `SameSite` for **non-session** app cookies (e.g. CSRF-less preference flags, or carrying a JWT the IdP already issued) |
| CORS | Existing `CorsMiddleware` |

## What is out of scope

| Capability | Owner |
|------------|--------|
| Server-side session store | Sesame-IDAM / IdP |
| Login / logout / refresh cookie sessions | Sesame-IDAM / IdP |
| CSRF synchronizer / double-submit kit | Not shipped — use Bearer or IdP BFF patterns |
| “Sessions included” as a framework feature | **Forbidden claim** |

## Production guidance

1. Prefer `Authorization: Bearer <access_token>` with JWKS validation.
2. If a cookie must carry a token (legacy SPA), use `cookie_name` on the provider
   and set cookies with [`SetCookieBuilder`](../src/security/set_cookie.rs)
   defaults: `Secure`, `HttpOnly`, `SameSite=Lax` (or `Strict` when appropriate).
3. Do **not** treat BRRTRouter as a session server.

## Option A (not chosen)

A full cookie + CSRF kit remains deferred. Revisit only if a product BFF has a
hard requirement that Sesame cannot satisfy.
