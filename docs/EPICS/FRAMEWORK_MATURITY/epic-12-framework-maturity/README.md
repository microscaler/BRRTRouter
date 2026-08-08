# Epic 12 — Framework maturity

**GitHub issue:** [#391](https://github.com/microscaler/BRRTRouter/issues/391)  
**Theme labels:** `framework-maturity`, `epic`  
**Testing:** [`../TESTING_STANDARD.md`](../TESTING_STANDARD.md)

## Overview

Close the highest-ROI gaps between “router that works” and “OpenAPI-first web
framework we trust in production”: inbound safety, OpenAPI contract fidelity,
validation before handlers, multipart honesty, typed status codegen, a small
webhook delivery kit for sesame-idam, truthful docs, and measurable perf science.

**Does not include WebSocket** (parked — needs may_minihttp upgrade).  
**Does not include** radix rewrites or stack-size plumbing (already ~85% done).

## Success criteria (epic-level)

- [ ] Stories 12.1–12.8 meet TESTING_STANDARD (positive + negative unit tests).
- [x] Inbound oversize bodies → **413** (not OOM / silent truncate). (12.2)
- [ ] `$ref` requestBodies/responses used by product specs resolve (no silent drop).
- [ ] Required query/header params fail closed **before** handler.
- [ ] Multipart either works or fails closed (no empty-object bypass).
- [ ] Docs (README feature table / ROADMAP) match reality.
- [ ] Webhook kit usable from sesame-style handlers (HMAC + retry documented).

## Wave plan

```text
Wave 0 ──► 12.1 docs ‖ 12.2 body 413
              │
Wave 1 ──► 12.3 $ref ‖ 12.4 param validation
              │
Wave 2 ──► 12.5 webhook outbound kit
              │
Wave 3 ──► 12.6 multipart ‖ 12.7 multi-status codegen
              │
Wave 4 ──► 12.8 perf science (benches + flamegraph)
```

| Wave | Stories | Outcome |
|------|---------|---------|
| 0 | 12.1, 12.2 | Trust + safety baseline |
| 1 | 12.3, 12.4 | OpenAPI contract E2E |
| 2 | 12.5 | Sesame webhook delivery |
| 3 | 12.6, 12.7 | Upload + REST status fidelity |
| 4 | 12.8 | Credible perf measurement |

## Stories

| Story | Title | Issue | Blocked by |
|-------|--------|-------|------------|
| 12.1 | Doc / status reconciliation | [#392](https://github.com/microscaler/BRRTRouter/issues/392) | — |
| 12.2 | Hard inbound body limits → 413 | [#393](https://github.com/microscaler/BRRTRouter/issues/393) | — |
| 12.3 | OpenAPI `$ref` for requestBodies / responses / pathItems | [#394](https://github.com/microscaler/BRRTRouter/issues/394) | — |
| 12.4 | Pre-handler query/header validation | [#395](https://github.com/microscaler/BRRTRouter/issues/395) | benefits from 12.3 |
| 12.5 | Webhook outbound delivery kit | [#396](https://github.com/microscaler/BRRTRouter/issues/396) | — |
| 12.6 | Multipart form-data truth | [#397](https://github.com/microscaler/BRRTRouter/issues/397) | 12.2 helpful |
| 12.7 | Multi-status typed / codegen | [#398](https://github.com/microscaler/BRRTRouter/issues/398) | — |
| 12.8 | Perf science Phase 6 | [#399](https://github.com/microscaler/BRRTRouter/issues/399) | Prefer after Wave 1 |

## References

- [`OPENAPI_3.1.0_COMPLIANCE_GAP.md`](../../../OPENAPI_3.1.0_COMPLIANCE_GAP.md)
- [`docs/PRD_TYPED_HANDLER_HTTP_STATUS.md`](../../../PRD_TYPED_HANDLER_HTTP_STATUS.md)
- [`docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](../../../PRD_HOT_PATH_V2_STABILITY_AND_PERF.md)
- [`docs/OPENAPI_VERSION_SUPPORT.md`](../../../OPENAPI_VERSION_SUPPORT.md)
