# Schema validation pipeline (runtime)

**PRD:** [`PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](../../PRD_HOT_PATH_V2_STABILITY_AND_PERF.md) — **Phase 4 (validator fast path)** targets optimizations along this pipeline. This page is the map of **where** validation happens today.

## Separation of concerns

| Layer | Role |
|-------|------|
| **`crate::validator`** | OpenAPI **document** validation at load/spec-build time — not per-request body validation. |
| **`server::request::parse_request` / `parse_request_body`** | Syntax: raw bytes + `Content-Type` → `serde_json::Value` (or form URL-encoded map). No JSON Schema. |
| **`ValidatorCache`** | Compile once, reuse: `jsonschema::Validator` keyed by spec version, handler, request vs response, status, schema digest. |
| **`AppService::call`** | Orchestrates security, **415** (Content-Type vs declared `request_content_types`), **400** (required body, request schema), dispatch, then response schema (**500** on handler output mismatch). |
| **`dispatcher`** | Consumes an already-parsed `body: Option<Value>`; does **not** run `jsonschema` on the request path. |

## Request path (order)

1. **Route match** — radix + terminal method table (`Router`).
2. **Security** — JWT / API key etc. (fail → 401/403 with RFC7807 body).
3. **V1a — Content-Type** — If the operation declares `requestBody.content` types and the client sent a non-empty body, the request’s `Content-Type` must match a declared type or **415** (`server/service.rs`, “V1a”).
4. **V2 — Required body** — If `request_body_required` and `body` is `None` → **400**.
5. **V1 & V3 — Request JSON Schema** — If the route has `request_schema` and `body` is `Some`:
   - `validator_cache.get_or_compile(handler, "request", None, schema)`
   - `compiled.iter_errors(body_val)`; any error → **400** with details.
6. **Dispatch** — `dispatcher.load().dispatch_with_request_id(...)` with validated body.

**Parsing note:** `parse_request_body` returns `None` for `multipart/form-data` so V1a can reject wrong media types instead of fabricating `{}` (see comment in `server/request.rs`).

## Code anchors (read these first)

| Area | File / symbol |
|------|----------------|
| Cached validators | [`src/validator_cache.rs`](../../../src/validator_cache.rs) — `ValidatorCache::get_or_compile`, cache keys, `BRRTR_SCHEMA_CACHE` |
| Hot-path orchestration | [`src/server/service.rs`](../../../src/server/service.rs) — `AppService::call`, blocks labelled **V1a**, **V2**, **V1 & V3** |
| Body parse | [`src/server/request.rs`](../../../src/server/request.rs) — `parse_request_body`, `parse_request`, `primary_content_type` |
| Spec-level validation | [`src/validator.rs`](../../../src/validator.rs) — not the per-request pipeline |

## Phase 4 — optimizations (in progress)

Measure before claiming macro wins: harness noise is ~15% on stress tests ([`PRD_HOT_PATH_V2` §Phase 6](../../PRD_HOT_PATH_V2_STABILITY_AND_PERF.md)).

**Implemented in `AppService::call`:**

- **DEBUG logging:** `required` is logged as `?schema.get("required")` (cheap `Option<&Value>`) instead of allocating a `Vec<String>` of required field names on **every** validated request.
- **Bounded error collection:** `iter_errors(...).take(64)` for request and response bodies — caps CPU and allocations on pathological invalid JSON; `details` in 400/500 may truncate (constant `MAX_JSON_SCHEMA_ERRORS` in `server/service.rs`).

**Still optional / future:**

- **Known shapes:** trivial-schema fast paths (higher risk; profile first).
- **Cache:** ensure startup `precompile_schemas` covers hot routes so the first real request never pays compile on the latency tail.
- **Microbench:** [`benches/schema_validation_hot_path.rs`](../../../benches/schema_validation_hot_path.rs) — compare with `--baseline ms02` after changes.

## See also

- [`bench-harness-phase-6.md`](./bench-harness-phase-6.md)
- Crate rustdoc overview in [`src/lib.rs`](../../../src/lib.rs)
