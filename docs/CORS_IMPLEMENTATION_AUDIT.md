# CORS implementation audit — BRRTRouter

**Audience:** Framework maintainers and service teams integrating BRRTRouter  
**Scope:** `src/middleware/cors/` (`CorsMiddleware`, `CorsMiddlewareBuilder`, OpenAPI `x-cors` / `RouteCors*`, wiring in examples such as `examples/pet_store/`)  
**Status:** Implementation is **substantially complete** for common browser CORS flows; several areas remain **partially delivered** or **application-dependent** for strict production hardening.

> **Note:** `docs/wip/CORS_AUDIT.md` is a short pointer to this file and `[CORS_OPERATIONS.md](./CORS_OPERATIONS.md)`. **Treat this document as the canonical architectural audit.**

---

## 1. How CORS works today

### 1.1 Placement in the pipeline

- Middleware implements `before` / `after` (`src/middleware/core.rs`): `**before`** can short-circuit; `**after`** augments successful handler responses.
- **Order matters:** CORS is typically registered with other middleware (e.g. auth, metrics). Teams must document whether auth runs before or after CORS for `OPTIONS` and credentialed requests.

### 1.2 Global configuration

- `**CorsMiddlewareBuilder`** (`builder.rs`): fluent API for origins (exact, `*`, regex, custom validator), methods, headers, credentials, exposed headers, `Access-Control-Max-Age`.
- **Validation at build time:** rejects wildcard origin + credentials and empty exact origins + credentials (`CorsConfigError`).
- `**CorsMiddleware::default()`:** empty allowed origins (secure default); `**permissive()`** for dev (`*` origin, no credentials).

### 1.3 Route-level policy (OpenAPI `x-cors`)

- `**extract_route_cors_config`** (`route_config.rs`): reads `x-cors` per operation — `inherit`, `false` (disabled), or object (`allowedHeaders`, `allowedMethods`, `allowCredentials`, `exposeHeaders`, `maxAge` camelCase).
- **Origins are intentionally not** in OpenAPI: they are expected from **environment config** (e.g. `config.yaml`) and merged at startup in consuming apps (see pet_store example).

### 1.4 Request handling (`CorsMiddleware` — `mod.rs`)


| Concern                              | Behavior                                                                                                                                                                                                                                                                                                        |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Route disabled** (`x-cors: false`) | `OPTIONS` → short-circuit **200** with no CORS headers; other methods proceed, `**after` skips CORS**                                                                                                                                                                                                           |
| **OPTIONS, no `Origin`**             | Short-circuit **200**, no CORS headers (treated as non-CORS)                                                                                                                                                                                                                                                    |
| **Preflight detection**              | `OPTIONS` + `Origin` + `**Access-Control-Request-Method`** → preflight path; missing `ACRM` → **not** treated as preflight (`before` returns `None` so the handler can run). If `ACRM` is present but method/headers are invalid or not allowed → **403** (`CorsPreflightOutcome::Denied`), handler not invoked |
| **Actual requests**                  | Valid `Origin` → handler runs; `**after`** adds ACAO, ACAH, ACAM, optional credentials, expose-headers, `**Vary`** (see P1 §2)                                                                                                                                                                                    |
| **Invalid origin**                   | `**before`** returns **403** with no CORS headers (for requests that have `Origin` and fail validation)                                                                                                                                                                                                         |
| **Same-origin**                      | `**is_same_origin`** compares the request **`Origin`** to the **effective server authority**: default **`Host`**; if **`trust_forwarded_host`** is enabled, **`Forwarded`** (`host` / `proto`) then **`X-Forwarded-Host`** / **`X-Forwarded-Port`** (see `effective_server_authority`, `forwarded.rs`) — **skips** CORS headers when they match (scheme, host, port; IPv6-safe) |


### 1.5 Design strengths

- Single reflected origin per response (not comma-separated multi-origin strings).
- Wildcard + credentials guarded at build / `with_origins` time.
- Route-specific policies merged with global origins in **pet_store** (reference integration).
- JSF-oriented **startup-time** parsing: route map built once; hot path uses map lookup + string checks.

---

## 2. Partially delivered / gaps (prioritized)

