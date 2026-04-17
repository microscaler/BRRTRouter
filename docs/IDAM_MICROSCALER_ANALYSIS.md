# IDAM: General Microscaler Service vs Per-System IDAM

**Purpose:** Decide whether the Identity & Access Management (IDAM) implemented in PriceWhisperer should become a **general Microscaler service** or whether **each system building with BRRTRouter** should create its own IDAM tailored to its requirements.

**Sources analysed:**
- `../PriceWhisperer/microservices/trader/idam/doc/openapi.yaml` — Identity + auth API used for docs/serving (~930 lines).
- `../PriceWhisperer/microservices/openapi/trader/idam/openapi.yaml` — Canonical/source spec (full scope: identity, auth, API keys, preferences).

**Related:** [BFF Proxy Analysis](BFF_PROXY_ANALYSIS.md) §6 (BFF ↔ IDAM ↔ Supabase); BRRTRouter BFF flow assumes BFF calls IDAM for auth/RBAC, not Supabase directly. **Core + extension deployment and BFF usage:** [IDAM Design: Core and Extension](IDAM_DESIGN_CORE_AND_EXTENSION.md).

---

## 1. What PriceWhisperer IDAM Does (Summary)

### 1.1 Architecture (from specs and BFF analysis)

- **Frontend (SolidJS)** → **BFF** → **IDAM** → **Supabase GoTrue** → **PostgreSQL**.
- **Sessions:** Redis (optional); apps never call Supabase directly.
- IDAM wraps Supabase GoTrue; BFF and other consumers call IDAM over HTTP.

### 1.2 Scope (from both OpenAPI specs)

| Area | Capabilities |
|------|---------------|
| **Identity** | Email upsert/lookup (single source of truth for `email_address_id`); user by `human_name_id`; verification status (email + phone). |
| **Auth** | Login (email/password); dual OTP (email+phone for customer, GitHub+phone for platform); verify email/phone OTP; complete dual login; Google/GitHub/SAML OAuth; GitHub callback. |
| **Domain model** | `human_name_id`, `email_address_id`, mobile number, verification; portal types `trader` (customer) and `platform`. |
| **Extended (canonical spec)** | `/api/identity/auth/update-password`, `/api/identity/users/me`, `/api/identity/users/me/verification-status`, `/api/identity/api-keys`, `/api/identity/api-keys/{key_id}`, `/api/identity/preferences` (theme, timezone, currency, risk-mode, layout). |

---

## 2. Generic vs Product-Specific Capabilities

