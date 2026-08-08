# Building with BRRTRouter — open reference

## Public reference product: Sesame-IDAM

For a **public, cloneable** example of a multi-service product built on BRRTRouter
(OpenAPI → `gen/` + `impl/`, typed handlers, Kind + Tilt on shared-k8s), use:

**[microscaler/sesame-idam](https://github.com/microscaler/sesame-idam)**  
(*Sesame* — identity and access management)

Start from that repository’s README and OpenAPI specs under `microservices/`, then
come back here for framework behaviour (routing, validation, CORS, body limits,
QUERY, epics).

In-repo **pet_store** (`examples/`) remains the small tutorial / CI fixture.
Sesame is the **product-shaped** reference.

## Other Microscaler consumers (not public references)

| Suite | Visibility | Role vs this doc |
|-------|------------|------------------|
| **Sesame-IDAM** | **Public** | Open reference for building with BRRTRouter |
| **pet_store** | In this repo | Minimal example / tests |
| **Hauliage** | Private | Production logistics suite — patterns may inform internals; do not cite as a public learn path |
| **PriceWhisperer** | Private | Early production dogfood; historical JSF feedback; not a public reference |
| **RERP** | Immature | Do not treat as a reference implementation yet |

Private suites still consume BRRTRouter and follow the same OpenAPI version policy
([OPENAPI_VERSION_SUPPORT.md](OPENAPI_VERSION_SUPPORT.md)); they are simply not
the docs’ “go look here” target.

## What to copy from Sesame

- Multi-crate workspace: OpenAPI per service → BRRTRouter codegen → `impl` controllers
- Shared-k8s / Tilt layout for local iteration
- Security schemes and public vs secured routes (`security: []` where intentional)
- Webhook-style REST surfaces (outbound delivery kit is Epic 12.5)

## Related

- [Local development](LOCAL_DEVELOPMENT.md) (Tilt + kind for this repo / pet_store)
- [Epic 12 — Framework maturity](EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md)
- [Sibling repos (wiki)](../llmwiki/topics/sibling-repos-and-wikis.md)
