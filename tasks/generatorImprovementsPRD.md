## BRRTRouter Generator & Templates Improvements PRD

### Objective
Deliver a robust, warning-free, deterministic code generation system (Askama-based) that produces compile-ready example services from OpenAPI specs, supports safe hot-reload, and is fully covered by tests and formatting gates.

### Scope
- Generator orchestrator in `src/generator/project/generate.rs`
- Askama templates in `templates/`
- Schema processing utilities in `src/generator/schema.rs`
- Example app generation under `examples/<slug>`
- Hot-reload integration points (generation → runtime registration)

### Non‑Goals
- Implement business logic in controllers beyond example data
- Replace `may`/runtime model
- Introduce WebSockets or OpenAPI validation features (tracked separately)

### Current State Summary
- End-to-end generation works but exhibits intermittent issues:
  - Occasional Askama template output with unused imports → warnings in generated example
  - No canonicalization guard when copying spec (risk of self-copy/truncation)
  - Force/partial regeneration UX is coarse (all or skip)
  - Example literal generation can panic on mismatched schemas
  - Limited OpenAPI format/enum handling; arrays/objects mostly handled
  - Hot-reload regenerates files inconsistently and is not atomically applied to runtime
  - Limited generator unit tests for edge-cases; coverage not enforced for generator path

### Problems & Gaps
1) Template hygiene: unused imports and conditionally unused blocks create 25+ warnings in example crate.
2) Generation safety: risk of copying spec onto itself and partial writes on error.
3) Regeneration control: cannot selectively regenerate (handlers, controllers, types) or dry-run.
4) Example robustness: example-driven literals may `unwrap()` and panic; error handling is not graceful.
5) Schema fidelity: OpenAPI `format` (int64, float), `enum`, `additionalProperties` not mapped precisely.
6) Hot-reload: file regeneration not atomic; dispatcher not updated live; no integration test for live route add.
7) Determinism: module/import ordering not explicitly enforced; file churn possible.
8) Observability: no version/hash of generator embedded for traceability.
9) Tests/coverage: insufficient unit tests around generator edge-cases; coverage gate not applied to generator code path.

### Functional Requirements
1) Template Hygiene
- Conditionalize template imports/usings to only emit when needed per file.
- Generated example crate compiles with zero warnings by default.

2) Spec Copy Safety
- Before copying, canonicalize source/destination and no-op if identical; never truncate.
- Emit clear logs of copy source/target.
- Copy the input spec unchanged to `examples/<slug>/doc/openapi.yaml`; honor `--force` for overwrite.

3) Regeneration Controls
- Support flags: `--force`, `--only=handlers|controllers|types|registry|main|docs`, `--dry-run`.
- Dry-run prints intended file operations without writing.

4) Robust Example Literal Generation
- Never panic during example-to-literal conversion; on mismatch, fall back to `Default::default()` with a comment.
- Preserve optionality without double-wrapping `Option`.
 - Ensure arrays/objects examples convert safely; avoid parsing fallbacks that can panic.
 - Arrays of named types import only the types actually used in the file.

5) Schema Fidelity
- Respect OpenAPI `format`: map `int32|int64|float|double` appropriately.
- Support `enum` as Rust enums (when feasible) or validated string literals otherwise.
- Support `additionalProperties` → `HashMap<String, T>`.
 - Honor `required` vs optional semantics from schemas; apply sensible defaults when absent.
 - Treat `nullable` if present by mapping to `Option<T>` consistently.

6) Hot‑Reload Friendly Generation
- Generate to a temp dir and atomically move into place on success.
- Provide an API to diff routes (added/removed/updated) for dispatcher updates.

7) Deterministic Output
- Consistently sorted modules/imports; stable file content ordering.
- Use a single lock file to track generator version and per-file content hashes; do not modify file headers.

8) Registry & Dispatcher Integration
- Registry template should avoid wildcard imports; import only used symbols.
- Prefer a single authoritative path: generate dispatch table keyed by handler name.
- Minimize or encapsulate `unsafe` in public registration helpers.

9) CLI & Developer UX
- `cargo run --bin brrtrouter-gen -- generate --spec <path> [flags]` prints a concise summary of created/updated/skipped files.
- Respect repository guidance: after regeneration, `cargo fmt` and `cargo test` must pass.

### Non‑Functional Requirements
- Generation time: for a 100‑endpoint spec, complete within ≤2s on a modern laptop (cold run ≤5s).
- Zero panics: generator must not panic under malformed but parseable specs; return actionable errors.
- Test coverage: ≥65% immediately, target 80% on generator and schema code.
- Deterministic: repeated runs without changes produce identical outputs.

