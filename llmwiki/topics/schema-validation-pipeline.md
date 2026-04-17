# Topic: Schema validation pipeline

- **Status**: verified
- **Source docs**: `docs/RequestLifecycle.md`, `docs/ARCHITECTURE.md`
- **Code anchors**:
  - `src/server/service.rs::<AppService as HttpService>::call` — the end-to-end pipeline
  - `src/validator_cache.rs` — compiled-validator cache keyed by (handler, "request"|"response", status, schema digest)
  - `src/spec/build.rs::extract_request_body_details` — declared content-types + request schema
  - `src/spec/build.rs::extract_response_schema_and_example` — response schemas by status

## What this covers

Every pre-handler and post-handler check BRRTRouter performs to validate input and output against the OpenAPI schema. The steps are numbered V1a, V1, V2, V6, V7 (plus S1–S7 for security, covered separately). This page is the master sequence and fire-order.

## Lifecycle (request in → handler → response out)

```
parse_request                     (server::request)
  │
  ▼
route match                        (router::core / radix)
  │
  ▼
┌─────────────── per-route gates, in this order ───────────────┐
│ S1–S7  security (API key / JWT / OAuth2)                     │
│ §V1a   Content-Type declared? (415 if not)                   │
│ §V2    Required body missing? (400)                          │
│ §V1    Request schema validation (400 on failure)            │
└──────────────────────────────────────────────────────────────┘
  │
  ▼
dispatcher → handler coroutine    (dispatcher::core)
  │
  ▼
┌───────── post-handler gate ──────────┐
│ §V6/V7 Response schema validation    │
│        (500 on failure — server-side │
│         contract break)              │
└──────────────────────────────────────┘
  │
  ▼
response write                    (server::response)
```

## Per-step details

### §V1a — Content-Type enforcement (NEW 2026-04-17)

- **Fires when**: request body size > 0 AND `RouteMeta.request_content_types` is non-empty.
- **Checks**: client's `Content-Type` (primary part, post-`;`-split) against the declared content-types in `request_content_types`.
- **Failure**: HTTP **415 Unsupported Media Type** with `Accept-Post` response header listing declared types and JSON body:
  ```json
  { "error": "Unsupported Media Type", "message": "…", "accepted": ["application/json"] }
  ```
- **Why it exists**: closes the multipart bypass where `multipart/form-data` was silently fabricated to `{}` and passed §V1. See [`entities/request-body-parsing.md`](../entities/request-body-parsing.md) "Historical gap".
- **Code anchor**: `src/server/service.rs` — block starting `// V1a: Content-Type enforcement`.
- **Back-compat**: operations with no declared `requestBody` (empty `request_content_types`) skip §V1a entirely — preserves the old behavior for GET / DELETE with accidental bodies.

### §V2 — Required body missing

- **Fires when**: `RouteMeta.request_body_required` is `true` AND `body.is_none()`.
- **Failure**: HTTP **400 "Request body required"**.
- **Runs after §V1a** deliberately: a multipart request to a JSON-only operation must be rejected as 415 (media type), not 400 (body missing), because the body is semantically present — it's just in an unsupported form.

### §V1 — Request schema validation

- **Fires when**: `RouteMeta.request_schema.is_some()` AND `body.is_some()`.
- **Uses**: `validator_cache.get_or_compile(handler_name, "request", None, schema)` — pre-compiled at startup via `precompile_schemas`, looked up by cache key.
- **Checks**: `jsonschema::iter_errors(&body)` → full error list (not fail-fast).
- **Failure**: HTTP **400 "Request validation failed"** with an array of error strings (one per schema violation). Invalid field names are extracted from the error messages and logged for telemetry.
- **Succeeds silently**: zero errors → pipeline continues to dispatcher.

### §V6 / §V7 — Response schema validation

- **Fires when**: after the handler returns a `HandlerResponse`, and the matched response (by status + content-type from `RouteMeta.responses`) has a schema.
- **Uses**: same `validator_cache` but keyed by `("response", status)`.
- **Failure**: HTTP **500 "Response validation failed"** with an array of error strings. This is deliberately 5xx because it represents a server-side contract break — the impl returned data that doesn't match what the OpenAPI spec promises to clients.
- **When it fires in practice**: DB drift (stored value not in OpenAPI enum — see fleet `type = 'Type'` incident 2026-04-17), impl bugs returning wrong shapes, gen-stub mismatch.

## Compiled validator cache

- **File**: `src/validator_cache.rs`
- **Shape**: `Cache<Key, Arc<JsonSchema>>`, keyed by:
  ```
  Key { handler_name: Arc<str>, kind: "request" | "response", status: Option<u16>, schema_digest: [u8; 32] }
  ```
  The `schema_digest` keys a specific schema content, so identical schemas across routes share a compiled validator.
- **Precompilation**: at service startup, `precompile_schemas(&routes)` iterates every route and compiles request + response schemas. The fleet pod startup log shows `[startup] precompiled 12 JSON schema validators` for 8 routes — 12 = 5 GETs (responses only) + 3 POST/PUTs × 2 (request + response) + 1 DELETE (response) + a handful of inherited error schemas.
- **Cold path**: `get_or_compile` still supports on-demand compilation if a key misses — used by hot-reloaded routes.

## What this pipeline does NOT do (gaps)

1. **Does not parse `multipart/form-data` into a JSON shape.** §V1a rejects multipart if undeclared; if an operation declares multipart, the body becomes `None` and §V2 fires. True multipart parsing into the request JSON Value is a future enhancement.
2. **Does not validate query parameters against the operation's `parameters` schema.** Parameter decoding happens in `server::request::decode_param_value` against the parameter's individual schema, but a missing required query param is only caught by the handler's `TryFrom<HandlerRequest>` — not by the pipeline here.
3. **Does not enforce OpenAPI `format:` beyond what `jsonschema` enforces by default.** `format: date`, `format: uuid`, etc. are advisory unless the crate's format assertion is enabled.
4. **Does not run response validation when the impl returns status codes not declared in `responses`.** Unknown statuses bypass §V6/V7 silently. (Could be tightened to 500 "Undeclared response status".)

## Related

- [`entities/request-body-parsing.md`](../entities/request-body-parsing.md) — how `body` is constructed per Content-Type.
- [`entities/route-meta.md`](../entities/route-meta.md) — all the fields this pipeline reads.
- [`reference/openapi-extensions.md`](../reference/openapi-extensions.md) — `x-*` extensions that alter pipeline behavior (stack size, CORS, SSE).
- Hauliage ADR 0016 "Three-Layer Defense-in-Depth" — layer 1 of the three (OpenAPI schema) is exactly this pipeline.
