# `brrtrouter-gen` CLI and Askama templates

- **Status**: `verified`
- **Source docs**: [`flows/code-generation-flow.md`](../flows/code-generation-flow.md)
- **Code anchors**: `src/bin/brrtrouter_gen.rs`, `src/generator/`, `src/generator/templates.rs`, `templates/*.txt` (Askama templates for gen crates)
- **Last updated**: 2026-04-17

## What it is

The **code generator** renders Rust crates (handlers, controllers, registry) from OpenAPI using **Askama** templates. Orchestration lives under `src/generator/`; the CLI entrypoint is **`src/bin/brrtrouter_gen.rs`**.

Consumer repos (e.g. Hauliage) run generation as part of their **service scaffold** workflow — see Hauliage [`PRD_BFF_SCAFFOLDING_REMEDIATION.md`](../../../hauliage/docs/PRD_BFF_SCAFFOLDING_REMEDIATION.md) and [`hauliage/docs/llmwiki/topics/scaffolding-lifecycle.md`](../../../hauliage/docs/llmwiki/topics/scaffolding-lifecycle.md).

## Cross-references

- [`reference/openapi-extensions.md`](../reference/openapi-extensions.md)
- [`flows/code-generation-flow.md`](../flows/code-generation-flow.md)