### Templates: Specific Requirements
- `templates/handler.rs.txt`
  - Import `decode_param_value`, `ParameterStyle`, `anyhow` only when parameters exist or required.
  - Add `#[serde(default)]` to `Request` when any optional fields exist.
  - Error messages for missing required params include location, style, and explode (when set).
  - Import named types (`handlers::types::*`) only when present; omit block when empty.

- `templates/controller.rs.txt`
  - Conditional imports block; render only on non-empty.
  - Optional feature-gate to return `Result<Response, ValidationError>` in future validation mode.
  - SSE example remains minimal; include comment on heartbeat/backpressure.
  - Import named types only when present; omit block when empty.

- `templates/registry.rs.txt`
  - Remove wildcard imports; import only controller symbols used.
  - Encapsulate or document `unsafe` usage and provide a safe wrapper when possible.
  - Prefer `register_from_spec` as single source of truth.

- `templates/mod.rs.txt`, `templates/handler_types.rs.txt`, `templates/main.rs.txt`
  - Enforce deterministic ordering and avoid emitting empty modules/blocks.

### Generator Orchestration Requirements
- Add pre‑write validation step: render to string then run a “lint” pass (e.g., simple checks for empty struct bodies, duplicate defs) before writing.
- Provide `--only` scoping and `--dry-run` log with a diff‑like summary.
- Add atomic write strategy: write to `*.tmp`, fsync, then rename.
- Produce a deterministic lock file (e.g., `examples/<slug>/.brrtrouter-lock.json`) containing generator version, template versions, and a map of relative file paths → content hashes; avoid embedding version/hash in file headers.

### Validation (Request & Response) Requirements

#### Objectives
- Invalid requests must never reach controllers. Fail fast with clear errors.
- Bad controller responses are detected, logged, and mapped to appropriate HTTP errors.
- Debug mode controls verbosity of error responses while always logging full details server-side.

#### Functional Requirements
- Request validation
  - Validate JSON request body against route request schema when present.
  - Honor OpenAPI `requestBody.required`; return 400 if missing when required.
  - Validate parameters (path/query/header/cookie) against their schemas and styles (including `style` and `explode`) before dispatch.
  - On validation error: log details; return 400 with Problem Details body; in debug mode include validation paths/messages.
  - Ensure controller is not invoked on invalid requests.

- Response validation
  - Validate handler response body against selected response schema/content-type (e.g., application/json).
  - Default policy: invalid response → 500 (server error); optional strict-contract mode can return 400. Always log full details.
  - Set Content-Type from spec when missing in handler response.

- Error response shape (Problem Details)
  - Use application/problem+json structure: `type`, `title`, `status`, `detail`, `instance`, and `errors` (array of details).
  - Include `method`, `path`, and `handler` in logs; include only when in debug for client responses.

- Configuration
  - `BRRTR_DEBUG_VALIDATION` (bool): toggles verbose client error bodies for validation failures.
  - `BRRTR_STRICT_RESPONSE_VALIDATION` (bool): if true, map invalid controller responses to 400 instead of 500.

- Tracing & Metrics
  - Emit tracing events for request/response validation results (success/failure) within request/response spans.
  - Metrics counters: `request_validation_failed_total`, `response_validation_failed_total` with labels (route, handler, reason).

#### Non‑Functional Requirements
- No panics: replace `expect("invalid ... schema")` with error handling; return 500 with terse body (verbose in debug) and log the cause.
- Performance: validation overhead should not increase p50 latency > 2ms on example app.
- Deterministic: same inputs produce identical validation outcomes.

#### Testing & Validation
- Unit tests
  - Missing required body → 400.
  - Bad body shape → 400 with details; controller not executed.
  - Parameter style/explode decoding failures → 400; verify decoding paths.
  - Invalid response body → 500 by default; 400 when strict mode enabled.

- Integration tests
  - Ensure invalid request never reaches controller (detect via side-effect or counter).
  - Verify debug flag toggles verbose/terse response bodies while logs always contain full details.
  - Validate content-type selection and response validation across multiple status codes.

#### Acceptance Criteria
- Invalid requests are blocked pre-dispatch with 400 Problem Details in debug and terse errors otherwise; controller not invoked.
- Invalid controller responses produce logs and 500 (or 400 in strict mode) Problem Details in debug; terse otherwise.
- Tracing spans and metrics reflect validation failures.
- No `expect` panics in validation paths; graceful error handling throughout.

### Dispatcher Improvements

#### Objectives
- Improve resilience, observability, and fairness in request dispatch while keeping middleware semantics clear and hot-reload friendly.

#### Functional Requirements
- Timeouts & Backpressure
  - Add configurable per-request timeout for handler responses (env: `BRRTR_DISPATCH_TIMEOUT_MS`). On timeout: log, increment metric, return 504 (or 500 configurable).
  - Prefer bounded channels per handler (configurable capacity) to avoid unbounded growth; define policy when full (block/reject/drop-oldest) via env.

