# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed (breaking: metric names)

- **`/metrics` uses the industry-standard names** (OpenTelemetry HTTP semantic conventions, Prometheus process collector).
  - Request metrics are now `http_server_request_duration_seconds{http_request_method,http_route,http_response_status_code,error_type}`, with the semantic-convention buckets, and `http_server_active_requests`. Every request is recorded, including those refused before routing.
  - Process metrics are now `process_cpu_seconds_total`, `process_resident_memory_bytes`, `process_virtual_memory_bytes`, `process_start_time_seconds`, `process_open_fds` and `process_max_fds`.
  - Removed: `brrtrouter_requests_total`, `brrtrouter_request_duration_seconds`, `brrtrouter_active_requests`, `brrtrouter_top_level_requests_total`, `brrtrouter_request_latency_seconds`, `brrtrouter_path_*` and `brrtrouter_metrics_path_overflow_total`.
  - Renamed: BRRTRouter's own memory views moved from `process_memory_*` to `brrtrouter_memory_*`.
  - The bundled Grafana dashboards and CI are updated.
  - Mapping: `docs/METRICS.md`.

### Added
- **Typed handlers — REST status without panicking:** `HandlerResponseOutput` and `HttpJson<T>` in `brrtrouter::typed`. Plain `Serialize` return types still produce HTTP 200; use `HttpJson::new(status, body)` (or `not_found`, `ok`) for other status codes. Response mapping is unified across `spawn_typed*`, `register_typed_with_pool`. Tests: `typed::core::tests`, `tests/typed_tests.rs` (`test_spawn_typed_http_json_status_without_panic`).
- Docs: `docs/MIGRATION_TYPED_HANDLER_HTTP_STATUS.md` — consumer migration from `panic!` to `HttpJson` on typed handlers.
- Observability: Introduced typed `RequestId` (ULID-backed) with end-to-end correlation.
  - Server now accepts inbound `X-Request-ID` (validated) or generates a ULID.
  - Server always echoes `X-Request-ID` on responses.
  - `HandlerRequest.request_id` migrated to a typed `RequestId` newtype.
  - Dispatcher gained `dispatch_with_request_id(...)` to pass correlation IDs explicitly.
  - `RequestLogger` now captures `request_id` at start; completion logs include it reliably.
- Tests updated to construct `RequestId` without parsing dummy strings.
- Docs: Updated `docs/LogAnalysis.md` and `docs/wip/LOGGING_PRD.md` for ULID `request_id` and header propagation; clarified that `request_id` must not be used as a Prometheus label.
- **Performance**: Lock-free metrics middleware using DashMap for high-throughput scenarios.
  - Replaced `RwLock<HashMap>` with `DashMap` for path and status metrics to eliminate contention at 5k+ RPS.
  - Added `MetricsMiddleware::pre_register_paths()` for startup path registration.
  - Added comprehensive concurrency tests and metrics contention benchmarks.
  - All per-path metrics now use atomic operations with minimal locking.

### Changed
- **Typed `Handler` trait:** `type Response` is now bounded by `HandlerResponseOutput` instead of `Serialize`. Any type that implements `Serialize` still qualifies via a blanket impl (existing handlers unchanged).
- Metrics test aligned to labeled series format for `brrtrouter_requests_total`.
- **Breaking**: `path_metrics` and `status_metrics` now use `DashMap` instead of `RwLock<HashMap>`.
  Internal API remains backward compatible for external consumers.

### Notes
- This change is backward compatible for log consumers: the `request_id` field remains, now populated with a ULID string.
- Prometheus dashboards are unaffected (no new labels introduced).
- Metrics collection now scales linearly with concurrent requests without lock contention.