### P0 — Correctness and security (address before high-risk production)

1. ~~**Preflight failure vs “not a preflight” conflation**~~ **(addressed)**
  `handle_preflight` now returns `**CorsPreflightOutcome`**: `**NotPreflight`** only when `Access-Control-Request-Method` is absent; `**Denied**` when a preflight is present but the method token is invalid, the method is not allowed, or a requested header is not allowed — `**before**` then returns **403** with JSON `{"error":"CORS preflight request denied"}` and no CORS success headers.
2. ~~`**CorsMiddlewareBuilder::build_with_routes` + Custom origins**~~ **(addressed)**
  `build_with_routes` now calls `**merge_route_policies_with_global_origins`** so `**RouteCorsPolicy::Custom`** routes receive the same origin policy as the built global middleware (exact/wildcard via `with_origins`, regex/custom copied from global). `**examples/pet_store**` uses the same helper instead of duplicating the merge loop. Manual wiring can call `**RouteCorsConfig::merge_global_origin_validation**` or `**merge_route_policies_with_global_origins**` when not using the builder.

### P1 — Production operations and proxies

1. ~~**Reverse proxies and `Host`**~~ **(addressed when configured)**
  Same-origin uses **effective server authority** vs `Origin` (§1.4). By default authority is **`Host`**; with **`trust_forwarded_host`**, RFC 7239 **`Forwarded`** (`host` / `proto`), then **`X-Forwarded-Host`** / **`X-Forwarded-Port`**, then **`Host`**. See `[CORS_OPERATIONS.md](./CORS_OPERATIONS.md)` (reverse proxies section) and `src/middleware/cors/forwarded.rs`.
2. **`Vary` (framework + app responsibility)**
  CORS sets **`Vary: Origin`**, or **`Vary: Origin, Access-Control-Request-Private-Network`** when **Private Network Access** is enabled. BRRTRouter does **not** merge other tokens (e.g. `Accept-Encoding`, `Authorization`); **apps** must append those to the final `Vary` value when responses vary on them — see `[CORS_OPERATIONS.md](./CORS_OPERATIONS.md#vary-merging)`.
3. ~~**Observability**~~ **(addressed)**
  `CorsMiddleware::with_metrics_sink` links to `MetricsMiddleware`; `/metrics` exports `brrtrouter_cors_origin_rejections_total`, `brrtrouter_cors_preflight_denials_total`, and `brrtrouter_cors_route_disabled_total` (per-route `x-cors: false`). Tracing `warn!`/`debug!` remains for detail.

### P2 — Spec and ecosystem coverage

1. ~~**Private Network Access (Chrome)**~~ **(opt-in)**
  Enable with **`allow_private_network_access`** on the builder or middleware. See `[CORS_OPERATIONS.md](./CORS_OPERATIONS.md)` and HTTP tests in `tests/cors_http_conformance_tests.rs`.
2. ~~**Redirects (3xx)**~~ **(documented)**
   CORS does not sanitize `**Location`** on redirects; browsers apply CORS rules separately. Covered in `[CORS_OPERATIONS.md](./CORS_OPERATIONS.md)`.
3. ~~**IDNA / Unicode hostnames**~~ **(documented + HTTP smoke)**
   Origin matching is **string-based**; no punycode↔Unicode normalization. **`http_cors_idna_origin_exact_bytes_reflected`** in `tests/cors_http_conformance_tests.rs` asserts punycode `Origin` reflection when allowlisted.

### P3 — Documentation and repo hygiene

1. ~~**Stale wip audit**~~ **Stubbed** — see `docs/wip/CORS_AUDIT.md`.
2. ~~**Operator guide**~~ `**[CORS_OPERATIONS.md](./CORS_OPERATIONS.md)`** — config, `x-cors`, middleware order, proxies, metrics; **Rustdoc** on `brrtrouter::middleware::cors` and related APIs documents behavior for `cargo doc`.

---

## 3. What “production ready” means here (checklist)

Use this as a release gate for framework + generated services:

