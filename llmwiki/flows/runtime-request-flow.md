# Runtime Request Flow

- Status: verified
- Source docs: `docs/ARCHITECTURE.md`, `docs/RequestLifecycle.md`, `docs/SecurityAuthentication.md`

## High-confidence flow (code-anchored)
1. OpenAPI spec is loaded into route metadata (`load_spec*`):
   - `src/spec/load.rs`
2. Route metadata is transformed into router structures (`Router::new`):
   - `src/router/core.rs`
3. Incoming requests are handled via `AppService` pipeline:
   - `src/server/service.rs`
4. Route matching occurs in router radix path:
   - `src/router/core.rs`
5. Handler dispatch uses coroutine channels and `HandlerRequest` / `HandlerResponse`:
   - `src/dispatcher/core.rs`
6. Response writing/validation and middleware metrics hooks execute in service layer:
   - `src/server/service.rs`

## Important runtime notes
- Hot-path modules deny some allocation-related clippy lints (`router/core.rs`, `dispatcher/core.rs`).
- Header and parameter collections use `SmallVec` for common-case stack allocation.
- Service owns optional metrics/memory middleware and security providers.

## Handler concurrency (2026-11-04)
- Each typed handler is served by one coroutine draining one channel (`spawn_typed_with_stack_size_and_name`
  in `src/typed/core.rs`); an untyped handler by `DEFAULT_HANDLER_CONCURRENCY` coroutines sharing one queue
  (`spawn_untyped_with_stack_size_and_name` in `src/dispatcher/core.rs`; see below). A request
  for a busy handler waits; `op_duration_seconds{component="brrtrouter.queue",op}` measures that wait
  (`src/timing.rs`, `QueueDepthGuard::queued_for`).
- `BRRTR_HANDLER_CONCURRENCY__<HANDLER>` (or `BRRTR_HANDLER_CONCURRENCY` for all) starts n coroutines
  for an **untyped** handler, each with a clone of the handler, sharing a `may` mpmc queue
  (`handler_concurrency`, `serve_untyped`). Default 2 (`DEFAULT_HANDLER_CONCURRENCY`, since
  2026-11-05); `1` restores one-at-a-time; clamped 1..=256. The generator's
  proxied routes are untyped. Typed handlers are unaffected.
- A handler must not rely on serving its requests one at a time to use it; shared state needs its own
  synchronisation. Tests: `tests/handler_concurrency_tests.rs`.
- Evidence and numbers: `docs/perf/copy-and-borrow-audit.md` pass 9.

> **Open:** the same setting for typed handlers (generated controllers are `#[derive(Clone, Copy)]`), and a
> spec extension instead of only environment variables.
