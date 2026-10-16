# HTTP server and process metrics (`/metrics`)

**Status:** verified (code + tests, 2026-10-13)

**Source docs:** [`docs/METRICS.md`](../../docs/METRICS.md) (names, labels, migration table)

## What and why

`/metrics` exposes request and process metrics under the industry-standard names, so a stock Prometheus scrape works
unmapped. That covers Google Cloud Managed Service for Prometheus, Cloud Monitoring's Application Monitoring golden
signals, Grafana HTTP and process dashboards, and alert packs.

- `http_server_request_duration_seconds{http_request_method,http_route,http_response_status_code,error_type}`:
  the OpenTelemetry `http.server.request.duration` histogram with the semantic-convention buckets. Traffic, error ratio
  and latency percentiles all derive from it; aggregate with `sum by (le)` before `histogram_quantile`.
- `http_server_active_requests{http_request_method}`.
- `process_*`: the Prometheus client-library process collector names (Linux `/proc/self`).
- `brrtrouter_*`: BRRTRouter-only operational detail (auth failures, CORS, worker pools, stacks, memory views).

## Recording model

- **One record per request**, written when the request logger (`RequestLogger` in `AppService::call`) is dropped.
  It is not written by `MetricsMiddleware`, which only sees dispatched requests. So the histogram includes requests
  refused before routing (401/403/404, CORS), synthesized preflights (under the pre-flighted route), and the built-in
  endpoints (`/health`, `/ready`, `/metrics`, `/openapi.yaml`, `/docs`).
- **Requests that fail to parse** (400 bad method or target, 413, 414, multipart errors) are recorded on the parse-error
  branch with `http_request_method="_OTHER"` and no route. They never reach `http_server_active_requests`: parsing is
  synchronous, so there is no window to count.
- **`http_route`** is the matched OpenAPI template, never the raw path, so cardinality is bounded by the spec.
  `MAX_SERIES` (4096) folds anything beyond into `http_route="_OTHER"` as a guard.
- **Hot path:** a read lock, then two `HashMap` lookups ((method, status), then the borrowed route `&str`). A
  `String` is allocated only the first time a series is seen.

## Code anchors

- `src/server/http_metrics.rs`: `observe`, `ActiveRequest`, `render_http`, `render_process`, `DURATION_BUCKETS`
- `src/server/service.rs`: `RequestLogger` (`_active`, `route`, the `Drop` impl calling `observe`), the parse-error
  branch, built-in route assignment, CORS preflight route
- `src/server/service.rs`: `metrics_endpoint` (standard names first, then `brrtrouter_*`)
- `src/middleware/memory.rs`: `export_metrics` (`brrtrouter_memory_*`)
- Tests: `server::http_metrics::tests`, `tests/metrics_endpoint_tests.rs`, `tests/cors_http_conformance_tests.rs`

## Gaps / drift

- `MetricsMiddleware` still keeps its per-path and status counters internally (used by unit tests), but they are no
  longer exported. Removing them is a follow-up.
- `process_*` metrics are Linux-only. Other platforms export none.

> **Open:** export through OTLP as well (the request span already carries `http.route`), or stay with the Prometheus
> scrape only?

## Why credentials are refused (2026-10-16)

`brrtrouter_auth_failures_total` counts every refused request. `brrtrouter_auth_failures_by_reason_total{reason}`
says why, one fixed token per refusal:

| reason | from |
|--------|------|
| `token_expired`, `key_not_found`, `invalid_signature`, `invalid_issuer`, `invalid_audience`, `missing_token`, `invalid_token_format`, `missing_kid`, `unsupported_algorithm`, `invalid_token_type`, `missing_required_claim`, `token_revoked`, `stale_token_version`, `token_status_unavailable`, `invalid_token_status_claims` | the JWKS bearer provider (`ValidationError::metric_reason`) |
| `insufficient_scope`, `scheme_undefined`, `provider_missing`, `invalid_credentials` | the router (`invalid_credentials` when the provider gave no reason) |

Mechanism (`src/security/telemetry.rs`): a provider calls `set_refusal_reason` (coroutine-local slot); the router
`take_refusal_reason`s once per refused request, counts it, and logs it in its `Authentication failed` line. The
router's second, scope-probing `validate` call overwrites the slot, so a refusal is never counted twice. Any provider
may record its own reason the same way.

JWKS freshness at a consumer: `brrtrouter_jwks_refresh_total{outcome="ok|failed"}`, `brrtrouter_jwks_keys`,
`brrtrouter_jwks_last_success_timestamp_seconds` (rendered only once a JWKS provider has refreshed). A consumer whose
last success is older than its cache TTL by a wide margin cannot validate tokens signed with a newly rotated key.

Tests: `security::telemetry::tests`.
