# Seasame-IDAM: Audit and Transformation Analysis

**Status:** Draft  
**Last Updated:** 2025-02-02  
**Scope:** Analyse `../seasame-idam` against IDAM goals derived from `./docs/SPIFFY_mTLS`; preserve pre-pivot state; then transform into a microservice IDAM component.

This document provides a **comprehensive audit** of the seasame-idam repository, positions it relative to our overall IDAM goals (including SPIFFY/mTLS, service-account auth, and user auth/authZ), and outlines how to transform it into an **IDAM component for microservices** (RERP, PriceWhisperer, BRRTRouter-backed services) — not a standalone SaaS IDAM product.

---

## 0. Pivot: From SaaS IDAM to Microservice IDAM Component

**Decision:** Seasame was originally conceived as a **SaaS IDAM solution**. The market is well served by such systems. We are **pivoting** seasame to an **IDAM component** that fits inside our microservice architecture (Identity + Access Management as defined in `./docs/SPIFFY_mTLS`).

**Implications:**

- **Current DB work can be torn out.** The existing schema and Sea-ORM entities (org-scoped RBAC, single-DB “SaaS” model) do not align with the target Identity + AM split and tenant/org model. They may be removed and replaced with schemas driven by `identity-openapi.yaml`, `access-management-openapi.yaml`, and the Lifeguard ERDs in `IDAM_OpenAPI_and_Integration.md`.
- **Pre-pivot state is preserved** so we can reference or restore the old design if needed.

**Preservation (done):**

| What | Where |
|------|--------|
| **Archive branch** | `archive/saas-idam-pre-microservice-pivot` — full repo state before the pivot. Pushed to `origin`. |
| **Archive tag** | `archive/saas-idam-2025-02-02` — same state as a tag; message: *"Archive: SaaS IDAM state before pivot to microservice IDAM component."* |
| **Archive repo** | `git@github.com:microscaler/sesame-idam-archived.git` — full SaaS IDAM state pushed to a dedicated read-only archive. Mark as **Archived** in GitHub Settings if desired. |

**Next steps (sledge hammer):** On `main`, remove or replace current DB migrations and entities, and align the codebase with the Identity + AM component design (see §5 and §6 below). The audit in §1–4 still describes the gap; §5–6 describe the target and transformation.

**What was torn out (done):**

