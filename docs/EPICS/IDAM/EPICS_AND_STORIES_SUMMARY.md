# IDAM Epics and Stories — Summary

**Source:** `docs/IDAM_MICROSCALER_ANALYSIS.md`, `docs/IDAM_GOTRUE_API_MAPPING.md`, `docs/IDAM_DESIGN_CORE_AND_EXTENSION.md`

**Purpose:** Single reference for Epic/Story docs under `docs/EPICS/IDAM/` and for creating/linking GitHub issues. IDAM epics are numbered 6–9 (after BFF_PROXY 1–5). See [EPICS_CATALOG.md](../EPICS_CATALOG.md).

---

## Epic 6 — IDAM contract and reference spec

**GitHub issue:** [#278](https://github.com/microscaler/BRRTRouter/issues/278)

**Scope:** Document the IDAM contract expected by the BFF (endpoints, behaviour, JWKS); produce a reference IDAM core OpenAPI derived from GoTrue (path prefix, minimal surface).

| ID   | Story | One-line description | Parent | Type | Labels | GitHub issue |
|------|--------|----------------------|--------|------|--------|--------------|
| 6.1  | Document IDAM contract | Add IDAM contract to BRRTRouter/Microscaler docs: endpoints and behaviour needed for auth, RBAC/claims, JWKS; BFF never calls Supabase directly. | Epic #278 | story | idam, story | [#282](https://github.com/microscaler/BRRTRouter/issues/282) |
| 6.2  | Reference IDAM core OpenAPI | Derive reference `idam-core.openapi.yaml` from GoTrue spec (path prefix, e.g. `/api/identity/auth/*`); document path layout for core vs extension. | Epic #278 | story | idam, story | [#283](https://github.com/microscaler/BRRTRouter/issues/283) |

---

## Epic 7 — IDAM core implementation (GoTrue proxy)

**GitHub issue:** [#279](https://github.com/microscaler/BRRTRouter/issues/279)

**Scope:** Implement IDAM core as GoTrue proxy (+ optional session/Redis); BRRTRouter-generated service or shared library.

| ID   | Story | One-line description | Parent | Type | Labels | GitHub issue |
|------|--------|----------------------|--------|------|--------|--------------|
| 7.1  | IDAM core service skeleton | IDAM core service skeleton: BRRTRouter codegen from reference spec or shared library; path prefix and GoTrue base URL config. | Epic #279 | story | idam, story | [#284](https://github.com/microscaler/BRRTRouter/issues/284) |
| 7.2  | GoTrue client integration | Integrate GoTrue client: token, logout, signup, recover, resend, magiclink, otp, verify, user (GET/PUT), reauthenticate, factors, identity link/unlink, SSO/SAML, settings, JWKS, health. | Epic #279 | story | idam, story | [#285](https://github.com/microscaler/BRRTRouter/issues/285) |
| 7.3  | Optional session/Redis store | Optional server-side session store (e.g. Redis) for IDAM; config and wiring. | Epic #279 | story | idam, story | [#286](https://github.com/microscaler/BRRTRouter/issues/286) |

---

## Epic 8 — IDAM extension and build/deploy

**GitHub issue:** [#280](https://github.com/microscaler/BRRTRouter/issues/280)

**Scope:** Support core + extension: merged spec at build (single service) and path conventions for two-service + ingress option.

| ID   | Story | One-line description | Parent | Type | Labels | GitHub issue |
|------|--------|----------------------|--------|------|--------|--------------|
| 8.1  | Core + extension spec merge at build | Generator or manual step merges reference idam-core and customer idam-extension OpenAPI into one spec; BRRTRouter codegen on combined spec → single IDAM service. | Epic #280 | story | idam, story | [#287](https://github.com/microscaler/BRRTRouter/issues/287) |
| 8.2  | Path conventions and ingress rules | Document path conventions (core vs extension prefixes); document ingress path-based routing rules for two-service deployment. | Epic #280 | story | idam, story | [#288](https://github.com/microscaler/BRRTRouter/issues/288) |

---

## Epic 9 — BFF ↔ IDAM integration

**GitHub issue:** [#281](https://github.com/microscaler/BRRTRouter/issues/281)

**Scope:** BFF uses a single IDAM base URL; config and documentation.

| ID   | Story | One-line description | Parent | Type | Labels | GitHub issue |
|------|--------|----------------------|--------|------|--------|--------------|
| 9.1  | BFF IDAM base URL config and path layout | BFF config: `IDAM_BASE_URL` (and optional JWKS/issuer); document path layout so BFF and IDAM agree on auth, users/me, preferences, API keys paths. | Epic #281 | story | idam, story | [#289](https://github.com/microscaler/BRRTRouter/issues/289) |
| 9.2  | Document BFF usage of IDAM | Document how BFF uses IDAM: single URL, claims enrichment (call IDAM users/me or introspect), optional RBAC from JWT or IDAM API; link to BFF Proxy Analysis §5.4, §6. | Epic #281 | story | idam, story | [#290](https://github.com/microscaler/BRRTRouter/issues/290) |

---

## Labels and linking (for GitHub)

- **Epic issues:** Label `epic`, `idam`. Title: `[Epic N] <title>` (N = 6–9).
- **Story issues:** Label `story`, `idam`. In body: "Part of Epic #X". Title: `[Epic N] <story title>`.
- **Optional:** `phase-1`, `backend`, `bff`, `docs` as needed.

---

## File layout

```
docs/EPICS/IDAM/
├── README.md
├── EPICS_AND_STORIES_SUMMARY.md
├── epic-6-idam-contract/
│   ├── README.md
│   ├── story-6.1-document-idam-contract.md
│   └── story-6.2-reference-idam-core-openapi.md
├── epic-7-idam-core/
│   ├── README.md
│   ├── story-7.1-idam-core-service-skeleton.md
│   ├── story-7.2-gotrue-client-integration.md
│   └── story-7.3-optional-session-redis.md
├── epic-8-idam-extension/
│   ├── README.md
│   ├── story-8.1-core-extension-spec-merge.md
│   └── story-8.2-path-conventions-ingress.md
└── epic-9-bff-idam/
    ├── README.md
    ├── story-9.1-bff-idam-base-url-config.md
    └── story-9.2-document-bff-usage-idam.md
```