**Informed by GoTrue API:** The split below is aligned with the actual [Supabase GoTrue API](https://github.com/supabase/auth). See [IDAM GoTrue API Mapping](IDAM_GOTRUE_API_MAPPING.md) for the full path-by-path table and UserSchema; GoTrue provides token, signup, recover, otp, verify, user (GET/PUT), reauthenticate, factors, identity link/unlink, SSO/SAML, settings, JWKS — it does **not** provide user-scoped API keys CRUD, structured preferences, or first-class human/email_address entities.

### 2.1 Generic = IDAM core (GoTrue proxy + optional session)

| Capability | Description |
|------------|-------------|
| **Supabase GoTrue proxy** | IDAM as HTTP facade over Supabase; apps never call Supabase directly. Same pattern for any system using Supabase for auth. |
| **Session / Redis** | Optional session store (e.g. Redis) for server-side session state; independent of product branding. |
| **OAuth providers** | Google, GitHub, SAML — configuration-driven; same integration pattern across products. |
| **Token issuance / JWKS** | IDAM returns JWTs issued via GoTrue; BFF validates with JWKS from same issuer. Standard for BFF ↔ IDAM. |
| **Email/password login** | Standard flow; shared validation (e.g. email format, password rules). |
| **Email/phone verification** | Verify-by-OTP pattern; transport (email, SMS) is configurable. |
| **“Users me” and verification status** | Generic “current user” and “verification status” endpoints; schema can be minimal (e.g. `user_id`, `email_verified`, `phone_verified`). |
| **Update password** | Standard auth operation; no product-specific logic. |

These can be provided by a **shared core**: IDAM = GoTrue proxy (+ optional session/Redis); same integration points (JWKS, OAuth config) and a minimal, product-agnostic API surface derived from GoTrue.

### 2.2 Product-specific = IDAM extension (not in GoTrue or product-bound)

| Capability | Why product-specific |
|------------|----------------------|
| **`human_name_id` / `email_address_id`** | Domain model for “human” and “email address” as first-class entities; other systems may use plain `user_id` + email string. |
| **Portal types: `trader` vs `platform`** | PriceWhisperer’s split between customer (trader) and platform portals; other products may have different roles or a single portal. |
| **Dual OTP flows** | Email+phone for customer, GitHub+phone for platform — specific to PriceWhisperer’s security policy and UX. |
| **API keys CRUD** | `/api/identity/api-keys`, `/api/identity/api-keys/{key_id}` — useful generically, but key semantics, quotas, and scopes may differ per product. |
| **Preferences (theme, timezone, currency, risk-mode, layout)** | Strongly UX/product-specific; not all systems need the same schema (e.g. risk-mode is trading-specific). |
| **Branding / tenant naming** | Any product-specific strings, logos, or multi-tenant naming live in the product, not in a generic IDAM. |

These are best implemented as **per-system extensions** on top of the GoTrue-proxy core; see [IDAM GoTrue API Mapping](IDAM_GOTRUE_API_MAPPING.md) §3–§4.

---

## 3. Options

### Option A: General Microscaler IDAM (shared service or shared core)

- **Idea:** Extract a single “Microscaler IDAM” (service or core library) that provides the generic capabilities (§2.1). Products either use the shared service or embed the core and add their own routes/config.
- **Pros:** One place to maintain Supabase/GoTrue proxy, session/Redis, OAuth, JWKS/token contract; BFF and BRRTRouter docs can assume “IDAM” with a stable, minimal API.
- **Cons:** Product-specific needs (dual OTP, portal types, API keys semantics, preferences schema) either require extension points and config, or remain in product-owned code; risk of the “general” service growing with every product’s quirks.

### Option B: Per-system IDAM (each system builds its own)

- **Idea:** Each system (PriceWhisperer, RERP, etc.) builds its own IDAM service tailored to its requirements. No shared IDAM service; optionally a **reference implementation** or **template** (e.g. derived from PriceWhisperer) that others copy and adapt.
- **Pros:** Maximum flexibility; no cross-product coupling; each IDAM stays aligned to one product’s domain model and UX.
- **Cons:** Duplication of Supabase proxy, session, OAuth, and token/JWKS integration; BFF/BRRTRouter docs must describe the **contract** (e.g. “BFF calls IDAM for auth; IDAM uses Supabase”) rather than a single implementation.

---

## 4. Recommendation

**Recommendation: shared core + per-system IDAM (hybrid).**

- **Do not** turn PriceWhisperer IDAM *as-is* into a single, general Microscaler service that every system must use. The product-specific parts (§2.2) would either bloat the shared service or require heavy extension machinery.
- **Do** treat the **architecture and generic capabilities** as the “Microscaler IDAM pattern”:
  - **BFF ↔ IDAM ↔ Supabase** (apps never call Supabase directly).
  - IDAM provides: GoTrue proxy, optional session/Redis, OAuth (config-driven), token/JWKS contract for BFF.
  - Minimal generic API: login (email/password), OAuth flows, “users me”, verification status, update password, and optionally a minimal “introspect/claims” for BFF enrichment (see [BFF Proxy Analysis](BFF_PROXY_ANALYSIS.md) §5.4, §6).
- **Deliverables:**
  1. **Document the contract:** In BRRTRouter (or Microscaler) docs, describe the **IDAM contract** expected by the BFF: endpoints and behaviour needed for auth, RBAC/claims, and JWKS. This keeps BFF/BRRTRouter agnostic of which IDAM implementation each system uses.
  2. **Shared core or template (optional):** If useful, extract a **small “IDAM core” or template** (e.g. from PriceWhisperer): Supabase client, session/Redis, OAuth wiring, and a minimal OpenAPI surface. Each system **clones or depends on** this core and adds its own routes (API keys, preferences, dual OTP, portal types, etc.). This reduces duplication without forcing one shared service.
  3. **PriceWhisperer:** Keeps its current IDAM as the product-specific implementation (including `human_name_id`, trader/platform, dual OTP, API keys, preferences); refactor later to use the shared core/template if and when it exists.

So: **each system may build its own IDAM specific to its requirements**, but the **pattern and, optionally, a reusable core/template** are general Microscaler concerns. The BFF and BRRTRouter depend on the **IDAM contract**, not on a single shared IDAM service.

---

## 5. Relationship to BFF Proxy Analysis

- **§6 BFF ↔ IDAM ↔ Supabase:** BFF never talks to Supabase directly; BFF calls IDAM for auth and RBAC. This document does not change that; it only clarifies that “IDAM” can be a per-system implementation that follows the same contract.
- **Custom claims from IDAM (§5.4, G4, G7, G8):** If the IDAM contract includes an endpoint for “get claims for this token” (or roles/permissions), BRRTRouter can add a claims-enrichment step that calls that endpoint. The contract is generic; the implementation of that endpoint is per-system (e.g. Supabase Auth Hooks vs IDAM-owned DB).

For the full list of BFF/IDAM gaps and options, see [BFF Proxy Analysis](BFF_PROXY_ANALYSIS.md) §5.4, §6, and §8.

---

## 6. Document Status

- **Scope:** Analysis and recommendation only; no code changes.
- **Next step:** Review this recommendation; then (1) add the “IDAM contract” to BRRTRouter/Microscaler docs, and (2) decide whether to extract a shared IDAM core/template from PriceWhisperer. For how core + extension compose (one vs two services) and how BFF uses IDAM, see [IDAM Design: Core and Extension](IDAM_DESIGN_CORE_AND_EXTENSION.md).
- **References:** PriceWhisperer IDAM OpenAPI specs (doc + canonical), [BFF Proxy Analysis](BFF_PROXY_ANALYSIS.md) §5.4, §6, §8, [IDAM Design: Core and Extension](IDAM_DESIGN_CORE_AND_EXTENSION.md), [IDAM GoTrue API Mapping](IDAM_GOTRUE_API_MAPPING.md) (GoTrue paths → IDAM core/extension).