- Error Handling & Observability
  - Log and trace on missing handler, send failure, receive failure, or timeout. Include handler, method, path, and latency.
  - Metrics counters for dispatch failures by reason: `dispatch_missing_handler_total`, `dispatch_send_fail_total`, `dispatch_recv_fail_total`, `dispatch_timeout_total`.

- Middleware Semantics
  - Short-circuit `before` chain: once a `before` returns a response, do not call subsequent `before` middlewares.
  - Ensure `after` is invoked only for the path taken (early response or handler response), with measured latency.

- Safe API Surface
  - Provide safe registration helpers so example code and templates avoid `unsafe` at call sites; encapsulate unsafety internally.

- Hot‑Reload Friendliness
  - Provide an atomic swap API to replace the handler map with minimal downtime; gracefully close old channels.

- Concurrency Limits
  - Optional per-handler limit (semaphore) to cap in-flight requests; configurable via env or spec extension.

- Cancellation
  - Optional early-cancel if client disconnects; plumb cancellation signal from server to dispatcher.

#### Non‑Functional Requirements
- No hangs: `dispatch` must not block indefinitely on `recv`; obey timeout policy.
- Performance: timeout checks and bounded channels must not add >1ms p50 overhead under normal load.
- Deterministic: handler selection and middleware ordering remain stable.

#### Testing & Validation
- Unit tests
  - Timeout returns configured status and increments metrics; handler eventually responds is ignored.
  - Bounded channel behavior under load (block/reject/drop); correctness of policy.
  - Middleware short-circuit behavior: later `before` not executed; `after` called with early response.
  - Missing handler, send/recv failures produce logs and metrics.

- Integration tests
  - End-to-end timeout behavior with a slow handler.
  - Hot-reload swap while requests in flight does not crash and routes new requests correctly.

#### Acceptance Criteria
- Dispatcher enforces request timeout and reports failures via logs, tracing, and metrics.
- Middleware short-circuit is well-defined and tested.
- Safe registration APIs exist; generated registry avoids `unsafe` for users.
- Bounded channels or equivalent backpressure strategy implemented and validated.

### Testing & Validation
- Unit tests:
  - Parameter extraction across path/query/header/cookie with style/explode variations.
  - Example literal generation for nested objects, arrays, and mismatched types (no panic).
  - Import pruning: ensure no unused imports when no named types.
  - Enum/format mapping correctness.
  - `additionalProperties` mapping to `HashMap`.

- Integration tests:
  - Full project generation; `cargo fmt` + `cargo check` pass.
  - `--dry-run` produces accurate summary; no writes.
  - `--only` regenerates targeted files and preserves others.
  - Hot‑reload path: generate to temp, swap, then dispatch diff applied (future PRD ties to runtime work).

- Quality gates:
  - `cargo fmt` and `cargo clippy -D warnings` on generated example.
  - Coverage via `cargo llvm-cov` with fail‑under 65% initially, 80% target.

### Success Criteria & Acceptance
- Generated example compiles with zero warnings (including imports) on first run.
- `just gen` (or cargo command) followed by `cargo fmt`, `cargo test` passes consistently.
- Generator never panics on valid OpenAPI docs; returns actionable errors.
- Dry‑run and `--only` work as documented; atomic write prevents partial outputs.
- Templates produce deterministic output; re-run without changes yields identical files. Only the lock file may change when appropriate (e.g., generator version bump without content deltas).
- Unit/integration tests added to cover edge cases; coverage ≥65%.
- The exact input spec is present at `examples/<slug>/doc/openapi.yaml` after generation (unless skipped by non-`--force` rule).
 - Generated `handlers/types.rs` correctly reflects component schemas including `enum` and `additionalProperties` maps; handlers/controllers compile using these types without warnings.

### Milestones
1) Hygiene & Safety (imports, spec copy guard, atomic writes)
2) Regeneration UX (dry-run, only flags, summary output)
3) Example robustness & schema fidelity (no panics, formats, enums, maps)
4) Determinism & metadata (ordering, version/hash in headers)
5) Tests & coverage (unit + integration, gates)
6) Hot‑reload preparedness (temp gen + diff API; runtime follow-up in separate PRD)

### Risks & Mitigations
- Risk: Template churn breaking examples → Mitigate with integration test asserting zero warnings.
- Risk: Performance regressions on large specs → Add basic timing in tests and budget alerts.
- Risk: Over‑eager strictness (deny unknown fields) breaking users → Make strict modes opt‑in via flags.

### Operational Workflow
- Regeneration command: `cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force`
- Repository guidance: After regeneration, run `cargo fmt` and `cargo test -- --nocapture`
- CI: Add llvm‑cov gate (≥65%), clippy `-D warnings`, and integration job to generate + build example.

### Progress Checklist

