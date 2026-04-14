# PRD: Typed Handlers and REST-Compliant HTTP Status Codes

**Project:** BRRTRouter  
**Document version:** 1.0  
**Date:** 2026-04-13  
**Status:** Partially implemented (core runtime + `HttpJson`; see §11)  
**Related:** [OpenAPI 3.1.0 Compliance Gap](../OPENAPI_3.1.0_COMPLIANCE_GAP.md), [Request Lifecycle](RequestLifecycle.md)

---

## Implementation snapshot (2026-04)

| Item | State |
|------|--------|
| `HandlerResponseOutput` trait + blanket for `Serialize` (`src/typed/core.rs`) | Shipped |
| `HttpJson<T>` for explicit status + JSON body (`src/typed/core.rs`) | Shipped |
| Shared `typed_handler_output_to_response` used by `spawn_typed`, `spawn_typed_with_stack_size_and_name`, `register_typed_with_pool` | Shipped |
| Unit tests in `typed::core::tests` | Shipped |
| Integration test `test_spawn_typed_http_json_status_without_panic` in `tests/typed_tests.rs` | Shipped |
| OpenAPI multi-status codegen, `components.responses` $ref, 204/HEAD helpers | Not started (see §12) |

---

## Table of contents

1. [Executive summary](#1-executive-summary)
2. [Problem statement](#2-problem-statement)
3. [Goals and non-goals](#3-goals-and-non-goals)
4. [Current architecture (audit)](#4-current-architecture-audit)
5. [Issues catalog](#5-issues-catalog)
6. [Requirements](#6-requirements)
7. [Design options (for review)](#7-design-options-for-review)
8. [OpenAPI and response validation](#8-openapi-and-response-validation)
9. [Implementation call sites](#9-implementation-call-sites)
10. [Risks and constraints](#10-risks-and-constraints)
11. [Deliverables](#11-deliverables)
12. [Long-term deliverables](#12-long-term-deliverables)
13. [Success metrics](#13-success-metrics)
14. [Open questions](#14-open-questions)
15. [References](#15-references)

---

## 1. Executive summary

Typed handlers (`brrtrouter::typed::Handler`, `#[handler]`, `spawn_typed*`) deserialize requests and convert the handler return value via [`HandlerResponseOutput`]. **Plain [`serde::Serialize`] return types still map to HTTP `200`** with a JSON body. **`HttpJson<T>`** allows an explicit status (e.g. **404**, **201**) **without panicking** (see implementation snapshot above).

**This PRD** originally framed the gap when every success path was forced to **200**; the runtime now supports non-200 via `HttpJson`. Remaining work includes **204 / HEAD**, **OpenAPI multi-status codegen**, and **ergonomic error helpers** (§12).

**This PRD** also catalogs code locations, requirements, and phased deliverables—including generator and migration guidance for consumers (e.g. hauliage `*_gen` crates).

---

## 2. Problem statement

| Stakeholder need | Gap (after `HttpJson`) |
|------------------|------------------------|
| Return **404** when a resource ID does not exist | Use **`HttpJson::new(404, body)`** (or `not_found`). No longer requires panic for a JSON error body. |
| Return **201** on create, **204** on delete/update-without-body | **201**: use `HttpJson::new(201, body)`. **204** / empty body: not yet first-class (see §12). |
| Distinguish **client** (4xx) vs **server** (5xx) without panic | **4xx**: `HttpJson`. **5xx** without panic: same; true server faults may still use panic → **500** where `catch_unwind` applies. |
| Match **OpenAPI** `responses` per status code | Wire status in Rust via `HttpJson`; **codegen** that emits per-status types is still future work. |

**Historical root cause:** The `Handler` trait used `type Response: Serialize` and the runtime always mapped to **HTTP 200**. **Resolved in core** by `HandlerResponseOutput` + `HttpJson`; plain `Serialize` types remain **200** for backward compatibility.

**What already works:** `HandlerResponse` (`src/dispatcher/core.rs`) already carries arbitrary `status`, `headers`, and `body`. `AppService` (`src/server/service.rs`) already uses `hr.status` for writing the wire response and for **per-status JSON Schema validation** (`response_body_schema_for_status`).

---

## 3. Goals and non-goals

### 3.1 Goals

1. Allow typed handlers to produce **arbitrary HTTP status codes** on the non-exceptional path (e.g. **201**, **204**, **404**, **409**) **without panicking**.
2. Keep **request** validation behavior: failed `TryFrom<HandlerRequest>` continues to yield **400** with a clear JSON error where applicable.
3. Preserve **response body validation** against OpenAPI when the spec defines a schema for the emitted status.
4. **Deduplicate** response-building logic so `spawn_typed`, `spawn_typed_with_stack_size_and_name`, and `register_typed_with_pool` do not drift.
5. Provide a **migration path** for generated services and hand-written `#[handler]` controllers.

### 3.2 Non-goals (for initial phases)

- Replacing `HandlerResponse` or the core HTTP write path (`write_handler_response`).
- Changing OpenAPI parser behavior for unrelated features (see [OPENAPI_3.1.0_COMPLIANCE_GAP.md](../OPENAPI_3.1.0_COMPLIANCE_GAP.md) for separate tracks).
- Guaranteeing automatic mapping from domain errors to HTTP status without explicit handler decisions (optional later via helpers).

---

## 4. Current architecture (audit)

| Layer | Behavior |
|-------|----------|
| **`HandlerResponse`** (`src/dispatcher/core.rs`) | `status`, `headers`, `body: serde_json::Value`. Helpers: `new`, `json(status, body)`, `error(status, message)`. |
| **`AppService`** (`src/server/service.rs`) | Uses **`hr.status`** for status line and for **`response_body_schema_for_status(route, hr.status)`** before `write_handler_response`. |
| **Request deserialize failure** | Typed paths: **`HandlerResponse::error(400, …)`** when `TryFrom<HandlerRequest>` fails. |
| **Handler panic** | In `spawn_typed*` loops: caught → **500** JSON. **Verify** parity for `register_typed_with_pool` (worker closure may differ). |
| **Typed success path** | Handler return value → `serde_json::to_value` → **`HandlerResponse::json(200, body)`** or **`HandlerResponse { status: 200, … }`**. |

---

## 5. Issues catalog

| ID | Issue | Detail |
|----|--------|--------|
| **T1** | Success path always **200** | After `handler.handle(typed_req)`, the runtime forces status **200** for any successful return. |
| **T2** | **`Handler` trait is status-blind** | `type Response: Serialize` encodes no HTTP status. |
| **T3** | **`#[handler]` macro** | Assumes a single Serialize return type; no status dimension. |
| **T4** | **Duplicated “STEP 4”** | Same serialize → **200** logic in **`spawn_typed`**, **`spawn_typed_with_stack_size_and_name`**, and **`Dispatcher::register_typed_with_pool`**. Minor inconsistency: `HandlerResponse::json(200, …)` vs raw struct (equivalent after headers). |
| **T5** | **OpenAPI validation** | Non-2xx bodies require matching **`responses[status]`** in the spec if strict validation is desired; otherwise validation may be skipped for that status. Mismatched body vs schema can force **500** (“Response validation failed”). |
| **T6** | **204 No Content** | Typed handlers always produce a JSON value; **204** typically implies empty body—may need explicit support. |
| **T7** | **Panic vs typed error** | Panics → **500**; pool path should be audited for equivalent `catch_unwind` behavior. |

---

## 6. Requirements

### 6.1 Functional

- **FR1:** Handlers can return a success payload with an HTTP status **other than 200** without panicking.
- **FR2:** Handlers can return **error** responses (4xx/5xx) with a JSON body consistent with OpenAPI when declared.
- **FR3:** Existing handlers that today return **`Serialize` + implicit 200** remain supported until deprecated (backward compatibility).
- **FR4:** Unit and integration tests cover at least: **201**, **204** (if supported), **404**, and **422** or **400** where applicable.

### 6.2 Non-functional

- **NFR1:** No significant regression on hot-path allocations (JSF alignment); document trade-offs if a wrapper type adds overhead.
- **NFR2:** Single internal helper for “build `HandlerResponse` from handler output” shared by all typed registration paths (**T4**).
- **NFR3:** Public API and book/docs updated (`src/typed/mod.rs`, README, migration note).

---

## 7. Design options (for review)

*Pick one primary approach during review; hybrid possible.*

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| **A** | `Result<SuccessBody, HttpError>` as response type | Idiomatic Rust | `Serialize` on `Result` is awkward; may need newtype or custom serialize |
| **B** | Wrapper struct, e.g. `HttpJson<T> { status: u16, body: T }` | Explicit status + typed body | Codegen must emit wrapper; verbose call sites |
| **C** | Trait `IntoHandlerResponse` / `TypedResponse` | Flexible conversions | More traits and impls to maintain |
| **D** | Response trait with `fn status(&self) -> u16` on success body | Minimal JSON shape change | Easy to forget; not type-safe for status |
| **E** | OpenAPI-first: generated **enum** per operation covering `(status, body)` variants | Matches spec closely | Large generated surface |

**Recommendation for engineering review:** choose **B** or **C** for clarity and testability, then implement **one** internal `fn typed_output_to_handler_response(...)` used by all three call sites (**T4**).

---

## 8. OpenAPI and response validation

- `AppService` already validates response bodies **when** `response_body_schema_for_status(route, hr.status)` returns a schema (`src/server/service.rs`).
- Handlers that emit **404** with a JSON body should declare **`404`** under `operation.responses`** with the appropriate `content` / schema if validation is required.
- If a status is **not** listed in the spec, validation for that status may be **skipped** (current behavior)—document for API authors.

---

## 9. Implementation call sites

| # | Location | Action |
|---|----------|--------|
| 1 | `src/typed/core.rs` — `Handler` trait | Extend or replace with a return type that carries status + body (or equivalent). |
| 2 | `src/typed/core.rs` — `spawn_typed` inner closure | Replace fixed **200** with helper mapping. |
| 3 | `src/typed/core.rs` — `spawn_typed_with_stack_size_and_name` inner closure | Same as (2). |
| 4 | `src/typed/core.rs` — `Dispatcher::register_typed_with_pool` closure | Same as (2); ensure **panic behavior** matches spawn paths if required. |
| 5 | `brrtrouter_macros/src/lib.rs` — `#[handler]` | Wire new return types / trait impls. |
| 6 | Generator (`src/generator/`) | Optional: emit response types aligned with OpenAPI multiple response codes. |
| 7 | `tests/typed_tests.rs`, integration tests | New cases for non-200 success and 4xx bodies. |
| 8 | `src/typed/mod.rs` | Module-level documentation update. |
| 9 | Downstream consumers | Migration guide (e.g. hauliage `*_gen`, impl controllers). |

---

## 10. Risks and constraints

- **Breaking API:** Changing `Handler` or `#[handler]` may require a **semver major** or feature-gated path.
- **Response validation:** Stricter alignment with OpenAPI may **surface** previously silent schema mismatches as **500** validation failures.
- **Pool vs spawn:** Different error/panic handling between worker pool and coroutine loops could confuse operators—**normalize behavior** under this PRD.
- **204 / empty body:** May require an explicit branch in `write_handler_response` or typed mapping—avoid sending JSON `null` for **204** unless documented.

---

## 11. Deliverables

### Phase 1 — Design sign-off (no code)

- [ ] Approved **design option** (§7) and backward-compatibility strategy.
- [ ] Decision on **semver** (major vs feature flag `typed_status` or similar).
- [ ] Audit note: **`register_typed_with_pool`** panic handling vs `spawn_typed*`.

### Phase 2 — Core runtime

- [x] New type(s) / trait(s): **`HandlerResponseOutput`**, **`HttpJson<T>`**.
- [x] Internal **`typed_handler_output_to_response`** used by **all** typed paths (**T4**).
- [x] Implementation in **`spawn_typed`**, **`spawn_typed_with_stack_size_and_name`**, **`register_typed_with_pool`**.
- [x] Unit tests in **`typed::core::tests`**; integration tests in **`tests/typed_tests.rs`** (including **`HttpJson`** / **404**).

### Phase 3 — Macro and docs

- [x] **`#[handler]`** error text references **`HandlerResponseOutput`** / **`HttpJson`** (compile-time shape unchanged).
- [x] **`src/typed/mod.rs`** module docs updated.
- [ ] **Migration guide** for hand-written controllers (before/after examples) — optional follow-up.

### Phase 4 — Ecosystem

- [x] **Changelog** entry under Unreleased.
- [ ] Optional: generator updates to emit typed multi-status responses from OpenAPI.

---

## 12. Long-term deliverables

These extend the core feature set and are **explicitly out of scope** for the minimum shippable version unless pulled forward by product need.

| # | Deliverable | Description |
|---|-------------|-------------|
| **L1** | **OpenAPI-driven multi-status codegen** | For each operation, generate Rust types covering **all** declared `responses` (2xx/4xx/5xx), not only the default success schema—aligned with [OPENAPI 3.1.0 COMPLIANCE_GAP.md](../OPENAPI_3.1.0_COMPLIANCE_GAP.md) where relevant. |
| **L2** | **First-class `components.responses` $ref resolution** | Today some component-level response references are incomplete (see gap doc §3); resolving them improves codegen and validation for multi-status operations. |
| **L3** | **Ergonomic domain-error mapping** | Helpers such as `not_found()`, `conflict()`, `unprocessable()` building validated JSON error bodies from shared types—optional `thiserror` / `ProblemDetails` style. |
| **L4** | **204 / empty / HEAD semantics** | Explicit support for **no-content** responses, `Content-Length`, and HEAD without allocating JSON bodies. |
| **L5** | **Observability** | Span attributes for **http.response.status_code** on typed paths without relying on panic strings; consistent logging for 4xx vs 5xx. |
| **L6** | **Worker pool parity** | Unified policy: panic handling, backpressure, and **identical** HTTP semantics between pooled and non-pooled typed handlers. |
| **L7** | **Client SDK alignment** | If BRRTRouter or sibling projects generate clients, align generated **discriminated unions** or **result types** with server-side multi-status responses. |
| **L8** | **Lint / CI rules** | Optional linter warning when OpenAPI declares **404** but handler never emits it (best-effort static check on stubs). |
| **L9** | **Training & examples** | Extend **pet store** example with **201/404** routes; add cookbook section in **BRRTRouter_OVERVIEW** or **RequestLifecycle**. |

---

## 13. Success metrics

| Metric | Target |
|--------|--------|
| Handlers can return **non-200** without panic | 100% of covered integration tests |
| No duplicated STEP 4 logic | Single helper used by spawn + pool paths |
| Downstream migration documented | At least one external-style crate migration notes (e.g. hauliage pattern) |
| Regressions | Zero increase in **500** rate on pet store CI for unchanged behaviors |

---

## 14. Open questions

1. Should **default** success status be **200** or inferred from OpenAPI **default** response for the operation (e.g. **201** for POST)?
2. How strictly should **breaking changes** to `Handler` be avoided vs a **v2** typed module?
3. Should **500** from response body validation remain the behavior when OpenAPI schema mismatches, or should handlers opt into **soft** validation?
4. Pool workers: should panics be **caught** identically to `spawn_typed*`?

---

## 15. References

### Source files (audit snapshot)

- `src/dispatcher/core.rs` — `HandlerResponse`
- `src/typed/core.rs` — `Handler`, `spawn_typed`, `spawn_typed_with_stack_size_and_name`, `register_typed_with_pool`
- `src/server/service.rs` — dispatch, response validation, `write_handler_response`
- `src/server/response.rs` — `write_handler_response`, `write_json_error`
- `brrtrouter_macros/src/lib.rs` — `#[handler]`
- `tests/typed_tests.rs` — typed spawn behavior

### Related documents

- [Consumer migration: `HttpJson` and panic replacement](./MIGRATION_TYPED_HANDLER_HTTP_STATUS.md)
- [OPENAPI 3.1.0 Compliance Gap](../OPENAPI_3.1.0_COMPLIANCE_GAP.md)
- [Request Lifecycle](RequestLifecycle.md)
- [JSF Compliance](JSF_COMPLIANCE.md) (hot-path allocation constraints)

---

*End of PRD*
