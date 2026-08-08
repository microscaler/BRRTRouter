# Theme: Framework Maturity (Epics 12+)

**Goal:** Make BRRTRouter a trustworthy OpenAPI-first web framework for
hauliage / sesame-idam / rerp / PriceWhisperer — safety, contract fidelity,
and honest docs. WebSocket work is **parked** (separate may_minihttp epic).

**Testing:** [`TESTING_STANDARD.md`](TESTING_STANDARD.md) (positive + negative unit tests mandatory).  
**Board:** [`BUILD_BOARD.md`](BUILD_BOARD.md)

## Scope

| In | Out (for now) |
|----|----------------|
| Inbound body hard limits | WebSocket / may upgrade |
| OpenAPI `$ref` fidelity | Full OAS 3.2 feature parity |
| Param validation before handler | Radix micro-opts / stack-size plumbing |
| Multipart truth | Full OAS callback auto-fire runtime |
| Multi-status codegen | Fleet `openapi: 3.2.0` cutover |
| Webhook **outbound delivery kit** | Trie rewrite |
| Doc reconciliation + measurable perf science | |

## Epic

| Epic | Title | Issue | Doc |
|------|--------|-------|-----|
| 12 | Framework maturity — safety, OpenAPI fidelity, platform kits | [#391](https://github.com/microscaler/BRRTRouter/issues/391) | [epic-12-…](epic-12-framework-maturity/README.md) |

## Related

- OpenAPI version dual-support: [`docs/OPENAPI_VERSION_SUPPORT.md`](../../OPENAPI_VERSION_SUPPORT.md)
- Compliance gap inventory: [`OPENAPI_3.1.0_COMPLIANCE_GAP.md`](../../OPENAPI_3.1.0_COMPLIANCE_GAP.md)
- Hot-path PRD: [`docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](../../PRD_HOT_PATH_V2_STABILITY_AND_PERF.md)
