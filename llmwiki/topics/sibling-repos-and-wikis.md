# Sibling repos: Lifeguard, Sesame, Hauliage, BRRTRouter

- **Status**: `verified`
- **Source docs**: [`AGENTS.md`](../../AGENTS.md), [`docs/BUILDING_WITH_BRRTROUTER.md`](../../docs/BUILDING_WITH_BRRTROUTER.md)
- **Code anchors**: n/a
- **Last updated**: 2026-08-08

## Layout (typical `microscaler/` checkout)

| Repo | Role | Public? | Wiki / docs |
|------|------|---------|-------------|
| **BRRTRouter** | OpenAPI-first HTTP router, validation, codegen | Yes | [`llmwiki/`](../index.md) (this tree) |
| **Lifeguard** | Coroutine ORM + `lifeguard-migrate` | Yes (typical) | [`lifeguard/docs/llmwiki/`](../../../lifeguard/docs/llmwiki/) |
| **Sesame-IDAM** | **Open reference product** on BRRTRouter (IDAM) | **Yes** — [github.com/microscaler/sesame-idam](https://github.com/microscaler/sesame-idam) | Sesame README + `docs/` |
| **Hauliage** | Logistics microservices + BFF (private dogfood) | **No** | Private wiki — do not cite as public learn path |
| **PriceWhisperer** | Market/trading dogfood | **No** | Private |
| **RERP** | Resource planning (immature) | Varies | **Not** a reference implementation yet |

## How agents / contributors should choose a reference

- **“How do I build a product with BRRTRouter?”** →
  [`docs/BUILDING_WITH_BRRTROUTER.md`](../../docs/BUILDING_WITH_BRRTROUTER.md) →
  **Sesame-IDAM** (public).
- **Transport / contract / 415 / OpenAPI extensions** → BRRTRouter wiki +
  [`reference/openapi-extensions.md`](../reference/openapi-extensions.md).
- **Entity DDL, migrations, UUID/chrono, pool** → Lifeguard wiki.
- **Private suite internals (Hauliage BFF merge, etc.)** → only when the user has
  that checkout; never as the default public pointer.

## Cross-references

- [`topics/brrtrouter-integration-pitfalls.md`](../../../lifeguard/docs/llmwiki/topics/brrtrouter-integration-pitfalls.md) (Lifeguard-side stack symptoms)