- **Preflight semantics:** invalid method/header on a **real** preflight returns **403**; only non-preflight OPTIONS (no `ACRM`) falls through to the handler.
- **Origin sources (framework):** `build_with_routes` and `**merge_route_policies_with_global_origins`** apply global origins to Custom routes; deployments must still configure the builder (or YAML) so global origins are non-empty when using credentials.
- **Credentials:** no `*` origin with credentials; add **end-to-end / integration** tests for credentialed browser flows (cookies / `Authorization`) if your release gate requires proof beyond unit tests.
- **`AppService` security vs CORS:** OpenAPI security runs **before** the dispatcher (`[CORS_OPERATIONS.md](./CORS_OPERATIONS.md#middleware-order-and-options-preflight)`). If your API requires API keys or JWT on every method, **`OPTIONS` preflight must include those credentials** unless ingress exempts `OPTIONS` or you implement an exemption — add tests when this interaction is non-trivial.
- **Ingress / proxies:** document who sets **`Host`**, **`Forwarded`**, and **`X-Forwarded-*`**. Enable **`trust_forwarded_host`** only when the **edge is trusted** (overwrites or strips spoofed forwarded headers from clients). See `[CORS_OPERATIONS.md](./CORS_OPERATIONS.md#reverse-proxies-and-host-envoy--nginx)`.
- **Monitoring:** CORS denial counters on `/metrics` when the middleware is linked (see `[CORS_OPERATIONS.md](./CORS_OPERATIONS.md)`); structured logs via `tracing` for context.
- **Regression suite:** unit tests (`middleware_tests.rs`) + HTTP-level checks in `tests/cors_http_conformance_tests.rs` (forwarded host, RFC 7239 `Forwarded`, PNA, IDN smoke). Add **credentialed** and **`OPTIONS` + auth** integration tests per your policy (see §3 above).

---

## 4. Testing inventory (current)


| Area                                                 | Location                    | Notes                                                                                                 |
| ---------------------------------------------------- | --------------------------- | ----------------------------------------------------------------------------------------------------- |
| Preflight, OPTIONS edge cases, builder, route config | `tests/middleware_tests.rs` | Broad unit coverage; credentialed preflight (`test_cors_preflight_includes_credentials_when_enabled`); route-disabled metric (`test_cors_metrics_sink_route_disabled`) |
| HTTP CORS (forwarded host, PNA, IDN)                  | `tests/cors_http_conformance_tests.rs` | Raw TCP to `HttpServer`; API key on preflight when spec requires auth |
| Auth + CORS header presence                          | `tests/auth_cors_tests.rs`  | Smoke-level                                                                                           |
| Pet store integration                                | `examples/pet_store`        | Reference merge of YAML origins + OpenAPI                                                             |


**Coverage:** `route_config::merge_tests`, `test_build_with_routes_merges_global_origin_and_builds`; preflight denial in `middleware_tests` (`test_cors_preflight_denied_`*).

---

## 5. Recommended backlog (ordered)

1. ~~Fix preflight **None vs denied** branching; add tests.~~ **Done** (see `CorsPreflightOutcome`, `tests/middleware_tests.rs`).
2. ~~Unify **route + origin merge~~** **Done** (`merge_route_policies_with_global_origins`, `RouteCorsConfig::merge_global_origin_validation`, `build_with_routes`, pet_store).
3. ~~Add **metrics~~** **Done** (`brrtrouter_cors_`*, `with_metrics_sink`). Optional **forwarded Host** behavior remains a product/ingress decision — see `CORS_OPERATIONS.md`.
4. ~~**wip/CORS_AUDIT.md**~~ **Stubbed** with pointers to canonical docs.
5. ~~**Private Network Access**~~ **Done** (opt-in `allow_private_network_access`; see `tests/cors_http_conformance_tests.rs`).
6. ~~Optional: RFC 7239 **`Forwarded`**~~ **Done** (`src/middleware/cors/forwarded.rs`; preferred over `X-Forwarded-*` when both are present).

---

## 6. References (in-tree)

- Implementation: `src/middleware/cors/mod.rs`, `builder.rs`, `forwarded.rs`, `route_config.rs`, `error.rs`
- Example wiring: `examples/pet_store/src/main.rs`, `examples/pet_store/config/config.yaml`
- Middleware contract: `src/middleware/core.rs`