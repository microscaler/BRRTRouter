# Theme: Framework Maturity (Epics 12+)

**Goal:** Make BRRTRouter a trustworthy OpenAPI-first web framework — safety,
contract fidelity, ops completeness, and honest docs — with
[**Sesame-IDAM**](https://github.com/microscaler/sesame-idam) as the **public**
reference consumer ([Building with BRRTRouter](../../BUILDING_WITH_BRRTRouter.md)).
WebSocket work is **parked** (separate may_minihttp epic).

**Testing:** [`TESTING_STANDARD.md`](TESTING_STANDARD.md) (positive + negative unit tests mandatory).  
**Board:** [`BUILD_BOARD.md`](BUILD_BOARD.md) ← **Epic 13 active**

## Scope

| In (Epic 13) | Out (parked / other epics) |
|--------------|----------------------------|
| Docs truth / claim reconciliation | WebSocket / may upgrade |
| Rate limiting → 429 | Full OAS 3.2 feature parity |
| RFC 7807 problem+json | Radix rewrites |
| Streaming uploads / download helpers | OAS callback auto-fire |
| Browser posture (kit or explicit OOS) | Fleet `openapi: 3.2.0` cutover |
| Handler deadlines → 504 | SPIFFE X.509 / mTLS |
| SSE live flush, opt-in compression | BFF claim enrichment (product) |
| Multi-status codegen + public TestApp | |

Epic 12 delivered: body 413, `$ref`, param validation, webhook kit, multipart MVP-A,
typed multi-status **runtime**, perf science.

## Epics

| Epic | Title | Issue | Doc |
|------|--------|-------|-----|
| 12 | Framework maturity — safety, OpenAPI fidelity, kits | [#391](https://github.com/microscaler/BRRTRouter/issues/391) | [epic-12-…](epic-12-framework-maturity/README.md) **done** |
| 13 | Framework completeness — ops, errors, files, DevEx | [#400](https://github.com/microscaler/BRRTRouter/issues/400) | [epic-13-…](epic-13-framework-completeness/README.md) **active** |

## Sibling themes (gap audit)

| Epic | Theme | Board |
|------|--------|--------|
| 14 | Zero-trust SPIFFE/mTLS (**critical**) | [../ZERO_TRUST/BUILD_BOARD.md](../ZERO_TRUST/BUILD_BOARD.md) |
| 15 | OpenAPI surface | [../OPENAPI_SURFACE/BUILD_BOARD.md](../OPENAPI_SURFACE/BUILD_BOARD.md) |
| 16 | Release & observability | [../RELEASE_MATURITY/BUILD_BOARD.md](../RELEASE_MATURITY/BUILD_BOARD.md) |

## Related

- OpenAPI version dual-support: [`docs/OPENAPI_VERSION_SUPPORT.md`](../../OPENAPI_VERSION_SUPPORT.md)
- Compliance gap inventory: [`OPENAPI_3.1.0_COMPLIANCE_GAP.md`](../../OPENAPI_3.1.0_COMPLIANCE_GAP.md)
- Hot-path / perf: [`docs/PERFORMANCE.md`](../../PERFORMANCE.md)
- Parked: [`../PARKED.md`](../PARKED.md)
