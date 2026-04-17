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
