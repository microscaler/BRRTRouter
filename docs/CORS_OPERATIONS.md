# CORS operations guide

Canonical architecture and gap analysis: [`CORS_IMPLEMENTATION_AUDIT.md`](./CORS_IMPLEMENTATION_AUDIT.md).

## Configuration

- **Global policy:** `CorsMiddlewareBuilder` (origins, methods, headers, credentials, `maxAge`, exposed headers).
- **Per-route overrides:** OpenAPI `x-cors` — `inherit`, `false` (disable CORS for that operation), or an object (`allowedMethods`, `allowedHeaders`, `allowCredentials`, `exposeHeaders`, `maxAge`). **Origins are not** in OpenAPI; set them in deployment config (e.g. `config.yaml`) and merge via `merge_route_policies_with_global_origins` or `CorsMiddlewareBuilder::build_with_routes`.
- **Reference app:** `examples/pet_store` (from `templates/main.rs.txt`) registers **`MetricsMiddleware`**, **`AppService::set_metrics_middleware`**, and chains **`CorsMiddleware::with_metrics_sink(metrics.clone())`** so **`brrtrouter_cors_*`** counters populate on `/metrics`.

## Middleware order and OPTIONS preflight

Register middleware in a defined order on the `Dispatcher`. Typical concerns:

1. **Tracing / logging** (optional, first to see raw requests).
2. **CORS** — must run **before** the handler for `OPTIONS` preflight short-circuits and for `before()` origin checks.
3. **Auth (OpenAPI security)** — in **`AppService`**, security is validated **before** the dispatcher runs (middleware inside the dispatcher runs only after routing and auth succeed). So **`OPTIONS` preflight requests must satisfy the same OpenAPI security** (e.g. `X-API-Key`, `Authorization`) as `GET`/`POST`, unless your **ingress** exempts `OPTIONS` from auth or you add an **application-level** exemption. Example HTTP test with API key on preflight: `tests/cors_http_conformance_tests.rs`.

Document the order your generated service uses. **In-repo HTTP integration tests:** global `ApiKeyHeader` — `tests/cors_http_conformance_tests.rs` (`http_cors_preflight_returns_401_without_api_key`, `http_cors_preflight_with_api_key_returns_200_and_acao`); global **Bearer** / **cookie** API key — `tests/cors_http_security_schemes_tests.rs`. **Browser E2E** (below) is optional and only needed when your gate requires real browser networking behavior.

## Reverse proxies and `Host` (Envoy / nginx)

Same-origin detection compares the request **`Origin`** to the **effective server authority** (see below). By default that authority is the **`Host`** header. If you enable **trusted forwarded host**, the authority comes from proxy metadata first, then falls back to **`Host`**:

- **`CorsMiddlewareBuilder::trust_forwarded_host(true)`** or **`CorsMiddleware::with_trust_forwarded_host(true)`** — same-origin checks use trusted proxy metadata in this order:
  1. **RFC 7239 `Forwarded`** — all `Forwarded` header lines are merged; the first `host` and first `proto` parameters (across comma-separated `forwarded-element` segments) build the authority. If `host` has no port, **`proto=http` / `proto=https`** supplies default ports **80** / **443** (see `src/middleware/cors/forwarded.rs`).
  2. **`X-Forwarded-Host`** — first comma-separated token; if there is no port on the host, **`X-Forwarded-Port`** is appended when present and valid.
  3. Else **`Host`**.

**Ingress / trust:** document how your edge sets **`Host`**, **`Forwarded`**, and **`X-Forwarded-*`**. Enable **`trust_forwarded_host`** only on a **trusted path**—typically TLS termination at Envoy/nginx, forwarded headers derived from the connection (not raw client input), and policy that **blocks or overwrites** spoofed `host=` / `X-Forwarded-Host` from untrusted clients.

**Runbook snippet (what to write down):** (1) Which component terminates TLS and injects `Forwarded` / `X-Forwarded-*`. (2) Whether clients can reach the app directly (if yes, `trust_forwarded_host` is usually wrong). (3) Which header names your ingress uses (`Forwarded` vs legacy `X-Forwarded-*`). (4) Who owns updating this when the edge changes.

**nginx (illustrative)** — set upstream `Host` and forward client proto; your app then sees consistent `Host` / `X-Forwarded-*` (enable `trust_forwarded_host` only behind this hop):

```nginx
location /api/ {
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_pass http://brrtrouter_upstream;
}
```

**Envoy** — use `xff_num_trusted_hops` / `use_remote_address` and `RouteConfiguration` `request_headers_to_add` for `Forwarded` or `X-Forwarded-*` per your platform docs; document the effective hop count in your runbook.

## `Vary`

Successful CORS responses set a **`Vary`** header for cache correctness. The framework sets:

- **`Vary: Origin`** when Private Network Access (PNA) is **off**.
- **`Vary: Origin, Access-Control-Request-Private-Network`** when PNA is **on** (`allow_private_network_access`).

### Vary merging

BRRTRouter **replaces** the `Vary` header for CORS (it does not parse or append to an existing comma-separated list from upstream). If your handler or compression middleware also needs **`Accept-Encoding`**, **`Accept-Language`**, **`Authorization`**, or other tokens, build the final value with **`brrtrouter::middleware::merge_vary_field_value`**, which merges a comma-separated existing `Vary` with extra field-name tokens (dedupes ASCII case-insensitively; if any token is `*`, the result is `*` per RFC 7231).

```rust
use brrtrouter::middleware::merge_vary_field_value;

let final_vary = merge_vary_field_value(
    resp.get_header("vary"),
    &["Accept-Encoding", "Accept-Language"],
);
// Set `final_vary` on the outgoing response (exact API depends on your handler).
```

