# Story 13.1 — Doc truth & claim reconciliation

**GitHub issue:** [#401](https://github.com/microscaler/BRRTRouter/issues/401)  
**Epic:** [Epic 13](README.md)  
**Wave:** 0  
**Effort:** S  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Align README, ROADMAP, OPENAPI compliance gap inventory, marketing guides, and
module docs with post–Epic-12 reality so contributors do not chase phantom
features (rate limit, compression, RFC 7807, “regex routing”) or miss shipped work
(`$ref` requestBodies/responses).

## Delivery

- Reconcile [`OPENAPI_3.1.0_COMPLIANCE_GAP.md`](../../../OPENAPI_3.1.0_COMPLIANCE_GAP.md) with Story 12.3.
- Point ROADMAP “Now” at Epic 13; mark Epic 12 complete.
- Strike or caveat unshipped claims: rate limiting, compression middleware, RFC 7807.
- Document `OAuth2Provider` as simplified JWT / prefer JWKS Bearer path.
- Fix stale module comments (e.g. multipart “future” if still present).
- Link Epic 13 BUILD_BOARD from EPICS_CATALOG / theme README.

## Functional requirements

| ID | Requirement |
|----|-------------|
| FR-1 | OPENAPI gap doc marks local `$ref` for requestBodies/responses/pathItems as **supported** (or equivalent truth). |
| FR-2 | ROADMAP / README point active work at Epic 13 board. |
| FR-3 | Marketing / RequestLifecycle do not list RateLimit or Compression as shipped middleware until 13.2/13.8 done. |
| FR-4 | Security docs state JWKS Bearer as production OAuth-shaped path; `OAuth2Provider` labeled non-production / stub. |
| FR-5 | Epic 12 success/checklist items that are done remain marked done; Epic 13 linked from catalog. |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| NFR-1 | No silent deletion of useful historical roadmap items without archive note. |
| NFR-2 | Relative links in touched docs resolve. |
| NFR-3 | Doc fixture tests are deterministic (`include_str!` / `rg` style). |

## Unit tests (required)

Docs story: fixture / `include_str!` guards.

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | OPENAPI gap reflects `$ref` requestBodies/responses support | present |
| P2 | ROADMAP or README links Epic 13 BUILD_BOARD | present |
| P3 | Epic 12 marked complete / not “Now” | present |
| P4 | JWKS recommended over stub OAuth2 in security docs | present |
| P5 | EPICS_CATALOG lists Epic 13 | present |
| P6 | Multipart documented as MVP-A or streaming story pointer | present |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Claim “rate limiting included” as shipped | forbidden until 13.2 |
| N2 | Claim RFC 7807 / problem+json as shipped | forbidden until 13.3 |
| N3 | Claim CompressionMiddleware as shipping code | forbidden until 13.8 |
| N4 | Gap doc still ❌ for requestBodies `$ref` | forbidden |
| N5 | Broken relative links in updated files | forbidden |
| N6 | WS listed as in-progress MVP | forbidden |

### Acceptance criteria (tests)

- [ ] Doc fixture test covers P1–P3 and N1–N4.

## Acceptance criteria

- [ ] Touched docs match engineering truth.
- [ ] Unit tests section complete.
- [ ] FR/NFR tables satisfied.

## References

- `docs/marketing/BEGINNER_GUIDE.md`, `docs/RequestLifecycle.md`, `docs/ARCHITECTURE.md`
- `docs/OPENAPI_3.1.0_COMPLIANCE_GAP.md`, `docs/openapi_component_refs.md`
