# Sibling repos: Lifeguard, Hauliage, BRRTRouter

- **Status**: `verified`
- **Source docs**: [`AGENTS.md`](../../AGENTS.md)
- **Code anchors**: n/a
- **Last updated**: 2026-04-17

## Layout (typical `microscaler/` checkout)

| Repo | Role | Wiki |
|------|------|------|
| **BRRTRouter** | OpenAPI-first HTTP router, validation, codegen | [`llmwiki/`](../index.md) (this tree) |
| **Lifeguard** | Coroutine ORM + `lifeguard-migrate` | [`lifeguard/docs/llmwiki/`](../../../lifeguard/docs/llmwiki/) |
| **Hauliage** | Microservices + BFF consumer of both | [`hauliage/docs/llmwiki/`](../../../hauliage/docs/llmwiki/) |

## How agents use the three

- **Transport / contract / 415 / OpenAPI extensions** → BRRTRouter wiki + [`reference/openapi-extensions.md`](../reference/openapi-extensions.md).
- **Entity DDL, migrations, UUID/chrono, pool** → Lifeguard wiki.
- **Service `impl/main.rs` registration, seeds, BFF** → Hauliage wiki.

## Cross-references

- [`topics/brrtrouter-integration-pitfalls.md`](../../../lifeguard/docs/llmwiki/topics/brrtrouter-integration-pitfalls.md) (Lifeguard-side stack symptoms)
