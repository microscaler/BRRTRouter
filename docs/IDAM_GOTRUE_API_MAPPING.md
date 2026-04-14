# IDAM Core vs Extension: GoTrue/Supabase Auth API Mapping

**Purpose:** Use the **actual** Supabase GoTrue API to decide what belongs in the **IDAM core** (reference implementation) vs **IDAM extension** (product-specific). This document analyses the GoTrue codebase and OpenAPI spec and tabulates functionality so the core/extension split is informed by real APIs.

**Sources:**
- GoTrue (Supabase Auth): `supabase/auth/openapi.yaml`, `internal/api/api.go`, `internal/api/router.go`
- IDAM strategy: [IDAM Microscaler Analysis](IDAM_MICROSCALER_ANALYSIS.md), [IDAM Design: Core and Extension](IDAM_DESIGN_CORE_AND_EXTENSION.md)

---

## 1. GoTrue API Summary (from openapi.yaml and router)

GoTrue exposes a REST API under `https://{project}.supabase.co/auth/v1`. Routes are registered in `internal/api/api.go` (chi router). The following table lists **paths**, **methods**, **OpenAPI summary**, and **tag** (auth, user, oauth-client, oauth-server, sso, saml, admin, general).

### 1.1 Paths and functionality

| Path | Method | Summary (from OpenAPI) | Tag |
|------|--------|------------------------|-----|
| `/token` | POST | Issues access and refresh tokens (grant_type: password, refresh_token, id_token, pkce, web3) | auth |
| `/logout` | POST | Logs out a user | auth |
| `/verify` | GET, POST | Authenticate by verifying one-time token (signup, invite, recovery, magiclink, email_change, sms, phone_change) | auth |
| `/authorize` | GET | Redirects to external OAuth provider | oauth-client |
| `/signup` | POST | Signs a user up (email/phone + password, optional PKCE) | auth |
| `/recover` | POST | Request password recovery (sends email) | auth |
| `/resend` | POST | Resends OTP (signup, email_change, sms, phone_change) | auth |
| `/magiclink` | POST | Send magic link to email | auth |
| `/otp` | POST | Send OTP over email or SMS | auth |
| `/user` | GET, PUT | Fetch / update current user (email, phone, password, data, app_metadata, user_metadata) | user |
| `/user/identities/authorize` | GET | Link OAuth identity (redirect to provider) | user |
| `/user/identities/{identityId}` | DELETE | Unlink identity | user |
| `/user/oauth/grants` | GET, DELETE | List / revoke OAuth grants (when OAuth server enabled) | user, oauth-server |
| `/reauthenticate` | POST | Send OTP for password change (email/phone confirmation) | user |
| `/factors` | POST | Enroll MFA factor (totp, phone, webauthn) | user |
| `/factors/{factorId}/challenge` | POST | Create MFA challenge | user |
| `/factors/{factorId}/verify` | POST | Verify MFA factor | user |
| `/factors/{factorId}` | DELETE | Remove MFA factor | user |
| `/callback` | GET, POST | OAuth callback from external provider | — |
| `/sso` | POST | SSO (SAML) sign-in | sso |
| `/sso/saml/metadata` | GET | SAML metadata | saml |
| `/sso/saml/acs` | POST | SAML assertion consumer | saml |
| `/invite` | POST | Invite user by email (admin) | admin |
| `/admin/generate_link` | POST | Generate magiclink/signup/recovery/email_change link | admin |
| `/admin/audit` | GET | Fetch audit log events | admin |
| `/admin/users` | GET, POST | List users, create user | admin |
| `/admin/users/{userId}` | GET, PUT, DELETE | Get, update, delete user | admin |
| `/admin/users/{userId}/factors` | GET | List user factors | admin |
| `/admin/users/{userId}/factors/{factorId}` | DELETE, PUT | Delete/update factor | admin |
| `/admin/sso/providers` | GET, POST | List/create SSO providers | admin |
| `/admin/sso/providers/{ssoProviderId}` | GET, PUT, DELETE | Get/update/delete SSO provider | admin |
| `/admin/oauth/clients` | POST, GET | Register/list OAuth clients (when OAuth server enabled) | admin |
| `/admin/oauth/clients/{client_id}` | GET, PUT, DELETE | Get/update/delete OAuth client | admin |
| `/admin/oauth/clients/{client_id}/regenerate_secret` | POST | Regenerate client secret | admin |
| `/oauth/clients/register` | POST | Dynamic client registration (public, rate limited) | oauth-server |
| `/oauth/token` | POST | OAuth token endpoint (client auth) | oauth-server |
| `/oauth/authorize` | GET | OAuth 2.1 authorize (Supabase as OAuth provider) | oauth-server |
| `/oauth/authorizations/{authorization_id}` | GET | Get authorization | oauth-server |
| `/oauth/authorizations/{authorization_id}/consent` | POST | Consent | oauth-server |
| `/health` | GET | Health check | general |
| `/settings` | GET | Public server settings (disable_signup, mailer_autoconfirm, phone_autoconfirm, sms_provider, saml_enabled, external providers) | general |
| `/.well-known/jwks.json` | GET | JWKS (not in openapi paths; in router) | — |
| `/.well-known/openid-configuration` | GET | OIDC discovery | — |

