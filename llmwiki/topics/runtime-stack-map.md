# Runtime stack map (spec → router → dispatcher → service)

- **Status**: `verified`
- **Source docs**: [`flows/runtime-request-flow.md`](../flows/runtime-request-flow.md), [`reference/codebase-entry-points.md`](../reference/codebase-entry-points.md)
- **Code anchors**: `src/spec/load.rs`, `src/spec/build.rs`, `src/router/core.rs`, `src/dispatcher/core.rs`, `src/server/service.rs`, `src/server/http_server.rs`
- **Last updated**: 2026-04-17

## What it is

BRRTRouter turns an **OpenAPI 3.1** document into a **route table** (`RouteMeta`), **JSON Schema** validators, and **may** coroutine handlers. The HTTP server funnels requests through **middleware**, **parameter extraction**, **request body parsing** (see [`entities/request-body-parsing.md`](../entities/request-body-parsing.md)), **validation gates** (see [`topics/schema-validation-pipeline.md`](./schema-validation-pipeline.md)), then the typed handler.

## Layered reading order

1. Load + build routes: `src/spec/load.rs`, `src/spec/build.rs`
2. Match path: `src/router/core.rs`
3. Dispatch to coroutine: `src/dispatcher/core.rs`
4. Service pipeline (validation, response serialization): `src/server/service.rs`

## Cross-references

- [`flows/runtime-request-flow.md`](../flows/runtime-request-flow.md)
- [`entities/route-meta.md`](../entities/route-meta.md)
- Lifeguard ORM (downstream data layer): [`../../../lifeguard/docs/llmwiki/`](../../../lifeguard/docs/llmwiki/)
