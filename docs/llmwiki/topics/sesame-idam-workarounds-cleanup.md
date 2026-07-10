---
title: Sesame-IDAM Workarounds — BRRTRouter Cleanup Tasks
status: verified
updated: 2026-07-07
---

# Sesame-IDAM Workarounds — BRRTRouter Cleanup Tasks

Implementation backlog for BRRTRouter changes that let **sesame-idam** and **hauliage** remove local workarounds. Sesame-side mirror: [`sesame-idam/docs/llmwiki/topics/topic-brrtrouter-refactor-backlog.md`](../../../../seasame-idam/docs/llmwiki/topics/topic-brrtrouter-refactor-backlog.md).

## Context

Sesame-IDAM auth implementation (2026-07-06) exposed four framework gaps. Phase 1 HTTP migration (`brrtrouter::http`) is **done** in this repo. Remaining items are codegen/runtime semantics — not a full rewrite.

---

## BR-1 — `security: []` means public (P1) ✅

**Status:** Landed 2026-07-06.

**Implementation:** `src/spec/security_presence.rs` scans raw JSON/YAML for explicit operation `security` keys before `oas3` deserialization erases the distinction. `load_spec` passes presence into `build_routes_with_security_presence`.

**Tests:** `tests/spec_security_tests.rs`, sesame `openapi_security.rs`.

**Unblocks:** sesame global security restored; hauliage mixed public/protected specs (**HI-5** audit — ✅ hauliage `16fae98`).

---

## BR-1b — In-cluster HTTP JWKS URLs (P1) ✅

**Status:** Landed 2026-07-06 (`085e67e`).

**Problem:** `JwksBearerProvider` rejected `http://*.svc.cluster.local` JWKS URLs → hauliage fleet pod crash-loop.

**Fix:** Allow HTTP JWKS fetch for Kubernetes cluster DNS suffixes.

**Unblocks:** hauliage HI-1 fleet JWKS in Kind.

---

## BR-1c — HTTP fetch path-only URI (P1) ✅

**Status:** Landed 2026-07-07 (`73744df`).

**Problem:** `fetch_get_http` / `fetch_post_http` passed the full URL as the HTTP path to `may_http`, breaking in-cluster JWKS GET from hauliage pods.

**Fix:** Parse URL; send `path + query` only to the client.

**Tests:** `tests/http_fetch_tests.rs`.

**Unblocks:** hauliage HI-3 E2E green; resolves **HI-9** fetch compile/runtime path.

---

## BR-2 — JWT claims in typed handlers (P2) ✅

**Status:** Landed 2026-07-10.

**Problem:** `TypedHandlerRequest<T>` conversion dropped `HandlerRequest::jwt_claims`. Principal-dependent endpoints could not use generated typed handlers.

**Fix:** `TypedHandlerRequest<T>` now includes `jwt_claims: Option<Value>`, populated in `spawn_typed`, `spawn_typed_with_stack_size_and_name`, and `register_typed_with_pool`.

**Tests:** `tests/typed_tests.rs::test_spawn_typed_preserves_jwt_claims`.

**Unblocks:** sesame **SI-3** — delete raw handler module for `/identity/me`, userinfo.

---

## BR-3 — Typed error HTTP status (P2) ✅ 2026-07-10

**Problem:** Typed handlers returning `Serialize` success types always map to HTTP 200. OAuth refresh failure in sesame returns empty `TokenResponse` at 200 instead of 401.

**Shipped:** Runtime `HttpJson<T>` existed; **codegen** now emits `HttpJson<Response>` when `RouteMeta::needs_http_json_return_type()` — operation has a non-2xx `application/json` response schema. Controller + impl stubs wrap success in `HttpJson::ok(...)`; `--sync` patches signature and return literal.

**Acceptance:**

- Operation with `401` + `ErrorResponse` schema can return 401 from typed handler without raw dispatch.

**Unblocks:** sesame **SI-4** — `auth_refresh` OAuth-compliant errors (consumer must declare `401` JSON schema in OpenAPI).

---

## BR-4 — Codegen `init_security` helper (P2)

**Problem:** Generated `main.rs` registers security providers from spec schemes; impl crates override `init_security` and can register a subset → runtime “Security provider not found”.

**Fix:** Emit `register_spec_security_providers(registry, spec)` from gen crate; impl calls it from `init_security` and adds custom providers only.

**Acceptance:** sesame login impl calls generated helper; no duplicate scheme names.

---

## BR-5 — JWKS refresh on `may::go!` (P3)

**Problem:** Background JWKS refresh still uses `std::thread` in `security/jwks_bearer/mod.rs` (and spiffe duplicate).

**Fix:** Replace with `may::go!` coroutine; shared via `fetch_get_text_with_retry`.

---

## BR-6 — Zero direct/transitive `reqwest` (P3)

**Status:** Production security providers migrated to `brrtrouter::http` (Phase 1, 2026-07-06). Remaining `reqwest` is OTEL HTTP exporter + optional jsonschema network.

**Fix:**

- Disable HTTP OTLP exporter feature; grpc-tonic only.
- Pin jsonschema without remote fetch if possible.
- Remove direct `reqwest` from `Cargo.toml` once tree is clean.

See sesame [`topic-http-client-policy.md`](../../../../seasame-idam/docs/llmwiki/topics/topic-http-client-policy.md).

---

## BR-7 — JWT validation sub-spans (P3)

**Problem:** Epic 9 Story 9.1 wants spans like `jwt.signature_verify` inside `JwksBearerProvider::validate_token()`. Today only handler-level authz spans exist.

**Fix:** Add `tracing` spans at validation steps inside provider; document span names in sesame observability wiki.

---

## Next staged work (Wave 2)

| Order | ID | Effort | Blocker for |
|-------|-----|--------|-------------|
| ~~1~~ | ~~**BR-1**~~ | ~~Small~~ | ✅ `a6aa511` |
| ~~2~~ | ~~**BR-1b**~~ | ~~Small~~ | ✅ `085e67e` |
| ~~3~~ | ~~**BR-1c**~~ | ~~Small~~ | ✅ `73744df` |
| 1 | **BR-4** | Small | Deploy smoke provider drift |
| 2 | **BR-2** | Medium | Raw handler removal (SI-3) |
| 3 | **BR-3** | Medium | OAuth status codes (SI-4) |
| 4 | **BR-5..BR-7** | Large | Platform hygiene |

**Consumer next:** hauliage Wave 3 — OpenAPI client, fleet ownership WIP, BR-2/BR-3.

> **Open:** BFF login to sesame takes ~10s (bcrypt); hauliage client uses 30s HTTP fetch timeout.

## Code anchors

| File | Relevance |
|------|-----------|
| `src/spec/build.rs` | BR-1 security inheritance |
| `src/typed/` | BR-2, BR-3 typed dispatch |
| `src/security/jwks_bearer/mod.rs` | BR-5, BR-7 |
| `src/http/fetch.rs` | BR-1c complete (`73744df`); path-only URI for may_http |

## Related

- [`topic-http-client-policy.md`](../../../../seasame-idam/docs/llmwiki/topics/topic-http-client-policy.md) (sesame)
- [`PRD_TYPED_HANDLER_HTTP_STATUS.md`](../../PRD_TYPED_HANDLER_HTTP_STATUS.md) (if present)