### 1.2 UserSchema (GoTrue user object)

From `openapi.yaml` components/schemas/UserSchema:

| Field | Type | Note |
|-------|------|------|
| id | uuid | User ID |
| aud | string | (deprecated) |
| role | string | |
| email | string | Primary contact email |
| email_confirmed_at | date-time | Verification status (email) |
| phone | string | Primary contact phone |
| phone_confirmed_at | date-time | Verification status (phone) |
| confirmation_sent_at, confirmed_at | date-time | |
| recovery_sent_at, new_email, email_change_sent_at | date-time | |
| new_phone, phone_change_sent_at, reauthentication_sent_at | date-time | |
| last_sign_in_at | date-time | |
| app_metadata | object | Server-set metadata |
| user_metadata | object | Client-set metadata (arbitrary JSON) |
| factors | array | MFA factors |
| identities | array | Linked identities |
| banned_until, created_at, updated_at | date-time / object | |

So **verification status** (email/phone confirmed) and **“users me”** (current user + update) are **provided by GoTrue**; no separate “verification status” endpoint — it is part of GET `/user`.

---

## 2. What GoTrue Does Not Provide

From the same codebase and spec:

| Capability | In GoTrue? | Note |
|------------|------------|------|
| **User-scoped API keys CRUD** | No | GoTrue has `apikey` header (server/project key) and OAuth clients (admin). It does **not** have “create/list/revoke my API keys” for end-users (e.g. for programmatic access to your app). That is product-specific. |
| **Structured user preferences** | No | GoTrue has `user_metadata` (arbitrary JSON). There is no standard schema or endpoint for “preferences” (theme, timezone, currency, risk-mode, layout). Products can store these in `user_metadata` but the **schema and API** (e.g. GET/PUT `/api/identity/preferences`) are product-specific. |
| **First-class human_name_id / email_address_id** | No | GoTrue has `id`, `email`, `phone`. No separate “human” or “email_address” entities. Domain model is product-specific. |
| **Portal types (e.g. trader vs platform)** | No | No built-in “portal” or “tenant” type. Products use `app_metadata` or `role` and enforce in their own logic. |
| **Dual OTP orchestration** | No | GoTrue has OTP (`/otp`, `/verify`) and MFA (`/factors`, challenge, verify). The **orchestration** of “require both email OTP and phone OTP” (or “GitHub + phone”) is not a single endpoint; it is a product flow. |
| **Server-side session store (e.g. Redis)** | No | GoTrue is stateless (JWT + refresh). Session storage is typically added by the app (e.g. IDAM) in front of GoTrue. |

---

## 3. IDAM Core vs Extension: Informed Split

Using the **actual** GoTrue API:

### 3.1 IDAM Core = GoTrue proxy + optional session/Redis

