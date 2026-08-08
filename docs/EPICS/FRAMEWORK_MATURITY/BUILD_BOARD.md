# Framework Maturity — Build Board

**Theme:** Epic 12  
**Testing:** [`TESTING_STANDARD.md`](TESTING_STANDARD.md)  
**Summary:** [`EPICS_AND_STORIES_SUMMARY.md`](EPICS_AND_STORIES_SUMMARY.md)

## Now / next

| Priority | ID | Status | Issue | Notes |
|----------|-----|--------|-------|-------|
| **DONE** | 12.1 | done | [#392](https://github.com/microscaler/BRRTRouter/issues/392) | Doc reconciliation |
| **DONE** | 12.2 | done | [#393](https://github.com/microscaler/BRRTRouter/issues/393) | Inbound body → 413 |
| **DONE** | 12.3 | done | [#394](https://github.com/microscaler/BRRTRouter/issues/394) | `$ref` bodies/responses/pathItems |
| **DONE** | 12.4 | done | [#395](https://github.com/microscaler/BRRTRouter/issues/395) | Pre-handler param validation |
| **NOW** | 12.5 | todo | [#396](https://github.com/microscaler/BRRTRouter/issues/396) | Webhook outbound kit |
| LATER | 12.6 | todo | [#397](https://github.com/microscaler/BRRTRouter/issues/397) | Multipart truth |
| LATER | 12.7 | todo | [#398](https://github.com/microscaler/BRRTRouter/issues/398) | Multi-status codegen |
| LATER | 12.8 | todo | [#399](https://github.com/microscaler/BRRTRouter/issues/399) | Perf science |

## Wave plan

```text
Wave 0 ──► 12.1 ‖ 12.2
Wave 1 ──► 12.3 ‖ 12.4
Wave 2 ──► 12.5
Wave 3 ──► 12.6 ‖ 12.7
Wave 4 ──► 12.8
```

## Full story index

| ID | Title | Wave | Status | GitHub |
|----|--------|------|--------|--------|
| Epic 12 | Framework maturity | — | todo | [#391](https://github.com/microscaler/BRRTRouter/issues/391) |
| 12.1 | Doc / status reconciliation | 0 | done | [#392](https://github.com/microscaler/BRRTRouter/issues/392) |
| 12.2 | Hard inbound body limits → 413 | 0 | done | [#393](https://github.com/microscaler/BRRTRouter/issues/393) |
| 12.3 | OpenAPI `$ref` requestBodies / responses / pathItems | 1 | done | [#394](https://github.com/microscaler/BRRTRouter/issues/394) |
| 12.4 | Pre-handler query/header validation | 1 | done | [#395](https://github.com/microscaler/BRRTRouter/issues/395) |
| 12.5 | Webhook outbound delivery kit | 2 | todo | [#396](https://github.com/microscaler/BRRTRouter/issues/396) |
| 12.6 | Multipart form-data truth | 3 | todo | [#397](https://github.com/microscaler/BRRTRouter/issues/397) |
| 12.7 | Multi-status typed / codegen | 3 | todo | [#398](https://github.com/microscaler/BRRTRouter/issues/398) |
| 12.8 | Perf science Phase 6 | 4 | todo | [#399](https://github.com/microscaler/BRRTRouter/issues/399) |

## Parked (not in this epic)

- WebSocket / may_minihttp upgrade
- Radix trie rewrite / stack-size plumbing
- Full OAS callback auto-fire runtime
- Fleet OpenAPI 3.2.0 cutover
