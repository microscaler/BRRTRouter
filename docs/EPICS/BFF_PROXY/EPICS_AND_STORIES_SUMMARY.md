# BFF Proxy Epics and Stories — Summary

**Source:** `docs/BFF_PROXY_ANALYSIS.md`  
**Purpose:** Single reference used to author Epic/Story docs under `docs/EPICS/BFF_PROXY/` and to create/link GitHub issues. Phase 1 scope only (no LifeReflector).

**Related:** [BFF_GENERATOR_EXTRACTION_ANALYSIS.md](BFF_GENERATOR_EXTRACTION_ANALYSIS.md) — whether to extract the BFF generator from RERP into BRRTRouter Python tooling so any consumer has standard BFF tools; RERP can then import the module or call the CLI.

---

## Epic 1 — Spec-driven proxy (RouteMeta + BFF generator)

**Scope:** Add spec-driven downstream path and service key so BRRTRouter and the BFF generator agree on proxy targets; fix BFF generator security/component merge for OpenAPI 3.1 compliance.

| ID   | Story | One-line description | Parent | Type | Labels | GitHub issue |
|------|--------|----------------------|--------|------|--------|--------------|
| 1.1  | RouteMeta extensions | Add `x-brrtrouter-downstream-path` and `x-service` to RouteMeta in BRRTRouter (`spec/build.rs`, `spec/types.rs`). | Epic #254 | story | bff-proxy, story | [#259](https://github.com/microscaler/BRRTRouter/issues/259) |
| 1.2  | BFF generator proxy extensions | BFF generator emits `x-brrtrouter-downstream-path` and `x-service` per operation at merge time. | Epic #254 | story | bff-proxy, story | [#260](https://github.com/microscaler/BRRTRouter/issues/260) |
| 1.3  | BFF generator components/security merge | BFF generator merges `components.parameters`, `components.securitySchemes`, and root `security` (OPENAPI_3.1.0_COMPLIANCE_GAP §8). | Epic #254 | story | bff-proxy, story | [#261](https://github.com/microscaler/BRRTRouter/issues/261) |
| 1.4  | Extract BFF generator to BRRTRouter tooling | Extract BFF generator from RERP into BRRTRouter tooling (implements 1.2+1.3); migrate all tests; update RERP to use BRRTRouter tooling (import or CLI). | Epic #254 | story | bff-proxy, story | [#277](https://github.com/microscaler/BRRTRouter/issues/277) |

---

## Epic 2 — BFF proxy library and generated handlers

**Scope:** Implement proxy library (URL from config + RouteMeta, forward HTTP), downstream base URL config, and Askama-generated thin proxy handlers.

| ID   | Story | One-line description | Parent | Type | Labels | GitHub issue |
|------|--------|----------------------|--------|------|--------|--------------|
| 2.1  | Proxy library | Implement proxy library: build URL from config + RouteMeta, forward method/path/query/headers/body. | Epic #255 | story | bff-proxy, story | [#262](https://github.com/microscaler/BRRTRouter/issues/262) |
| 2.2  | Downstream base URL config | Config map from `x_service` (service key) to base URL (host:port). | Epic #255 | story | bff-proxy, story | [#263](https://github.com/microscaler/BRRTRouter/issues/263) |
| 2.3  | Askama proxy handler | Askama generates thin proxy handler when RouteMeta has `downstream_path` / `x_service`. | Epic #255 | story | bff-proxy, story | [#264](https://github.com/microscaler/BRRTRouter/issues/264) |
| 2.4  | BFF proxy integration | End-to-end: BFF from generated spec proxies to backend microservice. | Epic #255 | story | bff-proxy, story | [#265](https://github.com/microscaler/BRRTRouter/issues/265) |

---

## Epic 3 — BFF ↔ IDAM auth/RBAC

**Scope:** BFF validates tokens and obtains RBAC via IDAM (not Supabase directly); optional claims enrichment from IDAM.

| ID   | Story | One-line description | Parent | Type | Labels | GitHub issue |
|------|--------|----------------------|--------|------|--------|--------------|
| 3.1  | BFF OpenAPI securitySchemes | BFF spec has securitySchemes (e.g. JWKS from IDAM/Supabase issuer) and security so auth runs. | Epic #256 | story | bff-proxy, story | [#266](https://github.com/microscaler/BRRTRouter/issues/266) |
| 3.2  | Optional claims enrichment | Optional step: call IDAM (e.g. get user metadata/roles), merge into HandlerRequest. | Epic #256 | story | bff-proxy, story | [#267](https://github.com/microscaler/BRRTRouter/issues/267) |
| 3.3  | RBAC from JWT or IDAM API | RBAC available to BFF: from JWT claims (Auth Hooks) and/or IDAM “get roles” API; document and implement. | Epic #256 | story | bff-proxy, story | [#268](https://github.com/microscaler/BRRTRouter/issues/268) |