- **All Rust code removed.** The entire `sesame/` crate (including any remaining `src/`, migrations, Cargo.toml) and `rustfmt.toml` have been **deleted**. The repo no longer contains any Rust implementation; it is a layout-only repo until microservices are implemented.
- **Repo aligned with RERP microservices layout.** The repository now follows the same pattern as RERP: a **microservices/** directory with an **idam** domain and two (initially) microservices: **authentication** and **authorization**. Further IDAM components may be added under `idam/` as identified.

**Current layout (RERP-style):**

- **Root `Cargo.toml`:** Workspace with `members = ["microservices"]`.
- **microservices/Cargo.toml:** Workspace with no members yet; commented placeholders for `idam/authentication/gen`, `idam/authentication/impl`, `idam/authorization/gen`, `idam/authorization/impl`.
- **microservices/idam/:** IDAM domain. Two microservices:
  - **authentication** — Identity, login, refresh, logout, token exchange, register, sessions, JWKS/OIDC (aligns with Identity Service / `identity-openapi.yaml`).
  - **authorization** — Access Management: apps, roles, permissions, principal/effective, authorize (aligns with AM Service / `access-management-openapi.yaml`).
- **openapi/idam/:** OpenAPI spec locations; canonical sources are `./openapi/identity-openapi.yaml` and `./openapi/access-management-openapi.yaml` in this (BRRTRouter) repo.
- **AGENTS.md** and **README.md** in seasame-idam describe the layout and point to this audit. When implementing, add `gen/` + `impl/` per microservice (BRRTRouter codegen + lifeguard) and register them in `microservices/Cargo.toml`.

---

## 1. Executive Summary

| Aspect | Finding |
|--------|--------|
| **Seasame-idam today** | Early-stage IDAM: rich OpenAPI (auth, RBAC, orgs, API keys, MFA, SAML, SCIM), Sea-ORM entities and migrations aligned to that spec, but **no request routing or AM integration**; development was **blocked** on BRRTRouter (dynamic dispatch), lifeguard (pooling), and photon. |
| **BRRTRouter since** | BRRTRouter has gained **SPIFFE JWT SVID validation** (tests in `tests/spiffe_tests.rs`), and the SPIFFY_mTLS design has crystallised **Identity + Access Management** as separate services with tenant/org model, JWT enrichment from AM, and dot-notation namespacing. |
| **Three-pillar split** | Our target architecture splits: **(1)** Interservice SPIFFY/mTLS authentication; **(2)** Service-account authentication and authorization to obtain SPIFFE credentials; **(3)** User authentication and authorization. Seasame-idam currently addresses only **(3)** in design and partially in spec; **(1)** and **(2)** are absent. |
| **Transformation direction** | Align seasame-idam with the **Identity Service** from `Generic_Identity_Service_IDAM_Design.md` and `identity-openapi.yaml`; introduce or integrate a separate **Access Management** service; add **tenant/organisation** model and **no-PII-in-URIs**; leave **SPIFFE credential issuance** to infrastructure (SPIRE/cert-manager) and use Identity only for **user and service-account tokens** (OAuth 2.1, Client Credentials, Token Exchange). |

---

## 2. Knowledge Split: The Three Pillars

From the SPIFFY_mTLS docs, our IDAM-related work splits into three pillars.

### 2.1 Pillar 1: Interservice SPIFFY/mTLS Authentication

**What it is:** Service-to-service authentication using **SPIFFE identities** and **X.509 SVIDs** (or JWT SVIDs where mTLS is not feasible). Trust domain (e.g. `spiffe://rerp.prod`), workload attestation (e.g. K8s SA), cert-manager + CSI driver or SPIRE, trust bundle distribution, BRRTRouter validating client SPIFFE IDs on mTLS connections.

**Where it lives:**  
- Design: `04_Design Plan_ SPIFFE-Based mTLS for BRRTRouter Services.md`, `SPIFFE_SPIRE Mutual TLS Architecture for BRRTRouter Services.md`, PRD EPIC 3 (mTLS infra), EPIC 4 (BRRTRouter mTLS).  
- Implementation: BRRTRouter has **SPIFFE JWT SVID** validation (trust domain, audience, signature via JWKS); X.509 SVID handling and trust-bundle loading are in the design/plan phase.

**Seasame-idam relevance:** **None.** Interservice mTLS is an infrastructure and gateway concern. Identity/AM services *consume* mTLS (they are called over mTLS by the gateway and other services) but do not *issue* SPIFFE SVIDs; that is the role of SPIRE or cert-manager + CSI.

---

### 2.2 Pillar 2: Service-Account Authentication and Authorization to Get SPIFFE Credentials

**What it is:** **Before** a workload can use SPIFFE (get an X.509 SVID from the Workload API or from the CSI driver), the **platform** must authenticate/authorize the workload. In Option 1 (SPIRE), the SPIRE Agent attests the workload (e.g. K8s service account); in Option 2 (cert-manager + CSI), the CSI driver uses the pod’s service account to request a cert. So “service-account auth to get SPIFFE credentials” is largely **K8s (or platform) identity** — not the same as “user logs in to get a JWT.”  
Separately, **application-level** service-to-service calls (e.g. “Invoice Service calling AM”) may use **OAuth 2.1 Client Credentials** or **RFC 8693 Token Exchange** to obtain **JWT access tokens** (not SPIFFE SVIDs). Those tokens are issued by the **Identity Service** (or STS). So:

- **SPIFFE credentials (X.509 SVID):** Obtained via SPIRE/cert-manager; platform attests workload; no IdP in the loop.
- **Service JWT (Client Credentials / Token Exchange):** Issued by Identity; used for app-level “call AM” or “call Invoice API”; Identity must support **Client Credentials** and optionally **Token Exchange** for service principals.

**Where it lives:**  
- Design: `02_High-Security Multi-Tenant Auth & AuthZ Architecture-Part2.md` (Client Credentials, Token Exchange, audience-scoped tokens, `act` claim), PRD stories 1.3 (Client Credentials), 1.5 (Token Exchange).  
- OpenAPI: `identity-openapi.yaml` should (or will) define client credentials and token-exchange endpoints.

**Seasame-idam relevance:** **Partial.** Seasame’s OpenAPI has **API Key** login (exchange API key for JWT) and an **OAuth2 token** endpoint with `grant_type: authorization_code | refresh_token` only — **no `client_credentials` or `urn:ietf:params:oauth:grant-type:token-exchange`**. So seasame does not yet model **service principals** or **Client Credentials / Token Exchange**. Transformation: add these grants and a clear notion of “service” vs “user” principals in Identity.

---

### 2.3 Pillar 3: User Authentication and Authorization

**What it is:**  
- **Authentication:** Users log in (password, OIDC, passkey, etc.); Identity issues **JWTs** (access + refresh); sessions in Redis; tenant and organisation in token and DB.  
- **Authorization:** What the user may do — roles, permissions, attributes. In our design this is delegated to the **Access Management (AM)** service: Identity calls AM at login to **enrich** the JWT with roles/permissions, or services call AM for per-request **authorize** checks.

**Where it lives:**  
- Design: `Generic_Identity_Service_IDAM_Design.md`, `Generic_Access_Management_Service_Design.md`, `IDAM_OpenAPI_and_Integration.md`.  
- OpenAPI: `identity-openapi.yaml` (auth, identity, discovery), `access-management-openapi.yaml` (apps, roles, permissions, principal/effective, authorize).  
- PRD: EPIC 1 (Identity & Auth), EPIC 1.6 (RBAC/ABAC/ACL and AM).

**Seasame-idam relevance:** **High.** Seasame’s spec and schema cover **user** login (email/password, OIDC, passkey, API key), sessions, organisations, **RBAC** (roles, permissions, role_permissions, user_roles, role_inheritance), MFA, SAML, SCIM, audit, impersonation. So it overlaps heavily with **user auth** and with **RBAC**, but:

- **Tenant/organisation model:** Seasame has **organisations** and **user_organization_info**; it does **not** have an explicit **tenant** entity or **tenant_id** in tokens. Our target model has **organisations** (legal entity) and **tenants** (e.g. divisions under an org) with clear demarcation; JWT and AM use `organization_id` and `tenant_id`.
- **Identity vs AM:** Seasame embeds RBAC **inside** the same service (roles/permissions in same DB as users). Our target **splits** Identity (who you are, issue JWT) and AM (what you can do, register apps/roles, evaluate permissions). So seasame’s RBAC would need to move to an AM service or be mirrored there, with Identity calling AM for JWT enrichment.
- **No PII in URIs:** Target Identity uses POST + body for email/phone lookups. Seasame’s OpenAPI uses path params in places (e.g. `/users/{id}`); any email-in-path patterns must be removed.
- **JWKS / OIDC discovery:** Required for JWT verification by gateway and services; should be explicit in Identity API (e.g. `/.well-known/jwks.json`, `/.well-known/openid-configuration`). Seasame spec does not clearly surface these.
- **Refresh / logout:** Target has explicit refresh and logout; seasame has logout and session management — align path and behaviour with target.

---

## 3. Current State of Seasame-IDAM

### 3.1 Repository Structure (excluding target/)

| Area | Content |
|------|--------|
| **App** | `sesame/` — Rust binary using `may_minihttp`; currently a trivial “Hello World” server; no route dispatch or DB wiring in `main.rs`. |
| **Entities** | `sesame/src/entity/` — Sea-ORM entities for: users, organizations, user_organization_info, roles, permissions, role_permissions, user_roles, role_inheritance, sessions, password_reset_tokens, email_verification_tokens, login_attempts, api_keys, api_key_rate_limits, mfa_devices, identity_providers, scim_*, org_login_methods, audit_logs, impersonation_logs, metrics_events, rate_limit_*. |
| **Modules** | `api_keys.rs`, `mfa.rs`, `oauth2.rs`, `org.rs`, `saml.rs`, `user.rs` — domain logic stubs or placeholders; not wired to HTTP or DB in main. |
| **Schema** | `sesame/migrations/001__initial_schema.sql` — one migration; matches entities (organizations, users, user_organization_info, roles, permissions, role_permissions, user_roles, role_inheritance, sessions, tokens, login_attempts, waitlist_signups, api_keys, mfa_devices, identity_providers, scim_*, org_login_methods, audit_logs, impersonation_logs, metrics_events, rate_limit_*). **No tenants table;** org is the top-level scope. |
| **API spec** | `specs/openapi.yaml` — OpenAPI 3.0; auth (login, logout, MFA, password-reset), organizations, users, roles, permissions, RBAC, API keys, SAML, SCIM, audit, metrics. Paths use `/users/{id}`, `/organizations/{id}` (no PII in path for email). OAuth2 token endpoint supports only `authorization_code` and `refresh_token`. |
| **Clients** | `clients/` — placeholders (js, js-wasm, python3, rust) with `.gitkeep`. |
| **Docs** | `docs/pdfs/` — e.g. “BRRTRouter as a Unified Proxy Gateway with AuthZ for Microservices” (gateway + Sesame-IDAM authZ), “Enterprise Authorization: Roles, Claims, and Access Control Schemes”. |
| **Blockers (from README)** | “Robust dynamic dispatch request routing — pending BRRTRouter”; “No Database connection pooling library — pending lifeguard”; “No easy to use wrapper — pending photon.” |

### 3.2 OpenAPI vs Target Identity/AM

| Dimension | Seasame `specs/openapi.yaml` | Target `identity-openapi.yaml` | Target `access-management-openapi.yaml` |
|-----------|-----------------------------|--------------------------------|----------------------------------------|
| **Auth** | Login (email/password, OIDC, passkey, api_key), logout, MFA, password-reset | Login, refresh, logout, token (exchange), register | N/A (Identity authenticates; AM authorizes) |
| **Identity** | Users, orgs; user_organization_info | Organizations, tenants, email/phone lookup (POST body), users/me, profile | N/A |
| **Tenant** | Not first-class (org only) | organization_id, tenant_id in JWT and APIs | organization_id, tenant_id for principal |
| **RBAC** | Roles, permissions, user_roles, role_inheritance (per org) | N/A | Applications, roles, permissions, role_permissions, principal_roles, principal_attributes |
| **Scoping** | Org-scoped roles/permissions | Tenant-under-org | App-slug (dot-notation) + tenant/org |
| **JWT enrichment** | Not specified | Identity calls AM `POST /api/v1/am/principal/effective` | AM returns roles/permissions for JWT |
| **Authorize** | Not a dedicated endpoint | N/A | `POST /api/v1/am/authorize` |
| **Discovery** | Not explicit | OIDC, JWKS | N/A |
| **Service auth** | API key → JWT only | Client Credentials, Token Exchange (RFC 8693) | N/A |
| **PII in URIs** | Some path params (e.g. user id) | No email/phone in path; POST body for lookups | No PII in path |

### 3.3 Database: Seasame vs Target

- **Seasame:** Single schema with users, organizations, user_organization_info, roles, permissions, role_permissions, user_roles, role_inheritance, sessions, api_keys, mfa, identity_providers, scim_*, audit, metrics, rate_limit_*. No `tenants` table; no separate “AM” schema.
- **Target Identity (from IDAM_OpenAPI_and_Integration.md):** organizations, tenants (under org), users, sessions, etc.; tenant_id and organization_id in relevant tables.
- **Target AM:** applications, roles, permissions, role_permissions, principal_roles, principal_attributes (with organization_id), optional policies. Separate AM DB or schema.

Seasame’s schema is a good base for **Identity** (with the addition of **tenants** and tenant_id/organization_id where needed) but currently mixes in **RBAC** that the target design assigns to **AM**. So either: (a) migrate seasame’s RBAC into a separate AM service and DB, or (b) keep a single deployment but split logical “Identity” vs “AM” APIs and data (e.g. AM tables in same DB, separate service later).

---

## 4. Gap Analysis: Seasame vs Target (by Pillar)

### 4.1 Pillar 1 (Interservice SPIFFY/mTLS)

| Gap | Severity | Note |
|-----|----------|------|
| Seasame does not implement mTLS or SPIFFE | N/A | Not in scope for Identity/AM; handled by BRRTRouter and cert-manager/SPIRE. |
| Seasame will be **called over** mTLS | Info | When deployed, gateway and services will call Identity/AM over mTLS; no change required in seasame for “consuming” mTLS. |

### 4.2 Pillar 2 (Service-Account Auth for Tokens)

| Gap | Severity | Note |
|-----|----------|------|
| No Client Credentials grant | High | Target: service principals get JWT via client_credentials; audience-scoped. Add to OpenAPI and implementation. |
| No Token Exchange (RFC 8693) | High | Target: exchange user token for down-scoped token for another audience; act claim. Seasame has “OAuth2 token” with only authorization_code/refresh_token. Add grant_type token-exchange and implement. |
| API key → JWT is present | OK | Covers “machine” login to get a JWT; align with “service” principal and audience/scope. |
| No explicit “service” vs “user” principal type in JWT | Medium | Target JWT has aud, scope, act for delegation; service tokens have no user sub (or act only). Define in spec and tokens. |

### 4.3 Pillar 3 (User Auth and AuthZ)

| Gap | Severity | Note |
|-----|----------|------|
| No tenant entity; org-only | High | Add tenants table and tenant_id to sessions, JWTs, and APIs; document org → tenant relationship. |
| RBAC inside Identity | High | Target: AM service owns roles/permissions/assignments; Identity calls AM for JWT enrichment. Either split AM out (new service + DB) or add AM API and tables to seasame and have “Identity” API call “AM” module for effective roles/permissions. |
| No AM-style registration (apps, dot-notation slugs) | High | Target: consuming apps register with AM (app slug, roles, permissions); principal/effective and authorize are app-scoped. Seasame has no “applications” or app-scoped RBAC; add or integrate AM. |
| No explicit JWKS / OIDC discovery paths | Medium | Add `/.well-known/jwks.json` and `/.well-known/openid-configuration` (or equivalent) to spec and implementation. |
| PII in URIs | Medium | Audit paths: avoid email/phone in path/query; use POST body for lookups (already required in target Identity). |
| Refresh / logout | Low | Seasame has logout and session handling; ensure refresh and logout match target semantics and paths. |
| Organisation vs tenant semantics | Medium | Target: organisation = legal entity; tenant = subdivision (e.g. division/region). Align naming and docs. |

---

## 5. Positioning and Transformation Roadmap

### 5.1 Where Seasame Fits Today

- **Pillar 1 (mTLS):** Out of scope; no change.
- **Pillar 2 (service tokens):** Spec and code lack Client Credentials and Token Exchange; API key login is a partial stand-in. **Position:** Extend seasame to be the **Identity Service** that issues both user JWTs and **service JWTs** (Client Credentials + Token Exchange).
- **Pillar 3 (user auth/authZ):** Seasame is **mostly aligned** with user auth (login, sessions, orgs, RBAC) but: (1) RBAC should live in or be integrated with an **AM** service; (2) **tenant** and **organisation** model must be brought in; (3) Identity should call AM for **JWT enrichment** (roles/permissions in token).

### 5.2 Target End-State (from SPIFFY_mTLS)

- **Identity Service:** Auth (login, refresh, logout, token exchange, register), identity (email/phone lookup by body, users/me), organisations and tenants, discovery (OIDC, JWKS). Issues **user** and **service** JWTs. Calls **AM** for principal/effective at login to enrich JWT.
- **Access Management Service:** Applications (dot-notation slugs), roles, permissions, role_permissions, principal_roles, principal_attributes, principal/effective, authorize. Separate DB (or separate schema). Used by Identity (enrichment) and by services (per-request authorize).
- **BRRTRouter / Gateway:** Validates user JWTs (JWKS from Identity); validates SPIFFE (mTLS or JWT SVID); routes to Identity, AM, and backend services. Internal calls use mTLS.

### 5.3 Transformation Directions (No Code Here — Plan Only)

1. **Align Identity API with target**
   - Adopt (or merge) `identity-openapi.yaml` as the contract: paths, request/response, tenant_id and organization_id, no PII in URIs, refresh, logout, token exchange.
   - Add tenants (table + APIs) and ensure sessions and JWTs carry tenant_id and organization_id.
   - Add JWKS and OIDC discovery endpoints.

2. **Introduce AM and split RBAC**
   - Option A: New Access Management service and DB (per `access-management-openapi.yaml` and IDAM integration doc); Identity calls AM `POST /api/v1/am/principal/effective` at login and embeds roles/permissions in JWT.
   - Option B: Add AM as a **module** and **schema** inside seasame (same process, same DB or schema), implement AM API and registration model; later split to a separate service if needed. Either way, **registration model** (apps with dot-notation slugs, role/permission definitions, principal assignments) and **authorize** + **principal/effective** must match the Generic AM design.

3. **Service principals and tokens**
   - Add **Client Credentials** grant: client_id + client_secret (or equivalent) → JWT with audience and scope for a service.
   - Add **Token Exchange** (RFC 8693): subject_token (user JWT) + requested audience/scope → new JWT with optional act claim.
   - Store and manage “clients” (service principals) and their scopes/audiences; document in OpenAPI.

4. **Unblock implementation**
   - README blockers: BRRTRouter (routing) and lifeguard (pooling) have evolved; re-evaluate whether seasame can use current BRRTRouter for routing and current lifeguard (or another pooler) for DB. If so, wire sesame to real routing and DB and implement the aligned Identity (and optionally AM) APIs.

5. **Keep seasame as “Identity” and optional AM**
   - Do not implement SPIFFE SVID issuance in seasame; that remains with SPIRE/cert-manager. Seasame only issues **application-layer** JWTs (user and service) and, if integrated, performs or calls AM for authorization.

---

## 6. Recommendations

1. **Treat seasame-idam as the Identity Service candidate** — Align its OpenAPI and data model with `Generic_Identity_Service_IDAM_Design.md` and `identity-openapi.yaml`; add tenants, organization_id/tenant_id in tokens and DB; add JWKS and OIDC discovery.
2. **Add Client Credentials and Token Exchange** — Implement and spec service-principal auth (Client Credentials) and RFC 8693 Token Exchange so that gateway and services can obtain audience-scoped JWTs and down-scoped delegation tokens.
3. **Decide AM placement** — Either (a) new AM service + DB (clean split, matches IDAM_OpenAPI_and_Integration.md) or (b) AM module inside seasame with shared or separate schema, then split later. In both cases, adopt the **registration model** and **principal/effective** and **authorize** semantics from Generic_Access_Management_Service_Design.md.
4. **Migrate RBAC from “per-org” to “per-app + tenant”** — Target uses app slugs (e.g. `accounting.invoice`) and tenant/organization_id for assignments. Plan migration from seasame’s current org-scoped roles/permissions to AM’s application-scoped model (and, if keeping seasame’s RBAC temporarily, add tenant_id and app_id where needed).
5. **Revisit blockers** — Confirm BRRTRouter and lifeguard (or alternatives) are sufficient to resume implementation; wire routing and DB so that real auth and AM flows can be implemented and tested.
6. **Document the three pillars** — Keep a short doc (or section) that states: (1) mTLS/SPIFFE is infra + gateway; (2) service JWTs are Identity’s job (Client Credentials + Token Exchange); (3) user auth and authZ are Identity + AM, with JWT enrichment from AM. This avoids conflating “getting a SPIFFE cert” with “getting a service JWT.”

---

## 7. References

- `./PRD_SPIFFE_mTLS_Multi-Tenant_Security.md` — EPICs and stories (Identity, mTLS, RLS).
- `./Generic_Identity_Service_IDAM_Design.md` — Identity service shape, tenant, no PII in URIs.
- `./Generic_Access_Management_Service_Design.md` — AM registration, dot-notation, principal/effective, authorize.
- `./IDAM_OpenAPI_and_Integration.md` — Identity + AM integration, sequences, ERDs for lifeguard.
- `./openapi/identity-openapi.yaml` — Target Identity OpenAPI.
- `./openapi/access-management-openapi.yaml` — Target AM OpenAPI.
- `./04_Design Plan_ SPIFFE-Based mTLS for BRRTRouter Services.md` — cert-manager/SPIRE options.
- `./SPIFFE_SPIRE Mutual TLS Architecture for BRRTRouter Services.md` — SPIFFE primer, workload identity.
- `./02_High-Security Multi-Tenant Auth & AuthZ Architecture-Part2.md` — OAuth 2.1, Client Credentials, Token Exchange, JWT design.
- `../seasame-idam/README.md` — Current features and blockers.
- `../seasame-idam/specs/openapi.yaml` — Current Sesame API spec.
- `../seasame-idam/sesame/migrations/001__initial_schema.sql` — Current schema.
- `../seasame-idam/docs/pdfs/BRRTRouter as a Unified Proxy Gateway with AuthZ for Microservices.md` — Gateway + Sesame-IDAM vision.
