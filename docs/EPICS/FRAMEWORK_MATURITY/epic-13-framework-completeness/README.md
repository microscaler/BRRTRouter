# Epic 13 — Framework completeness

**GitHub issue:** [#400](https://github.com/microscaler/BRRTRouter/issues/400)  
**Theme labels:** `framework-maturity`, `epic`  
**Testing:** [`../TESTING_STANDARD.md`](../TESTING_STANDARD.md)  
**Board:** [`../BUILD_BOARD.md`](../BUILD_BOARD.md)

## Overview

Close the highest-ROI gaps between “Epic 12 safety/fidelity core” and a
**complete OpenAPI-first HTTP web framework** for suite products: honest docs,
ops middleware (rate limit, deadlines, optional compression), RFC 7807 errors,
streaming file upload/download, browser/session posture (or explicit out-of-scope),
live SSE flush, and DevEx (multi-status codegen + public `TestApp`).

**Does not include WebSocket** (parked — [PARKED.md](../../PARKED.md)).  
**Sibling epics (gap audit):** [Epic 14 SPIFFE/mTLS](../../ZERO_TRUST/epic-14-spiffe-mtls-federation/README.md) (**critical**),
[Epic 15 OpenAPI surface](../../OPENAPI_SURFACE/epic-15-openapi-surface-completion/README.md),
[Epic 16 Release maturity](../../RELEASE_MATURITY/epic-16-release-and-observability/README.md).  
BFF claim enrichment remains Epics 3–5 / 6–9.

Grounded in the post–Epic-12 gap audit (2026-08-09).

## Success criteria (epic-level)

- [ ] Stories 13.1–13.10 meet TESTING_STANDARD (positive + negative unit tests).
- [ ] Docs / marketing no longer claim unshipped rate-limit, compression, or RFC 7807
      unless the matching story has shipped.
- [ ] Public APIs can enable **rate limiting** → **429** with stable error JSON + metrics.
- [ ] Framework errors can emit **`application/problem+json`** (RFC 7807) with stable `type` URIs.
- [ ] Large multipart uploads have a **stream-to-disk** path (not only buffered MVP-A).
- [ ] Browser cookie/CSRF posture is either **shipped as a kit** or **explicitly out of scope**
      in operator docs (Bearer/JWKS path remains the default).
- [ ] Slow handlers can hit a **deadline → 504** without hanging the worker forever.
- [ ] Multi-status codegen + public `TestApp` usable from Sesame-style product crates.

## Wave plan

```text
Wave 0 ──► 13.1 docs truth
              │
Wave 1 ──► 13.2 rate limit ‖ 13.3 problem+json
              │
Wave 2 ──► 13.4 streaming files
              │
Wave 3 ──► 13.5 browser posture ‖ 13.6 handler deadlines
              │
Wave 4 ──► 13.7 SSE flush ‖ 13.8 compression
              │
Wave 5 ──► 13.9 multi-status codegen ‖ 13.10 TestApp
```

| Wave | Stories | Outcome |
|------|---------|---------|
| 0 | 13.1 | Trust baseline — claims match code |
| 1 | 13.2, 13.3 | Ops middleware + interoperable errors |
| 2 | 13.4 | Production-grade file bodies |
| 3 | 13.5, 13.6 | Browser honesty + k8s timeout story |
| 4 | 13.7, 13.8 | Streaming responses / bandwidth |
| 5 | 13.9, 13.10 | Consumer DevEx |

## Stories

| Story | Title | Issue | Effort | Blocked by |
|-------|--------|-------|--------|------------|
| 13.1 | Doc truth & claim reconciliation | [#401](https://github.com/microscaler/BRRTRouter/issues/401) | S | — |
| 13.2 | Rate limiting middleware | [#402](https://github.com/microscaler/BRRTRouter/issues/402) | M | — |
| 13.3 | Problem Details (RFC 7807) | [#403](https://github.com/microscaler/BRRTRouter/issues/403) | M | benefits from 13.1 |
| 13.4 | Streaming uploads & download helpers | [#404](https://github.com/microscaler/BRRTRouter/issues/404) | L | 12.2/12.6 helpful |
| 13.5 | Browser security posture | [#405](https://github.com/microscaler/BRRTRouter/issues/405) | M | — |
| 13.6 | Handler / request deadlines → 504 | [#406](https://github.com/microscaler/BRRTRouter/issues/406) | M | — |
| 13.7 | SSE live flush streaming | [#407](https://github.com/microscaler/BRRTRouter/issues/407) | M | — |
| 13.8 | Response compression middleware | [#408](https://github.com/microscaler/BRRTRouter/issues/408) | M | — |
| 13.9 | Multi-status response codegen | [#409](https://github.com/microscaler/BRRTRouter/issues/409) | M | 12.7 |
| 13.10 | Public TestApp / RequestBuilder | [#410](https://github.com/microscaler/BRRTRouter/issues/410) | M | — |

## Functional requirements (epic)

| ID | Requirement |
|----|-------------|
| E-FR-1 | Operators can enable per-route or global rate limits that shed with **429**. |
| E-FR-2 | Framework-generated client errors can use RFC 7807 Problem Details. |
| E-FR-3 | OpenAPI `multipart/form-data` file parts can be written to a temp/stream sink under size caps. |
| E-FR-4 | Handlers can return file downloads with correct disposition / content-type helpers. |
| E-FR-5 | Cookie/CSRF helpers exist **or** docs state Bearer-only and forbid implying sessions. |
| E-FR-6 | Configurable handler deadline yields **504** with metrics. |
| E-FR-7 | `x-sse` routes can flush events without buffering the entire stream. |
| E-FR-8 | Optional response compression for eligible content types. |
| E-FR-9 | Codegen emits usable multi-status typed returns from OpenAPI `responses`. |
| E-FR-10 | Product crates can drive the app in-process via a public test client API. |

## Non-functional requirements (epic)

| ID | Requirement |
|----|-------------|
| E-NFR-1 | No panics on hostile input on any new middleware/kit path. |
| E-NFR-2 | Hot-path rate-limit / deadline checks are lock-friendly (prefer atomics / sharded maps; no `RwLock` on router match). |
| E-NFR-3 | Error JSON / Problem Details fields are stable (`type`/`title`/`status`/`detail`/`reason` as specified per story). |
| E-NFR-4 | Streaming uploads honor existing global/route body caps (Story 12.2) — no silent truncate. |
| E-NFR-5 | Docs and marketing must not claim features until the story is **done**. |
| E-NFR-6 | Default configs remain safe for public APIs (fail-closed rate limits off until configured; compression opt-in). |
| E-NFR-7 | Unit tests follow TESTING_STANDARD (≥5 positive + ≥5 negative where domain allows). |

## Out of scope here (tracked elsewhere)

| Item | Where |
|------|--------|
| SPIFFE X.509 / mTLS / federation | **Epic 14** |
| Response headers, servers, encoding, methods, versioning, callbacks fidelity, OAS 3.2 readiness | **Epic 15** |
| crates.io / API freeze / fake OTEL / beta | **Epic 16** |
| WebSocket, callback auto-fire engine, radix rewrite | [PARKED.md](../../PARKED.md) |
| BFF claim enrichment | Epics 3–5 / 6–9 |

## References

- Gap audit canvas (session 2026-08-09)
- [`OPENAPI_3.1.0_COMPLIANCE_GAP.md`](../../../OPENAPI_3.1.0_COMPLIANCE_GAP.md)
- [`docs/PERFORMANCE.md`](../../../PERFORMANCE.md) § Phase 6
- Epic 12 board (complete): [`../BUILD_BOARD.md`](../BUILD_BOARD.md)
