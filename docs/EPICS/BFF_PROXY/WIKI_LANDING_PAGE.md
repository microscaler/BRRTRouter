# BFF Proxy

Epics and stories for implementing **BFF proxy behaviour** in BRRTRouter. Phase 1 covers: BFF ↔ IDAM ↔ Supabase auth/RBAC, spec-driven proxy to backend microservices, and Lifeguard with claims/row-based access. LifeReflector is planned for a later phase.

---

## Contents

| Page | Description |
|------|-------------|
| [BFF Proxy Analysis](BFF-Proxy/BFF-Proxy-Analysis) | Full analysis, target behaviour, auth flow, and recommendations |
| [Epics and Stories Summary](BFF-Proxy/Epics-and-Stories-Summary) | Summary table and GitHub issue mapping (#254–#258, #259–#273) |

---

## Epics (Phase 1)

| # | Epic | Issue | Overview |
|---|------|--------|----------|
| 1 | **Spec-driven proxy** (RouteMeta + BFF generator) | [#254](https://github.com/microscaler/BRRTRouter/issues/254) | [Epic 1 overview](BFF-Proxy/Epic-1-Spec-Driven-Proxy) |
| 2 | **BFF proxy library** and generated handlers | [#255](https://github.com/microscaler/BRRTRouter/issues/255) | [Epic 2 overview](BFF-Proxy/Epic-2-BFF-Proxy-Library) |
| 3 | **BFF ↔ IDAM auth/RBAC** | [#256](https://github.com/microscaler/BRRTRouter/issues/256) | [Epic 3 overview](BFF-Proxy/Epic-3-BFF-IDAM-Auth) |
| 4 | **Enrich downstream** with claims/RBAC | [#257](https://github.com/microscaler/BRRTRouter/issues/257) | [Epic 4 overview](BFF-Proxy/Epic-4-Enrich-Downstream) |
| 5 | **Microservices: claims + Lifeguard** row-based access | [#258](https://github.com/microscaler/BRRTRouter/issues/258) | [Epic 5 overview](BFF-Proxy/Epic-5-Microservices-Claims-Lifeguard) |

---

## Quick reference

- **Epic 1:** RouteMeta extensions (`x-brrtrouter-downstream-path`, `x-service`); BFF generator proxy extensions and components/security merge.
- **Epic 2:** Proxy library, downstream base URL config, Askama proxy handler, end-to-end BFF proxy integration.
- **Epic 3:** BFF OpenAPI securitySchemes (JWKS/IDAM), optional claims enrichment, RBAC from JWT or IDAM API.
- **Epic 4:** Proxy claim headers (e.g. X-User-Id, X-Roles), configurable claim→header mapping.
- **Epic 5:** Expose jwt_claims to typed handlers, Lifeguard session claims for RLS, microservice auth model (validate forwarded claims).

---

## Repo sources

- **Analysis:** `docs/BFF_PROXY_ANALYSIS.md`
- **Epics & stories:** `docs/EPICS/BFF_PROXY/`  
- **Wiki structure:** `docs/EPICS/BFF_PROXY/WIKI_STRUCTURE.md`
