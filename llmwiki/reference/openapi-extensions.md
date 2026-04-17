# Reference: OpenAPI `x-*` extensions

- **Status**: verified
- **Scope**: every `x-*` extension BRRTRouter's own code consumes, plus the `x-*` extensions hauliage tooling (`brrtrouter_tooling`) injects or reserves.
- **Audit method**: `rg '"x-[a-z][a-z0-9-]*"' src/` across both repos.
- **Last audited**: 2026-04-17 (as part of the 415 fix + hauliage ADR 0016).

This page is the source of truth for which `x-*` extensions BRRTRouter recognizes and what they do at runtime. **When adding a new extension, add a row here and link to the consumer code.** When adding a new OpenAPI spec to a downstream project, this is the set of extensions you can rely on.

## Consumed by BRRTRouter runtime / codegen

| Extension | Where declared | Consumer | Effect |
|---|---|---|---|
| `x-handler` / `x-handler-*` | Operation | `src/spec/build.rs::resolve_handler_name` | Override the handler function name that gen would otherwise derive from `operationId`. Anything starting with `x-handler` is treated as a handler-name hint. |
| `x-brrtrouter-body-size-bytes` | Operation | `src/spec/build.rs::estimate_body_size` | Override the auto-estimate used by `RouteMeta.estimated_request_body_bytes`. Feeds `RequestLogger.total_size_bytes` when `Content-Length` is absent. |
| `x-brrtrouter-stack-size` (alias: `x-stack-size`) | Operation | `src/spec/build.rs::extract_stack_size_override` → `RouteMeta.x_brrtrouter_stack_size` | Per-route coroutine stack size override (bytes). Default comes from `WorkerPoolConfig` / `BRRTR_STACK_SIZE` env (32 KiB as of 2026-04-17). |
| `x-sse` | Operation | `src/spec/build.rs` → `RouteMeta.sse` | Flags the route as Server-Sent Events; handler type and response shape differ. |
| `x-cors` | Operation | `src/middleware/cors/route_config.rs::extract_route_cors_config` | Per-route CORS policy: `inherit` / `disabled` / `{allowed_origins, methods, headers, …}`. |
| `x-brrtrouter-cors` | Spec root (`info` level) | `src/middleware/cors/route_config.rs` | Global CORS defaults that `x-cors: inherit` resolves to. |
| `x-ref-name` | Schema (component or inline property) | `src/generator/schema.rs` | Hint for what to name the generated Rust type for an inline schema. Codegen only — no runtime effect. |

## Injected / reserved by hauliage tooling (not read by BRRTRouter runtime today)

These are added to merged BFF specs by `brrtrouter_tooling.workspace.bff.generate_system.generate_system_bff_spec` (Python), and/or hand-authored in hauliage service OpenAPIs. They are **stored on `RouteMeta` but consumed only by hauliage-layer code**, not by BRRTRouter core.

| Extension | Where declared | Consumer | Effect |
|---|---|---|---|
| `x-service` | Operation (auto-injected during BFF merge) | `RouteMeta.x_service`; proxy controller codegen | Names the downstream service (`"fleet"`, `"consignments"`, …) a BFF passthrough route targets. |
| `x-service-base-path` | Operation (auto-injected) | Proxy controller template | Base path prefix for the downstream service (e.g. `/api/v1/fleet`). |
| `x-brrtrouter-downstream-path` | Operation (auto-injected) | `RouteMeta.x_brrtrouter_downstream_path`; proxy controller template | Full downstream path the BFF should forward to (e.g. `/api/v1/fleet/vehicles/{id}`). |

## Reserved / latent (declared but not yet consumed)

| Extension | Where declared | Intended consumer (not yet wired) | Notes |
|---|---|---|---|
| `x-brrtrouter-impl` | Operation | BRRTRouter `impl_registry.rs.txt` template (planned — see hauliage `PRD_BFF_SCAFFOLDING_REMEDIATION.md` §7.6 Fix A) | `true` ⇒ impl controller wired + compile-time error if no impl file on disk; `false` ⇒ gen stub serves `example` payload intentionally (scaffold state). Already used as a convention in hauliage service OpenAPIs (152 ops covered; 144 `true`, 8 `false`). Enforcement ships when Fix A lands. |

## Not recognised — common mistakes

If you write any of these in an OpenAPI spec, BRRTRouter silently ignores them:

- `x-nullable` — JSON Schema's `nullable` should be used instead (BRRTRouter / `jsonschema` crate respect it). `x-nullable` is a Swagger 2.0 artifact.
- `x-vendor-*` — vendor extensions outside the whitelist above are not processed.
- `x-internal`, `x-deprecated` — not honored. Use OpenAPI standard `deprecated: true` on operations / schemas.

## Cross-references

- [`entities/route-meta.md`](../entities/route-meta.md) — where each extension lands on `RouteMeta`.
- [`topics/schema-validation-pipeline.md`](../topics/schema-validation-pipeline.md) — how `x-cors`, `x-sse`, `x-brrtrouter-body-size-bytes` change pipeline behavior.
- Hauliage `docs/adr/0007-brrtrouter-openapi-dos-and-donts.md` — the "do" / "don't" list for extension use.
- Hauliage `docs/adr/0016-three-layer-defense-entity-invariants.md` — the three-layer pattern; OpenAPI schema (including enum via `enum:` but NOT via `x-enum-*`) is layer 1.