If your gateway normalizes `Vary`, you can call the same helper there instead of in Rust.

## Prometheus metrics

When `CorsMiddleware::with_metrics_sink(Arc<MetricsMiddleware>)` uses the **same** `Arc` as the dispatcher’s metrics middleware, `/metrics` includes:

| Metric | Meaning |
|--------|--------|
| `brrtrouter_cors_origin_rejections_total` | 403 — `Origin` not allowed (before handler). |
| `brrtrouter_cors_preflight_denials_total` | 403 — preflight: method/header not allowed after origin validated. |
| `brrtrouter_cors_route_disabled_total` | Per-route CORS off (`x-cors: false`): request handled without CORS headers (not an error). |

If no sink is linked, CORS behavior is unchanged; counters stay at zero.

## Production deployment

Use this checklist when wiring a generated or hand-rolled service for production:

1. **Ingress / forwarded headers** — Document who sets **`Host`**, **`Forwarded`**, and **`X-Forwarded-*`** at your edge (Envoy, nginx, cloud LB). Enable **`trust_forwarded_host`** on `CorsMiddleware` **only** on a **trusted path** (TLS at the edge, forwarded metadata from the proxy, not unvalidated client input). See [Reverse proxies and `Host`](#reverse-proxies-and-host-envoy--nginx) and the runbook snippet there.
2. **Metrics** — Call **`CorsMiddleware::with_metrics_sink`** with the **same** `Arc<MetricsMiddleware>` instance registered on the `Dispatcher` so `/metrics` exposes `brrtrouter_cors_*` counters. **`AppService::set_metrics_middleware` alone does not attach CORS to those counters** — the CORS middleware must use `with_metrics_sink`. Without linking, behavior is correct but CORS counters stay at zero. The **`examples/pet_store`** template wires both.
3. **Tracing** — Run a **`tracing`** subscriber (JSON or pretty) in the process so CORS-related `warn!` / `debug!` lines are available in your log pipeline.
4. **Optional browser verification** — In-repo tests use **raw TCP HTTP**. For a **real browser** check (cookie jar, `credentials: 'include'`, redirect chains), either:
   - **Automated:** Playwright / WebDriver / Cypress against a staging URL; assert `OPTIONS` and follow-up requests show expected `Access-Control-*` and auth headers.
   - **Manual:** Open DevTools → Network → trigger the flow from your SPA; confirm preflight returns 2xx with correct CORS headers, then the actual request.
   - **Console snippet (same-origin API on another port still cross-origin):**

```javascript
fetch("https://api.example.com/echo", {
  credentials: "include",
  headers: { "X-Custom": "1" },
}).then((r) => console.log(r.status));
```

5. **OAuth / third-party IdP** — If browsers complete redirects or PKCE outside your API, validate flows in staging; the framework tests do not replace IdP integration testing.

## Credentials and cookies

Do not combine **wildcard** `Access-Control-Allow-Origin: *` with **credentials**; the builder and `RouteCorsConfig::with_origins` enforce this at startup.

Preflight responses include `Access-Control-Allow-Credentials: true` when credentials are enabled (`test_cors_preflight_includes_credentials_when_enabled` in `tests/middleware_tests.rs`).

## Redirects (`3xx`)

BRRTRouter does **not** rewrite the `Location` header on redirect responses. For CORS **simple** requests, browsers follow redirects and apply CORS to the **final** response URL. For requests that use the CORS-preflight path, behavior depends on redirect status and same-origin rules (see [Fetch](https://fetch.spec.whatwg.org/#cors-protocol-and-http-caches)).

**Operational guidance:** if your API returns redirects to another origin, validate that clients and browser CORS rules behave as expected; do not assume the framework rewrites redirect targets.

## Internationalized domain names (IDNA / Unicode hosts)

`Origin` validation uses **exact string comparison** on the configured allowlist vs the `Origin` header (after policy selection). BRRTRouter does **not** normalize Unicode hostnames to punycode (or the reverse) for matching.

**Recommendation:** list allowed origins in the **same serialization** browsers send in `Origin` for your site (often punycode in the host part, e.g. `https://xn--mnchen-3ya.de`). If an origin is rejected unexpectedly, compare raw header values against config. Optional future work: canonical origin comparison via a shared normalizer.

## Private Network Access (Chrome / [WICG](https://wicg.github.io/private-network-access/))

When **`CorsMiddlewareBuilder::allow_private_network_access(true)`** (or **`CorsMiddleware::with_allow_private_network_access(true)`**):

- **Preflight:** if the browser sends **`Access-Control-Request-Private-Network`** (value `true` or empty), a successful preflight includes **`Access-Control-Allow-Private-Network: true`** when the origin is allowed.
- **Non-preflight cross-origin responses:** **`Access-Control-Allow-Private-Network: true`** is added on successful CORS responses when the feature is enabled (so the actual response satisfies browsers that require it after preflight).

Use this when a **public** site must call an API served on a **less-public** address space (e.g. private RFC1918). HTTP-level checks: `tests/cors_http_conformance_tests.rs`.

## API documentation (`cargo doc`)

Run `cargo doc -p brrtrouter --no-deps` (or `--open`) for rustdoc on `brrtrouter::middleware`, `CorsMiddleware`, `CorsMiddlewareBuilder`, `merge_route_policies_with_global_origins`, and `MetricsMiddleware` CORS counters.

## Related

- Implementation: `src/middleware/cors/` (`mod.rs`, `builder.rs`, `forwarded.rs`, `route_config.rs`)
- `MetricsMiddleware`: `src/middleware/metrics.rs`
