# Framework Maturity — Epics and stories summary

## Epic 13 — Framework completeness (active)

| ID | Title | Effort | Wave | Notes |
|----|--------|--------|------|-------|
| Epic 13 | Framework completeness | — | — | Parent |
| 13.1 | Doc truth & claim reconciliation | S | 0 | OPENAPI gap, marketing, ROADMAP |
| 13.2 | Rate limiting middleware | M | 1 | 429 + metrics |
| 13.3 | Problem Details (RFC 7807) | M | 1 | problem+json |
| 13.4 | Streaming uploads & download helpers | L | 2 | Beyond multipart MVP-A |
| 13.5 | Browser security posture | M | 3 | CSRF/cookies kit **or** explicit OOS |
| 13.6 | Handler / request deadlines → 504 | M | 3 | Completes k8s ops story |
| 13.7 | SSE live flush streaming | M | 4 | Not buffered collect-only |
| 13.8 | Response compression middleware | M | 4 | Opt-in gzip |
| 13.9 | Multi-status response codegen | M | 5 | Finish 12.7 codegen |
| 13.10 | Public TestApp / RequestBuilder | M | 5 | Consumer test API |

## Epic 12 — Framework maturity (done)

| ID | Title | Effort | Wave | Notes |
|----|--------|--------|------|-------|
| Epic 12 | Framework maturity | — | — | Parent — **done** |
| 12.1 | Doc / status reconciliation | S | 0 | README/ROADMAP truth |
| 12.2 | Hard inbound body limits → 413 | S–M | 0 | DoS / memory |
| 12.3 | OpenAPI `$ref` requestBodies / responses / pathItems | M | 1 | Silent schema drop |
| 12.4 | Pre-handler query/header validation | M | 1 | OpenAPI-first E2E |
| 12.5 | Webhook outbound delivery kit | M | 2 | Sesame; not OAS auto-fire |
| 12.6 | Multipart form-data truth | L | 3 | MVP-A |
| 12.7 | Multi-status typed / codegen | L | 3 | Runtime done; codegen → 13.9 |
| 12.8 | Perf science (Phase 6) | M | 4 | Evidence: no radix rewrite |

**Sibling epics:** 14 (SPIFFE/mTLS), 15 (OpenAPI surface), 16 (release maturity).  
**Parked:** see [`../PARKED.md`](../PARKED.md).
