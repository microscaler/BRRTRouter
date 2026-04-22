# BRRTRouter LLM Wiki Index

## Core

- [Schema](./SCHEMA.md)
- [Log](./log.md)
- [Docs Catalog](./docs-catalog.md)

## Reconciliation

- [Core Docs vs Codebase](./reconciliation/core-docs-vs-codebase.md)
- [CORS Operations vs Codebase](./reconciliation/cors-operations-vs-codebase.md)
- [Performance Docs vs Codebase](./reconciliation/performance-docs-vs-codebase.md)

## Functional flows

- [Runtime Request Flow](./flows/runtime-request-flow.md)
- [Code Generation Flow](./flows/code-generation-flow.md)

## Reference

- [Codebase Entry Points](./reference/codebase-entry-points.md)
- [OpenAPI `x-*` Extensions](./reference/openapi-extensions.md)

## Entities

- [RouteMeta](./entities/route-meta.md)
- [Request body parsing](./entities/request-body-parsing.md)

## Topics

- [Schema validation pipeline](./topics/schema-validation-pipeline.md)
- [Auto-research perf loop](./topics/auto-research-perf-loop.md) — cron / background perf iterations; charter in `auto-research/docs/`
- [Runtime stack map](./topics/runtime-stack-map.md) — spec → router → dispatcher → `server/service`
- [Generator CLI and Askama](./topics/generator-cli-and-askama.md) — `brrtrouter_gen`, templates, Hauliage codegen pointers
- [Sibling repos and wikis](./topics/sibling-repos-and-wikis.md) — Lifeguard + Hauliage + this repo
- [Canonical docs vs WIP](./topics/canonical-docs-vs-wip.md) — how to treat `docs/wip/` vs maintained docs

## PRDs (active)

- [Hot-path v2 — stability & perf](../docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md) — stops per-response `Box::leak`, unbounded metrics keys, `RwLock` on hot path; adds Goose v2 harness. Targets Hauliage dev-env reboot cadence.

## Cross-references

- **Lifeguard ORM / migrations:** [`../../lifeguard/docs/llmwiki/`](../../lifeguard/docs/llmwiki/)
- **Hauliage services / BFF:** [`../../hauliage/docs/llmwiki/`](../../hauliage/docs/llmwiki/)
