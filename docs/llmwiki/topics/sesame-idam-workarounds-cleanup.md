---
title: Sesame-IDAM Workarounds — BRRTRouter Cleanup Tasks
status: verified
updated: 2026-07-06
---

# Sesame-IDAM Workarounds — BRRTRouter Cleanup Tasks

Implementation backlog for BRRTRouter changes that let **sesame-idam** and **hauliage** remove local workarounds. Sesame-side mirror: [`sesame-idam/docs/llmwiki/topics/topic-brrtrouter-refactor-backlog.md`](../../../../seasame-idam/docs/llmwiki/topics/topic-brrtrouter-refactor-backlog.md).

## Context

Sesame-IDAM auth implementation (2026-07-06) exposed four framework gaps. Phase 1 HTTP migration (`brrtrouter::http`) is **done** in this repo. Remaining items are codegen/runtime semantics — not a full rewrite.

---

## BR-1 — `security: []` means public (P1)

**Problem:** OpenAPI 3 defines `security: []` on an operation as *no authentication required*. Codegen currently treats empty operation security as “inherit spec-level global security”:

```rust
// src/spec/build.rs (~615)
let security = if !operation.security.is_empty() {
    operation.security.clone()
} else {
    spec.security.clone()
};
```

Because `oas3` deserializes both *omitted* and *explicit `[]`* to `Vec::default()` (empty), BRRTRouter cannot distinguish them today.

**Symptom:** In-cluster `POST /idam/v1/auth/login` returned 401 until sesame-idam removed global `security` from login/session specs (commit `26b4aba` on sesame-idam).

**Fix options (pick one):**

1. **oas3 + build.rs:** Use `Option<Vec<SecurityRequirement>>` with serde presence tracking (`#[serde(default, skip_serializing_if = "Option::is_none")]`) — `None` = inherit global, `Some([])` = public.
2. **Vendor extension:** `x-brrtrouter-public: true` on operations (fallback if oas3 change is too invasive).

**Acceptance:**

- Spec with global `security: [BearerAuth]` + operation `security: []` generates route with **no** auth middleware.
- Spec with global security + operation security **omitted** still inherits global.
- Unit test in `brrtrouter-gen` or build.rs tests with fixture YAML.

**Unblocks:** sesame **SI-1** — restore global security on IDAM specs.

---

## BR-2 — JWT claims in typed handlers (P2)

**Problem:** `TypedHandlerRequest<T>` conversion drops `HandlerRequest::jwt_claims`. Principal-dependent endpoints cannot use generated typed handlers.

**Sesame workaround:** `identity-session-service/impl/src/raw_handler.rs` — manual `spawn_raw_handler` + `authenticated_principal()`.

**Fix:** Extend typed dispatch to pass claims, e.g.:

- `AuthenticatedHandlerRequest<T>` with `claims: Option<JwtClaims>`, or
- Second parameter on handler trait: `fn handle(req: T, ctx: &HandlerContext)`.

**Acceptance:**

- Handler registered for `BearerAuth` route receives non-empty claims after successful validation.
- Existing handlers without auth unchanged.

**Unblocks:** sesame **SI-3** — delete raw handler module for `/identity/me`, userinfo.

---

## BR-3 — Typed error HTTP status (P2)

**Problem:** Typed handlers returning `Serialize` success types always map to HTTP 200. OAuth refresh failure in sesame returns empty `TokenResponse` at 200 instead of 401.

**Partial solution exists:** `HttpJson<T>` for explicit status (see hauliage PRD). Gap is **codegen** — generated handler stubs don't use `HttpJson` for multi-response OpenAPI operations.

**Fix:**

- Teach `brrtrouter-gen` to emit `HttpJson<T>` when operation defines non-2xx response schemas with bodies, or
- Document + enforce “auth error paths → raw handler or `HandlerResponse`” in sesame only.

**Acceptance:**

- Operation with `401` + `ErrorResponse` schema can return 401 from typed handler without raw dispatch.

**Unblocks:** sesame **SI-4** — `auth_refresh` OAuth-compliant errors.

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

## Next staged work (this repo)

| Order | ID | Effort | Blocker for |
|-------|-----|--------|-------------|
| 1 | **BR-1** | Small | sesame global security restore, hauliage mixed public/protected specs |
| 2 | **BR-4** | Small | Deploy smoke provider drift |
| 3 | **BR-2** | Medium | Raw handler removal |
| 4 | **BR-3** | Medium | OAuth status codes |
| 5 | **BR-5..BR-7** | Large | Platform hygiene |

## Code anchors

| File | Relevance |
|------|-----------|
| `src/spec/build.rs` | BR-1 security inheritance |
| `src/typed/` | BR-2, BR-3 typed dispatch |
| `src/security/jwks_bearer/mod.rs` | BR-5, BR-7 |
| `src/http/fetch.rs` | Phase 1 complete |

## Related

- [`topic-http-client-policy.md`](../../../../seasame-idam/docs/llmwiki/topics/topic-http-client-policy.md) (sesame)
- [`PRD_TYPED_HANDLER_HTTP_STATUS.md`](../../PRD_TYPED_HANDLER_HTTP_STATUS.md) (if present)
