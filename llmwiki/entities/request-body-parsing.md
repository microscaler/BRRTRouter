# Entity: Request body parsing

- **Status**: verified
- **Source docs**: `docs/RequestLifecycle.md`
- **Code anchor (primary)**: `src/server/request.rs` — `parse_request_body` (private), `parse_request` (public)
- **Code anchor (consumers)**: `src/server/service.rs::call` §V1a, §V1, §V2
- **History**: multipart bypass closed 2026-04-17 (see "Historical gap" below)

## What it is

`parse_request_body(raw: &[u8], content_type: &str) -> Option<Value>` is the sole entry point that converts the raw HTTP body bytes into the `serde_json::Value` that BRRTRouter's request validator, handler request parser, and downstream logic all consume. It is called from `parse_request`, which builds a `ParsedRequest { method, path, headers, cookies, query_params, body }`.

**Key invariant**: the value `body: Option<Value>` is the same value consumed by the schema validator at `src/server/service.rs` §V1. If it's `None`, validation is skipped. If it's `Some(Value::Object({}))`, validation runs against an empty object.

## Content-Type × body shape matrix

Derived from `src/server/request.rs::parse_request_body` (post-2026-04-17 fix):

| Client `Content-Type` | Parser branch | Returned body | Notes |
|---|---|---|---|
| `application/json` | `serde_json::from_slice(raw).ok()` | `Some(Value)` if parse succeeds, `None` on parse failure | `.ok()` swallows parse errors — bad JSON ⇒ `None` ⇒ §V1 skipped, §V2 fires if body required |
| `application/<foo>+json` | Same as `application/json` | Same | Catches `application/vnd.api+json`, `application/ld+json`, etc. |
| `application/x-www-form-urlencoded` | `form_urlencoded_body_to_json(raw)` | `Some(Value::Object(…))` — always | Form pairs → JSON object. Repeated keys collapse. |
| `multipart/form-data` | **`None`** ← **since 2026-04-17 fix** | `None` | See "Historical gap" below. §V1a 415 in `service.rs` handles the rejection. |
| *unknown / no Content-Type* | `serde_json::from_slice(raw).ok()` (fallback) | Best-effort JSON parse | Liberal fallback preserved for clients that send JSON without a Content-Type header |

## Downstream decision points in `service.rs::call`

After `parse_request` produces `ParsedRequest`, the dispatcher makes five body-related decisions in sequence:

| Step | Condition | Outcome |
|---|---|---|
| **§V1a** Content-Type enforcement | `body_size_bytes > 0` ∧ `route.request_content_types.iter().all(|t| t != client_content_type)` | **415 Unsupported Media Type** with `Accept-Post` header listing declared types |
| **§V2** Required body missing | `route.request_body_required` ∧ `body.is_none()` | **400 "Request body required"** |
| **§V1** Request schema validation | `route.request_schema.is_some()` ∧ `body.is_some()` | Validate via `validator_cache`; **400 "Request validation failed"** with error list on fail |
| (dispatch) | All gates pass | Coroutine channel to handler |
| **§V6/V7** Response schema validation | Response schema present on matched status | **500 "Response validation failed"** on fail (server-side schema contract break) |

§V1a runs **before** §V2 deliberately: a client sending `multipart/form-data` to an operation that only declares `application/json` should get a media-type rejection (415), not a body-missing rejection (400), even though internally the `None` body would trigger §V2 if allowed through.

## Historical gap — multipart fabrication (pre-2026-04-17)

Before the 415 fix, the multipart branch in `parse_request_body` was:

```rust
if ct_lower == "multipart/form-data" {
    return Some(json!({}));
}
```

This fabricated an empty `{}` JSON object for every `multipart/form-data` request regardless of what the operation actually accepted. Consequences:

- `multipart/form-data` → `Some(Value::Object({}))` → §V1 request validation ran against `{}`.
- If the operation declared required fields, §V1 caught it → 400 (safe by accident).
- If the operation declared no required fields (or all fields were optional), §V1 passed `{}` → handler received an empty struct from serde deserialization → wrote garbage / defaults to the DB.
- Any operation that only declared `application/json` accepted `multipart/form-data` silently — **the bypass**.

The 2026-04-17 fix:

1. `parse_request_body` multipart branch changed from `Some(json!({}))` to `None`.
2. `RouteMeta` gained `request_content_types: Vec<String>` populated from `requestBody.content` keys.
3. New §V1a in `service.rs::call` rejects unmatched Content-Type with 415 before §V2 / §V1.

See also:
- BRRTRouter commit `feat(server): reject undeclared Content-Type with HTTP 415`
- Hauliage ADR 0016 §1 for the security rationale
- [`topics/schema-validation-pipeline.md`](../topics/schema-validation-pipeline.md) for the end-to-end V1a–V7 sequence

## Gotchas

1. **`.ok()` swallows JSON parse errors.** Malformed JSON under `application/json` becomes `body = None`. That's intentional — §V2 or §V1 handle the downstream decision — but is surprising. Clients that care about error clarity should validate JSON client-side.
2. **`multipart/form-data` is not actually parsed.** Even if an operation declares `multipart/form-data` in its `requestBody.content`, BRRTRouter does not parse the multipart payload into fields yet. Operations relying on multipart today would fail §V1 against their schema because `body` is `None`. File-upload operations need a future enhancement in this module.
3. **Unknown Content-Type falls back to JSON parse.** A client sending `text/plain` with a JSON payload will still succeed through §V1 (confirmed by test in the 415-fix commit). This is loose-by-design for interoperability, but it means `Content-Type` is not a strict gate unless the route declares `request_content_types`.

## Code anchors (repo-relative)

- `src/server/request.rs` — `parse_request_body` (lines 250–273 post-fix), `parse_request` (line 287+)
- `src/server/service.rs` — §V1a Content-Type enforcement, §V2 Required body missing, §V1 Schema validation
- `src/validator_cache.rs` — compiled validator cache keyed by (handler_name, "request"/"response", optional status, schema digest)