Everything in the table below is **provided by GoTrue**. The IDAM core is a **proxy** (and optionally session/Redis) in front of GoTrue; it does **not** add new auth primitives, it exposes the same capabilities under the IDAM contract (e.g. under `/api/identity/auth/*` or your chosen path prefix).

| IDAM core capability | GoTrue path(s) | Note |
|----------------------|----------------|------|
| Login (email/password) | POST `/token` (grant_type=password) | Core |
| Login (refresh) | POST `/token` (grant_type=refresh_token) | Core |
| Logout | POST `/logout` | Core |
| Signup | POST `/signup` | Core |
| Password recovery | POST `/recover` | Core |
| Resend OTP | POST `/resend` | Core |
| Magic link | POST `/magiclink` | Core |
| OTP (email/SMS) | POST `/otp` | Core |
| Verify (token) | GET/POST `/verify` | Core (signup, recovery, magiclink, email_change, sms, phone_change) |
| OAuth redirect | GET `/authorize` | Core |
| OAuth callback | GET/POST `/callback` | Core |
| **Users me** | GET `/user` | Core |
| **Update user** (email, phone, password, data, app_metadata, user_metadata) | PUT `/user` | Core; includes “update password” (with reauth flow) |
| **Verification status** | Part of GET `/user` (email_confirmed_at, phone_confirmed_at) | Core; no separate endpoint in GoTrue |
| Reauthenticate (for password change) | POST `/reauthenticate` | Core |
| Link/unlink identity | GET `/user/identities/authorize`, DELETE `/user/identities/{id}` | Core |
| MFA: enroll, challenge, verify, unenroll | POST `/factors`, POST `/factors/{id}/challenge`, POST `/factors/{id}/verify`, DELETE `/factors/{id}` | Core |
| SSO/SAML | POST `/sso`, GET `/sso/saml/metadata`, POST `/sso/saml/acs` | Core (if enabled) |
| Public settings | GET `/settings` | Core (for UI: disable_signup, providers, etc.) |
| JWKS / OIDC discovery | `/.well-known/jwks.json`, `/.well-known/openid-configuration` | Core |
| Health | GET `/health` | Core |
| Admin: invite, generate_link, audit, users CRUD, factors, SSO providers, OAuth clients | `/invite`, `/admin/*` | Core (admin JWT); optional in minimal core |

So the following from the earlier “generic” list are **in GoTrue and therefore core**: email/password login, OAuth, signup, recover, resend, magiclink, otp, verify, “users me”, verification status (from User), update password (PUT user + reauthenticate), reauthenticate, MFA, identity link/unlink, public settings, JWKS, health.

### 3.2 IDAM Extension = Not in GoTrue or product-specific

| IDAM extension capability | In GoTrue? | Recommendation |
|---------------------------|------------|----------------|
| **API keys CRUD** (user-scoped “my API keys” for app access) | No | **Extension.** Product defines schema (scopes, quotas, expiry). |
| **Preferences** (theme, timezone, currency, risk-mode, layout) | No standard schema or endpoint; only user_metadata | **Extension.** Schema and GET/PUT preferences endpoint are product-specific; storage can be user_metadata in GoTrue. |
| **human_name_id / email_address_id** (first-class entities) | No | **Extension.** Domain model. |
| **Portal types** (trader vs platform) | No | **Extension** (or app_metadata/role; enforcement in product). |
| **Dual OTP orchestration** (e.g. email+phone, GitHub+phone) | No single API | **Extension.** Compose GoTrue OTP + MFA + custom flow. |
| **Server-side session (Redis)** | No | **Core** if IDAM adds it (same for all products using IDAM); not “extension”. |

---

## 4. Revised Core vs Extension Table

