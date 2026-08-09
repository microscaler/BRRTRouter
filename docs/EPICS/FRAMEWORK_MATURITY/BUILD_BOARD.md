# Framework Maturity — Build Board

**Theme:** Epics 12–13  
**Testing:** [`TESTING_STANDARD.md`](TESTING_STANDARD.md)  
**Summaries:** [`EPICS_AND_STORIES_SUMMARY.md`](EPICS_AND_STORIES_SUMMARY.md)

## Now / next — Epic 13

| Priority | ID | Status | Issue | Notes |
|----------|-----|--------|-------|-------|
| **DONE** | 13.1 | done | [#401](https://github.com/microscaler/BRRTRouter/issues/401) | Doc truth & claim reconciliation |
| **DONE** | 13.2 | done | [#402](https://github.com/microscaler/BRRTRouter/issues/402) | Rate limiting |
| **DONE** | 13.3 | done | [#403](https://github.com/microscaler/BRRTRouter/issues/403) | Problem Details (RFC 7807) |
| **DONE** | 13.4 | done | [#404](https://github.com/microscaler/BRRTRouter/issues/404) | Streaming uploads/downloads |
| **DONE** | 13.5 | done | [#405](https://github.com/microscaler/BRRTRouter/issues/405) | Browser security posture (Option B) |
| **DONE** | 13.6 | done | [#406](https://github.com/microscaler/BRRTRouter/issues/406) | Handler deadlines → 504 |
| **NOW** | 13.7 | todo | [#407](https://github.com/microscaler/BRRTRouter/issues/407) | SSE live flush (may_minihttp streaming) |
| **DONE** | 13.8 | done | [#408](https://github.com/microscaler/BRRTRouter/issues/408) | Response compression |
| — | 13.9 | todo | [#409](https://github.com/microscaler/BRRTRouter/issues/409) | Multi-status codegen |
| — | 13.10 | todo | [#410](https://github.com/microscaler/BRRTRouter/issues/410) | Public TestApp |

## Wave plan (Epic 13)

```text
Wave 0 ──► 13.1
Wave 1 ──► 13.2 ‖ 13.3
Wave 2 ──► 13.4
Wave 3 ──► 13.5 ‖ 13.6
Wave 4 ──► 13.7 ‖ 13.8
Wave 5 ──► 13.9 ‖ 13.10
```

## Epic 13 story index

| ID | Title | Wave | Status | GitHub |
|----|--------|------|--------|--------|
| Epic 13 | Framework completeness | — | todo | [#400](https://github.com/microscaler/BRRTRouter/issues/400) |
| 13.1 | Doc truth & claim reconciliation | 0 | done | [#401](https://github.com/microscaler/BRRTRouter/issues/401) |
| 13.2 | Rate limiting middleware | 1 | done | [#402](https://github.com/microscaler/BRRTRouter/issues/402) |
| 13.3 | Problem Details (RFC 7807) | 1 | done | [#403](https://github.com/microscaler/BRRTRouter/issues/403) |
| 13.4 | Streaming uploads & download helpers | 2 | done | [#404](https://github.com/microscaler/BRRTRouter/issues/404) |
| 13.5 | Browser security posture | 3 | done | [#405](https://github.com/microscaler/BRRTRouter/issues/405) |
| 13.6 | Handler / request deadlines → 504 | 3 | done | [#406](https://github.com/microscaler/BRRTRouter/issues/406) |
| 13.7 | SSE live flush streaming | 4 | todo | [#407](https://github.com/microscaler/BRRTRouter/issues/407) |
| 13.8 | Response compression middleware | 4 | done | [#408](https://github.com/microscaler/BRRTRouter/issues/408) |
| 13.9 | Multi-status response codegen | 5 | todo | [#409](https://github.com/microscaler/BRRTRouter/issues/409) |
| 13.10 | Public TestApp / RequestBuilder | 5 | todo | [#410](https://github.com/microscaler/BRRTRouter/issues/410) |

## Epic 12 — complete (archive)

| ID | Title | Status | GitHub |
|----|--------|--------|--------|
| Epic 12 | Framework maturity | done | [#391](https://github.com/microscaler/BRRTRouter/issues/391) |
| 12.1 | Doc / status reconciliation | done | [#392](https://github.com/microscaler/BRRTRouter/issues/392) |
| 12.2 | Hard inbound body limits → 413 | done | [#393](https://github.com/microscaler/BRRTRouter/issues/393) |
| 12.3–12.8 | See [epic-12 README](epic-12-framework-maturity/README.md) | done | #394–#399 |

## Sibling epics (from gap audit — not in Epic 13)

| Epic | Board | Why separate |
|------|--------|----------------|
| **14** SPIFFE/mTLS/Federation | [ZERO_TRUST](../ZERO_TRUST/BUILD_BOARD.md) | **Critical** zero-trust; large may_minihttp/TLS surface |
| **15** OpenAPI surface | [OPENAPI_SURFACE](../OPENAPI_SURFACE/BUILD_BOARD.md) | Contract fidelity beyond ops/DevEx kits |
| **16** Release maturity | [RELEASE_MATURITY](../RELEASE_MATURITY/BUILD_BOARD.md) | Packaging / API freeze / OTEL tests |

## Parked (no epic)

See [`../PARKED.md`](../PARKED.md): WebSocket, callback auto-fire engine, radix rewrite.  
BFF claim enrichment remains Epics **3–5 / 6–9**.