#### 1) Template Hygiene
- [ ] Conditional imports in handler template (only when params used)
- [ ] Conditional imports in controller template (only when non-empty)
- [ ] Zero warnings in generated example crate by default

#### 2) Spec Copy Safety
- [ ] Canonicalize spec source/target, avoid self-copy
- [ ] Clear logs for copy source/target
- [ ] Fallback behavior when copy fails (actionable error)
- [ ] Copy spec to `examples/<slug>/doc/openapi.yaml` honoring `--force`

#### 3) Regeneration Controls
- [ ] Implement `--dry-run` (no writes, summary only)
- [ ] Implement `--only=handlers|controllers|types|registry|main|docs`
- [ ] Human-readable summary of created/updated/skipped files

#### 4) Robust Example Literal Generation
- [ ] Remove all `unwrap()` in example conversion path
- [ ] Graceful fallback to `Default::default()` with comment on mismatch
- [ ] Preserve `Option<T>` semantics without double-wrapping
 - [ ] Arrays/objects conversion paths are panic-free
 - [ ] Arrays of named types only import used names

#### 5) Schema Fidelity
- [ ] Map OpenAPI `format` → Rust types (int32/int64/float/double)
- [ ] Support `enum` (Rust enum or validated literals)
- [ ] Support `additionalProperties` → `HashMap<String, T>`
 - [ ] Honor `required` and `nullable` semantics consistently

#### 6) Hot‑Reload Friendly Generation
- [ ] Generate into temp dir, fsync, atomic rename on success
- [ ] Expose route diff (added/removed/updated) API for dispatcher
- [ ] Add integration test scaffold (runtime wiring in separate PRD)

#### 7) Deterministic Output
- [ ] Sorted modules and imports for stability
- [ ] Generate lock file with generator/template versions and per-file hashes; no per-file header hashes
- [ ] Repeat runs without changes yield identical outputs

#### 8) Registry & Dispatcher Integration
- [ ] Remove wildcard imports from registry template
- [ ] Encapsulate or document `unsafe` and provide safe wrapper
- [ ] Prefer `register_from_spec` as single source of truth

#### 9) CLI & Developer UX
- [ ] Concise summary output after generation
- [ ] Respect repo guidance: auto-run `cargo fmt` (configurable) or provide instruction
- [ ] Helpful error messages with file and template context

#### 10) Testing & Validation
- [ ] Unit tests: parameter extraction (all locations/style/explode)
- [ ] Unit tests: example conversion (nested objects/arrays, mismatches)
- [ ] Unit tests: import pruning (no unused imports)
- [ ] Unit tests: enum/format/additionalProperties mapping
 - [ ] Unit tests: arrays of refs/named types and import pruning
 - [ ] Unit tests: serde round-trip for generated enums
- [ ] Integration: full project generation builds + formats
- [ ] Integration: `--dry-run` summary correctness
- [ ] Integration: `--only` scoped regeneration behavior
- [ ] Quality gates: clippy `-D warnings` on generated example
- [ ] Coverage: `cargo llvm-cov` ≥65% (target 80%)

#### 11) Success Criteria
- [ ] Zero-warning example on first generation
- [ ] No panics; actionable errors only
- [ ] Deterministic output; identical re-runs without changes
- [ ] All tests (unit/integration) pass with coverage ≥65%

#### 12) Validation
- [ ] Request body validation against schema; honor `requestBody.required`
- [ ] Parameter validation pre-dispatch (path/query/header/cookie with style/explode)
- [ ] Problem Details error responses; debug-mode verbosity toggle
- [ ] Response validation policy (default 500; strict mode 400) with logs
- [ ] Replace `expect` with graceful error handling; no panics
- [ ] Tracing events and metrics counters for validation failures

#### 13) Dispatcher Improvements
- [ ] Per-request timeout (env-configurable) with 504/500 response and metrics
- [ ] Bounded channels/backpressure policy (block/reject/drop-oldest)
- [ ] Logs/tracing/metrics for missing handler, send/recv failures, timeouts
- [ ] Short-circuit `before`; consistent `after` invocation and latency capture
- [ ] Safe registration helpers (avoid `unsafe` in templates)
- [ ] Atomic handler map swap for hot-reload; graceful old channel close
- [ ] Optional per-handler concurrency limit
- [ ] Optional cancellation on client disconnect

#### 14) Middleware Improvements
- [ ] Short-circuit semantics for `before`; `after` invoked with final response
- [ ] CORS: credentials, exposed headers, max-age, Origin-aware behavior
- [ ] Tracing: enrich spans (route, handler, status, error flag)
- [ ] Metrics: Prometheus counters/histograms (requests, durations, failures)
- [ ] AuthMiddleware: clarify example-only or parse Bearer; prefer server security
- [ ] Middleware stack builder ergonomics
- [ ] Unit/integration tests covering CORS, tracing, metrics, short-circuit


