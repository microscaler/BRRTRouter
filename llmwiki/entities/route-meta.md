# Entity: RouteMeta

- **Status**: verified
- **Source docs**: `docs/ARCHITECTURE.md`, `docs/RequestLifecycle.md`
- **Code anchor (primary)**: `src/spec/types.rs`
- **Code anchor (population)**: `src/spec/build.rs`
- **Code anchor (consumers)**: `src/server/service.rs`, `src/router/core.rs`, `src/router/radix.rs`, `src/validator_cache.rs`

## What it is

`RouteMeta` is the single, hot-path record BRRTRouter holds for every OpenAPI operation. Every incoming request is matched to exactly one `RouteMeta`, and every piece of downstream logic — dispatch, schema validation, Content-Type enforcement, CORS, security, middleware, metrics — reads its decisions from this struct. It is `Clone` (uses `Arc<str>` for path/handler interning) so the hot path pays O(1) to clone, not O(n).

## Field catalog

Every field listed with: *what it is*, *populated from where*, *consumed by whom*.

| Field | Type | Populated from (OpenAPI → Rust) | Consumed by |
|---|---|---|---|
| `method` | `http::Method` | `operation.method_str` via `Method::from_bytes` | `router::core`, `router::radix` (HTTP method matching) |
| `path_pattern` | `Arc<str>` | Path key of the operation (`/vehicles/{id}`) | `router` (regex / radix match) |
| `handler_name` | `Arc<str>` | `x-handler` extension or derived from `operationId` via `resolve_handler_name` | `dispatcher::core` (channel keying) |
| `parameters` | `Vec<ParameterMeta>` | `item.parameters ∪ operation.parameters`, resolved + flattened by `extract_parameters` | Request parser in `server::request`, handler `TryFrom<HandlerRequest>` impls |
| `request_schema` | `Option<Value>` | `requestBody.content."application/json".schema` via `extract_request_body_details` | `server::service::call` §V1 (request validation) |
| `request_body_required` | `bool` | `requestBody.required` (default `false`) | `server::service::call` §V2 (400 "Request body required") |
| `request_content_types` | `Vec<String>` | Keys of `requestBody.content` (e.g. `["application/json", "multipart/form-data"]`). Added 2026-04-17 for 415 enforcement. See [`entities/request-body-parsing.md`](./request-body-parsing.md). | `server::service::call` §V1a (415 Unsupported Media Type) |
| `response_schema` | `Option<Value>` | Default response (200 `application/json`), via `extract_response_schema_and_example` | `server::service::call` §V6/V7 (response validation) |
| `example` | `Option<Value>` | `responses[<status>].content.<ct>.example` (first found, 200 preferred) | Gen-stub controllers (serve as mock data when no impl is wired — see [`topics/register-and-overwrite-lifecycle.md`](../topics/register-and-overwrite-lifecycle.md)) |
| `responses` | `Responses` (map status→content-type→response) | Every declared `responses.*` entry | Gen-stub codegen; response validation (selects schema by status + content-type) |
| `security` | `Vec<SecurityRequirement>` | `operation.security` with fallback to spec-level `security` | `server::service::call` §S1–S7 (security gate) |
| `example_name` | `String` | Codegen slug derived from `handler_name` | Template rendering only |
| `project_slug` | `String` | Filename-safe slug from `info.title` | Codegen output paths |
| `output_dir` | `PathBuf` | CLI `--out` argument | Codegen only |
| `base_path` | `String` | `info.contact.url` path portion, or CLI `--base-path` | Router prefix matching |
| `sse` | `bool` | `x-sse: true` extension | Handler-type dispatch (Server-Sent Events shape) |
| `estimated_request_body_bytes` | `Option<usize>` | `estimate_body_size(request_schema)` at build time | `RequestLogger.total_size_bytes` fallback when `Content-Length` header is absent |
| `x_brrtrouter_stack_size` | `Option<usize>` | `x-brrtrouter-stack-size` or alias `x-stack-size` extension | `dispatcher::core` (per-route coroutine stack override) |
| `cors_policy` | `RouteCorsPolicy` | `x-cors` extension merged with top-level `x-brrtrouter-cors` via `extract_route_cors_config` | `middleware::cors` |
| `x_service` | `Option<String>` | `x-service` extension (set by `brrtrouter_tooling` BFF merger) | Proxy controller downstream routing |
| `x_brrtrouter_downstream_path` | `Option<String>` | `x-brrtrouter-downstream-path` extension (set by BFF merger) | Proxy controller URL construction |

See [`reference/openapi-extensions.md`](../reference/openapi-extensions.md) for the full `x-*` catalog with enforcement semantics.

## Methods (non-field surface)

- `content_type_for(status: u16) -> Option<String>` — first content-type declared for a response status (used to pick the `Content-Type` header on the outgoing response).

## Where RouteMeta is constructed

The canonical constructor is **not exposed** — there is no public `RouteMeta::new`. It is assembled inline by `spec::build::build_routes` (iterating `paths → methods` in the OpenAPI spec) and returned as a `Vec<RouteMeta>` plus the collected security schemes. Test fixtures (`src/router/tests.rs`, `src/router/radix.rs`, `src/router/performance_tests.rs`, `src/generator/stack_size.rs`, `src/validator_cache.rs`, `tests/server_tests.rs`) construct literal `RouteMeta { … }` values with sensible defaults.

## Gotcha: adding a field

Adding a field to `RouteMeta` touches **~7 files** because of test fixtures. The mechanical diff shape:

1. `src/spec/types.rs` — add the field with rustdoc.
2. `src/spec/build.rs` — populate in `build_routes` (and update `extract_*` helper if applicable).
3. Test fixtures — add the field to six literal constructors: `src/router/tests.rs`, `src/router/radix.rs`, `src/router/performance_tests.rs`, `src/generator/stack_size.rs`, `src/validator_cache.rs` (3 occurrences), `tests/server_tests.rs`.

This happened 2026-04-17 for `request_content_types` — the commit landing that field (`feat(server): reject undeclared Content-Type with HTTP 415`) touches exactly these files and is a useful reference diff.

## Cross-references

- `RouteMeta.request_content_types` → §V1a 415 enforcement → [`entities/request-body-parsing.md`](./request-body-parsing.md) and [`topics/schema-validation-pipeline.md`](../topics/schema-validation-pipeline.md).
- `RouteMeta.x_service` / `x_brrtrouter_downstream_path` → BFF proxy routing, consumed by generated passthrough controllers; authored by `brrtrouter_tooling.workspace.bff.generate_system` in Python.
- Hauliage ADR 0016 "Three-Layer Defense-in-Depth for Entity Invariants" lists OpenAPI → schema-field layer 1 assertions that land in `request_schema` here.