---

## Epic 4 — Enrich downstream with claims/RBAC

**Scope:** Proxy injects claim-derived headers so backend receives user/role context.

| ID   | Story | One-line description | Parent | Type | Labels | GitHub issue |
|------|--------|----------------------|--------|------|--------|--------------|
| 4.1  | Proxy claim headers | Proxy library injects claim-derived headers (e.g. X-User-Id, X-Roles) when forwarding. | Epic #257 | story | bff-proxy, story | [#269](https://github.com/microscaler/BRRTRouter/issues/269) |
| 4.2  | Configurable claim→header mapping | Config or OpenAPI extension defines which claims map to which header names. | Epic #257 | story | bff-proxy, story | [#270](https://github.com/microscaler/BRRTRouter/issues/270) |

---

## Epic 5 — Microservices: claims in handlers + Lifeguard row-based access

**Scope:** Backend microservices can read claims and use them with Lifeguard for Postgres RLS; validate forwarded claims.

| ID   | Story | One-line description | Parent | Type | Labels | GitHub issue |
|------|--------|----------------------|--------|------|--------|--------------|
| 5.1  | Expose jwt_claims to typed handlers | TypedHandlerRequest or generated Request exposes `jwt_claims` for microservice handlers. | Epic #258 | story | bff-proxy, story | [#271](https://github.com/microscaler/BRRTRouter/issues/271) |
| 5.2  | Lifeguard session claims | Lifeguard API to set session claims (e.g. `request.jwt.claims`) per request for RLS. | Epic #258 | story | bff-proxy, story | [#272](https://github.com/microscaler/BRRTRouter/issues/272) |
| 5.3  | Microservice auth model | Document and implement: validate forwarded claims (JWT or signed headers), bind to Lifeguard session. | Epic #258 | story | bff-proxy, story | [#273](https://github.com/microscaler/BRRTRouter/issues/273) |

---

## Labels and linking (for GitHub)

- **Epic issues:** Label `epic`, `bff-proxy`. Title: `[Epic N] <title>`.
- **Story issues:** Label `story`, `bff-proxy`. Link to parent Epic (e.g. “Part of Epic #X”). Title: `[Epic N] <story title>`.
- **Optional:** `phase-1` for all; `backend` / `bff` / `tooling` as needed.

---

## File layout

```
docs/EPICS/BFF_PROXY/
├── README.md                          # Index of epics
├── EPICS_AND_STORIES_SUMMARY.md       # This file
├── epic-1-spec-driven-proxy/
│   ├── README.md                      # Epic 1 overview (issue #TBD)
│   ├── story-1.1-route-meta-extensions.md
│   ├── story-1.2-bff-generator-proxy-extensions.md
│   └── story-1.3-bff-generator-components-security.md
├── epic-2-proxy-library/
│   ├── README.md
│   ├── story-2.1-proxy-library.md
│   ├── story-2.2-downstream-base-url-config.md
│   ├── story-2.3-askama-proxy-handler.md
│   └── story-2.4-bff-proxy-integration.md
├── epic-3-bff-idam-auth/
│   ├── README.md
│   ├── story-3.1-bff-openapi-security-schemes.md
│   ├── story-3.2-optional-claims-enrichment.md
│   └── story-3.3-rbac-from-jwt-or-idam.md
├── epic-4-enrich-downstream/
│   ├── README.md
│   ├── story-4.1-proxy-claim-headers.md
│   └── story-4.2-configurable-claim-header-mapping.md
└── epic-5-microservices-claims-lifeguard/
    ├── README.md
    ├── story-5.1-expose-jwt-claims-typed-handlers.md
    ├── story-5.2-lifeguard-session-claims.md
    └── story-5.3-microservice-auth-model.md
```

After GitHub issues are created, each README and story file is updated with `**GitHub issue:** #N`.

---

## GitHub issues (created)

| Epic | Issue | Stories | Story issues |
|------|-------|---------|--------------|
| 1 | [#254](https://github.com/microscaler/BRRTRouter/issues/254) | 1.1, 1.2, 1.3, 1.4 | #259, #260, #261, #277 |
| 2 | [#255](https://github.com/microscaler/BRRTRouter/issues/255) | 2.1–2.4 | #262, #263, #264, #265 |
| 3 | [#256](https://github.com/microscaler/BRRTRouter/issues/256) | 3.1–3.3 | #266, #267, #268 |
| 4 | [#257](https://github.com/microscaler/BRRTRouter/issues/257) | 4.1, 4.2 | #269, #270 |
| 5 | [#258](https://github.com/microscaler/BRRTRouter/issues/258) | 5.1–5.3 | #271, #272, #273 |

Stories are linked as sub-issues to their Epic in GitHub. All issues use labels `bff-proxy` and `epic` or `story`.