| Item | Earlier classification | After GoTrue analysis | Place in IDAM |
|------|-------------------------|------------------------|----------------|
| Supabase GoTrue proxy | Generic (core) | GoTrue provides all auth/user/verify/MFA/SSO APIs | **Core** (proxy only) |
| Session / Redis | Generic (core) | Not in GoTrue; IDAM adds it | **Core** (IDAM infra) |
| OAuth providers | Generic (core) | GoTrue: `/authorize`, `/callback`, `/token` (pkce, id_token) | **Core** |
| Token / JWKS | Generic (core) | GoTrue: `/token`, `/.well-known/jwks.json` | **Core** |
| Email/password login | Generic (core) | GoTrue: POST `/token` (password), POST `/signup` | **Core** |
| Email/phone verification | Generic (core) | GoTrue: `/otp`, `/verify`, `/resend`; User has email_confirmed_at, phone_confirmed_at | **Core** |
| “Users me” and verification status | Generic (core) | GoTrue: GET `/user` (includes verification timestamps) | **Core** |
| Update password | Generic (core) | GoTrue: PUT `/user` (password) + POST `/reauthenticate` | **Core** |
| MFA (factors) | — | GoTrue: `/factors`, challenge, verify, delete | **Core** |
| Reauthenticate | — | GoTrue: POST `/reauthenticate` | **Core** |
| Identity link/unlink | — | GoTrue: `/user/identities/*` | **Core** |
| Public settings | — | GoTrue: GET `/settings` | **Core** |
| API keys CRUD | Product-specific (extension) | **Not in GoTrue** | **Extension** |
| Preferences (theme, timezone, risk-mode, etc.) | Product-specific (extension) | **Not in GoTrue** (only user_metadata) | **Extension** |
| human_name_id / email_address_id | Product-specific (extension) | **Not in GoTrue** | **Extension** |
| Portal types (trader vs platform) | Product-specific (extension) | **Not in GoTrue** | **Extension** |
| Dual OTP flows | Product-specific (extension) | **Not a single GoTrue API** | **Extension** |

---

## 5. Recommendation

1. **IDAM core** should **mirror GoTrue’s API surface** (under your path prefix and with optional session/Redis). That is: token, logout, signup, recover, resend, magiclink, otp, verify, authorize, callback, GET/PUT user, reauthenticate, factors, user/identities, sso/saml, GET settings, JWKS, health, and optionally admin. No need to invent “verification status” as a separate endpoint — it is part of GET `/user` (email_confirmed_at, phone_confirmed_at).

2. **IDAM extension** should contain only what **GoTrue does not provide**: user-scoped **API keys CRUD**, structured **preferences** (schema + endpoint), **human_name_id/email_address_id** if the product needs them, **portal types**, and **dual OTP orchestration** (product flow composing GoTrue primitives).

3. **Reference OpenAPI for IDAM core:** Derive the core spec from GoTrue’s `openapi.yaml` (path prefix + optional renames), or maintain a minimal “IDAM core” spec that lists the same operations. Do not put API keys, preferences schema, or domain entities into the core spec; those stay in the customer extension spec.

4. **Update [IDAM Microscaler Analysis](IDAM_MICROSCALER_ANALYSIS.md)** and **[IDAM Design: Core and Extension](IDAM_DESIGN_CORE_AND_EXTENSION.md)** so that “core” is explicitly “GoTrue proxy + session/Redis” and “extension” is “API keys, preferences, domain model, portal types, dual OTP flows” as above.

---

## 6. Document Status

- **Scope:** Analysis only; no code changes. Informs IDAM core vs extension and reference spec content.
- **Next step:** (1) Update IDAM_MICROSCALER_ANALYSIS.md §2 with “core = GoTrue proxy + session; extension = API keys, preferences, domain, portal, dual OTP”. (2) Optionally add a reference IDAM core OpenAPI (from GoTrue spec + path prefix). (3) Keep IDAM_DESIGN_CORE_AND_EXTENSION.md path conventions; add pointer to this document for API-level mapping.
- **References:** Supabase Auth `openapi.yaml`, `internal/api/api.go`, `internal/api/router.go`; [IDAM Microscaler Analysis](IDAM_MICROSCALER_ANALYSIS.md), [IDAM Design: Core and Extension](IDAM_DESIGN_CORE_AND_EXTENSION.md).
