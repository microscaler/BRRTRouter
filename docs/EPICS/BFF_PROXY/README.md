# BFF Proxy Epics and Stories

**Source analysis:** [docs/BFF_PROXY_ANALYSIS.md](../../BFF_PROXY_ANALYSIS.md)  
**Summary (for authoring and GitHub issues):** [EPICS_AND_STORIES_SUMMARY.md](EPICS_AND_STORIES_SUMMARY.md)

This directory contains Epics and Stories for implementing BFF proxy behaviour in BRRTRouter (Phase 1: BFF ↔ IDAM ↔ Supabase, spec-driven proxy, Lifeguard with claims/row-based access; no LifeReflector).

## Epics

| Epic | Title | Directory | GitHub issue |
|------|--------|-----------|--------------|
| 1 | Spec-driven proxy (RouteMeta + BFF generator) | [epic-1-spec-driven-proxy/](epic-1-spec-driven-proxy/) | [#254](https://github.com/microscaler/BRRTRouter/issues/254) |
| 2 | BFF proxy library and generated handlers | [epic-2-proxy-library/](epic-2-proxy-library/) | [#255](https://github.com/microscaler/BRRTRouter/issues/255) |
| 3 | BFF ↔ IDAM auth/RBAC | [epic-3-bff-idam-auth/](epic-3-bff-idam-auth/) | [#256](https://github.com/microscaler/BRRTRouter/issues/256) |
| 4 | Enrich downstream with claims/RBAC | [epic-4-enrich-downstream/](epic-4-enrich-downstream/) | [#257](https://github.com/microscaler/BRRTRouter/issues/257) |
| 5 | Microservices: claims in handlers + Lifeguard row-based access | [epic-5-microservices-claims-lifeguard/](epic-5-microservices-claims-lifeguard/) | [#258](https://github.com/microscaler/BRRTRouter/issues/258) |

Each Epic has its own subdirectory with an Overview README and story files. After GitHub issues are created, Epic and Story docs are updated with the corresponding issue numbers.

## Quick reference

- **Epic 1:** RouteMeta extensions in BRRTRouter; BFF generator emits `x-brrtrouter-downstream-path` and `x-service`; BFF generator merges components/security.
- **Epic 2:** Proxy library (URL from config + RouteMeta, forward HTTP); downstream base URL config; Askama thin proxy handler; E2E BFF proxy integration.
- **Epic 3:** BFF OpenAPI securitySchemes (JWKS/IDAM); optional claims enrichment (call IDAM); RBAC from JWT or IDAM API.
- **Epic 4:** Proxy injects claim-derived headers; configurable claim→header mapping.
- **Epic 5:** TypedHandlerRequest or generated Request exposes jwt_claims; Lifeguard session claims API for RLS; microservice auth model (validate forwarded claims).

## References

- BRRTRouter: `src/spec/types.rs`, `src/spec/build.rs`, `src/server/service.rs`, `src/dispatcher/core.rs`, `src/security/mod.rs`, `src/typed/core.rs`
- OPENAPI_3.1.0_COMPLIANCE_GAP.md §8
- RERP: `openapi/accounting/bff-suite-config.yaml`, BFF generator (e.g. `generate_system.py`)
- IDAM: `idam/README.md`; Lifeguard: `lifeguard/README.md`
