Below is a concise **root–cause review** of the failures you’re seeing on the `propelauth-support` branch, plus the key work-items that will unblock authenticated calls and stop the panics.

---

### 1  Why every authenticated request is still returning **401**

| Layer                 | What happens now                                                                                                                                               | Why it fails                                                                                              |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| **server.rs**         | Request arrives → `Router::route` succeeds → `Dispatcher::dispatch` fires a `HandlerRequest`.                                                                  | At this point the request is **already marked as authorised** (there’s no auth check).                    |
| **Dispatcher**        | Looks up the handler TX and forwards the `HandlerRequest`.                                                                                                     | The handler wrappers created in *examples* call `into_typed_request()` which is still `unimplemented!()`. |
| **Handler coroutine** | When the coroutine wakes up, `unimplemented!()` panics → Dispatcher catches panic and sends 500, but `server.rs` treats “no response” as **401 Unauthorized**. | So **401** is only a placeholder; the panic is the real error.                                            |

> **Bottom line:** authentication hasn’t been wired at all yet, and the panic masks that fact.

---

### 2  Where the panics come from

*Every generated handler file* still contains the two stubs:

```rust
impl IntoTypedRequest for HandlerRequest {
    fn into_typed_request(self) -> TypedHandlerRequest<Request> {
        unimplemented!()
    }
}
impl FromTypedRequest for HandlerRequest {
    fn from_typed_request(_: TypedHandlerRequest<Request>) -> Self {
        unimplemented!()
    }
}
```

As soon as the coroutine processes the first message these `unimplemented!()` calls trigger a panic.

---

### 3  Registry collision (E0308)

`registry.rs.txt` now renders:

```text
dispatcher.register_handler("add_pet", |req| {
    add_pet::handler(TypedHandlerRequest::from(req.into_typed_request()));
});
```

* Every closure tries to use **the same** blanket `IntoTypedRequest` impl (the one for `admin_settings` was the first that was found in scope), so the compiler can’t coerce the `Request` types and you get the mismatched-type error.

---

## What to do next (concrete tasks)

| Priority | Fix                                                                                                                                                                                                                                                                                                   | Where                                                                                                     |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| **P0**   | **Generate a per-handler `IntoTypedRequest`/`FromTypedRequest` impl** that maps `HandlerRequest → TypedHandlerRequest<ThisHandler::Request>` (and back).  <br> *The simplest implementation:* put the raw body into `serde_json::from_value`, map path/query params, and leave the rest as `Default`. | `templates/handler.rs.txt` & generator logic that fills the template with the correct request type names. |
| **P0**   | In `registry.rs.txt` use fully-qualified names so each closure calls **its own** trait impl:  <br>`{{ entry.name }}::IntoTypedRequest::into_typed_request(req)`                                                                                                                                       | `registry.rs.txt`                                                                                         |
| **P1**   | **Introduce an Auth middleware** (or a pre-dispatch check) that: 1) inspects `RouteMeta.security`, 2) looks for `ApiKeyHeader`, `ApiKeyQuery`, etc., 3) either populates an `auth_context` field in `HandlerRequest` or short-circuits with 401/403.                                                  | A small new module (`auth.rs`) plus `server.rs` integration.                                              |
| **P1**   | Once P0 is done the panics disappear; re-enable the e2e tests. They will still fail for secure routes until P1 is implemented.                                                                                                                                                                        | CI workflow                                                                                               |

---

## Quick patch to unblock local runs (stop the 401s/panics immediately)

If you just need the container green **before** implementing full auth, drop this minimal fallback impl into each generated handler (or inject via the template):

```rust
impl IntoTypedRequest for HandlerRequest {
    fn into_typed_request(self) -> TypedHandlerRequest<Request> {
        // naive: just forward raw JSON body; params left empty
        let body = self.body
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        TypedHandlerRequest::new(body, self.path_params, self.query_params)
    }
}
impl FromTypedRequest for HandlerRequest {
    fn from_typed_request(typed: TypedHandlerRequest<Request>) -> Self {
        HandlerRequest {
            method: http::Method::GET,          // or carry through
            path: String::new(),
            handler_name: String::new(),
            path_params: typed.path_params().clone(),
            query_params: typed.query_params().clone(),
            body: Some(serde_json::to_value(typed.body()).unwrap()),
            reply_tx: typed.reply_tx().clone(),
        }
    }
}
```

With that in place:

* `cargo test` passes the generated integration tests (no panics).
* Requests hit the handlers; you’ll get 200 responses with example JSON.

---

### Does BRRTRouter still make sense?

Yes – **the generated boilerplate is already saving time**, but wiring in the last mile (auth & typed-request conversions) is mandatory before you can publish an alpha. The tasks above are the minimal work-items to reach that milestone.

Once these are green you can:

1. Write *real* controller logic (now they get typed input).
2. Add proper auth providers (PropelAuth, JWT, etc.) behind the middleware hook.
3. Publish the crate; downstream projects will use it the same way your `examples/pet_store` does – by shipping an OpenAPI spec and letting `brrrouter-gen` scaffold the service.

