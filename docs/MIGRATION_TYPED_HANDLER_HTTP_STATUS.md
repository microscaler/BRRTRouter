# Migration: typed handlers and REST status codes (`HttpJson`)

**Audience:** Services using `brrtrouter::typed::Handler`, `#[handler]`, and `TypedHandlerRequest<T>`.

## Background

- **`Serialize` return types** still map to **HTTP 200** with a JSON body (backward compatible).
- **`HttpJson<T>`** (`brrtrouter::typed::HttpJson`) sets **status** and **JSON body** without panicking.

## Replacing `panic!` for errors

**Before:**

```rust
if let Err(e) = do_work() {
    panic!("work failed: {e}");
}
Response::default()
```

**After:**

```rust
use brrtrouter::typed::HttpJson;
use serde_json::json;

match do_work() {
    Ok(()) => HttpJson::ok(json!({})),
    Err(e) if is_not_found(&e) => HttpJson::not_found(json!({ "errors": [e.to_string()] })),
    Err(e) => HttpJson::new(500, json!({ "errors": [e.to_string()] })),
}
```

Adjust the handler return type to **`HttpJson<serde_json::Value>`** (or a single `Serialize` type if all branches share one type).

## OpenAPI

Add **`responses`** entries for each status you emit (e.g. **404**, **500**) so strict response validation can succeed when enabled.

## See also

- [`PRD_TYPED_HANDLER_HTTP_STATUS.md`](./PRD_TYPED_HANDLER_HTTP_STATUS.md)
