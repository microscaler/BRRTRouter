# `/metrics` reference

BRRTRouter serves Prometheus text on `GET /metrics`. Request and process metrics use the industry-standard names, so a
stock scrape works with no BRRTRouter-specific mapping. That covers Google Cloud Managed Service for Prometheus,
Cloud Monitoring's Application Monitoring golden signals, Grafana's HTTP and process dashboards, and common alert packs.

## Standard metrics

### HTTP server ([OpenTelemetry HTTP semantic conventions](https://opentelemetry.io/docs/specs/semconv/http/http-metrics/))

| Metric | Type | Labels |
|---|---|---|
| `http_server_request_duration_seconds` | histogram | `http_request_method`, `http_route`, `http_response_status_code`, `error_type` (5xx only) |
| `http_server_active_requests` | gauge | `http_request_method` |

- **What is counted:** every request, once, when it completes. That includes requests refused before routing (401, 403, 404, CORS) and the built-in endpoints (`/health`, `/ready`, `/metrics`, `/openapi.yaml`).
- **`http_route`:** the matched OpenAPI path template (`/orders/{id}`), never the raw path, so label cardinality is bounded by the spec. It is omitted when no route matched.
- **`http_request_method`:** one of the nine standard methods, otherwise `_OTHER`.
- **Buckets:** the semantic-convention advisory boundaries `0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1, 2.5, 5, 7.5, 10` seconds.

Golden signals:

```promql
# traffic (requests/s)
sum by (http_route) (rate(http_server_request_duration_seconds_count[5m]))
# server error ratio
sum(rate(http_server_request_duration_seconds_count{http_response_status_code=~"5.."}[5m]))
  / sum(rate(http_server_request_duration_seconds_count[5m]))
# p95 latency
histogram_quantile(0.95, sum by (le) (rate(http_server_request_duration_seconds_bucket[5m])))
```

### Process (Prometheus client-library process collector; Linux)

`process_cpu_seconds_total`, `process_resident_memory_bytes`, `process_virtual_memory_bytes`,
`process_start_time_seconds`, `process_open_fds`, `process_max_fds`.

## BRRTRouter-specific metrics

These have no standard equivalent; they are operational detail alongside the standard set.

- `brrtrouter_auth_failures_total`
- `brrtrouter_cors_origin_rejections_total`, `brrtrouter_cors_preflight_denials_total`, `brrtrouter_cors_route_disabled_total`
- `brrtrouter_rate_limit_sheds_total`, `brrtrouter_handler_deadline_timeouts_total`, `brrtrouter_compression_responses_total`
- `brrtrouter_connection_closes_total`, `brrtrouter_connection_errors_total`, `brrtrouter_connection_health_ratio`
- `brrtrouter_coroutine_stack_bytes`, `brrtrouter_coroutine_stack_used_bytes`
- `brrtrouter_worker_pool_{queue_depth,shed_total,dispatched_total,completed_total}{handler}`
- With the memory middleware:
  - `brrtrouter_memory_{heap,peak_rss,peak_heap,growth,baseline_rss}_bytes`
  - `brrtrouter_memory_measurements_total`
  - `brrtrouter_handler_invocations_total{handler}`

## Migrating from the previous names

| Removed | Use instead |
|---|---|
| `brrtrouter_requests_total{path,status}` | `http_server_request_duration_seconds_count{http_route,http_response_status_code}` |
| `brrtrouter_request_duration_seconds` (no labels) | `http_server_request_duration_seconds` (`sum by (le)` for the old shape) |
| `brrtrouter_active_requests` | `sum(http_server_active_requests)` |
| `brrtrouter_top_level_requests_total` | `sum(http_server_request_duration_seconds_count)` |
| `brrtrouter_request_latency_seconds` (average) | `rate(..._sum[5m]) / rate(..._count[5m])` |
| `brrtrouter_path_requests_total`, `brrtrouter_path_latency_seconds_{avg,min,max}`, `brrtrouter_metrics_path_overflow_total` | `http_server_request_duration_seconds` by `http_route` |
| `process_memory_rss_bytes` / `process_memory_vss_bytes` | `process_resident_memory_bytes` / `process_virtual_memory_bytes` |
| `process_memory_{heap,peak_rss,peak_heap,growth,baseline_rss}_bytes` | `brrtrouter_memory_*_bytes` |
| `handler_invocations_total`, `memory_measurements_total` | `brrtrouter_handler_invocations_total`, `brrtrouter_memory_measurements_total` |
