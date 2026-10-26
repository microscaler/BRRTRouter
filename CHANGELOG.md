# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Fixed (log redaction; response sampling per operation)

- **Hyphenated secret headers were logged in clear.** Redaction patterns are written with `_` (`api_key`), so
  `X-API-Key` (and `x.api.key`) never matched. Name matching now ignores ASCII case and treats `-`, `.` and space as
  `_`, with no allocation per header. Also redacted now: `private_key`, `session_id`/`sessionid` (e.g.
  `X-Session-Id`). `authorization`, `proxy-authorization`, `cookie`/`set-cookie` and any `*-token` header were and
  stay redacted.
- **`sample:<rate>` samples each operation.** One process-wide counter could check every response of one route and
  none of another under a steady interleaving (Codex review on #510); the count is now kept per handler.
- **Tooling: `metadata.extensions` in a BFF suite config** puts root `x-*` keys (e.g. `x-brrtrouter-stack-size`)
  into the generated BFF spec, right after `openapi`. Root extensions had no source, so they were hand-added to
  generated copies and lost on the next generation.

### Added (spec lint: OpenAPI 3.0 keywords in 3.1 documents) — ADR 006 §4

- `brrtrouter-gen lint` reports, as errors, `nullable: true` in an OpenAPI 3.1 document (3.1 schemas are JSON
  Schema 2020-12, which ignores it, so a `null` fails response validation: PriceWhisperer `ab1d7059`) and boolean
  `exclusiveMinimum`/`exclusiveMaximum`. Read from the document as written (the typed model drops `nullable`);
  examples, enums, defaults and properties named `nullable` are not flagged. Locations are JSON pointers.
  First run over the consumers (26 Oct 2026): PriceWhisperer trader `market` (3), `orders` (1, request) and the
  trader BFF spec (4), and every Sesame-IDAM service spec, still declare nulls this way.
- Tooling: the BFF generator's fallback `Error` schema declared `details` with `nullable: true`; it is now
  `type: [object, 'null']`, so a regenerated 3.1 BFF spec passes this lint.

### Changed (response validation modes; pass-through is opt-in) — ADR 006

- **`BRRTR_RESPONSE_VALIDATION=full|sample:<rate>|off`**, default `full` (unchanged behaviour: a mismatch is a 500).
  `sample:<rate>` checks every `1/rate`-th response and logs + counts a mismatch without failing the request; `off`
  checks nothing. New counters on `/metrics`: `brrtrouter_response_validation_total{handler,result="ok"|"mismatch"}`
  and `brrtrouter_response_validation_skipped_total{handler}`. `x-brrtrouter-validate-response: true` forces `full`
  for an operation.
- **Proxied JSON pass-through is now opt-in** (`BRRTR_PROXY_PASSTHROUGH=on`, default off). The previous entry made it
  the default and skipped validation of proxied bodies; with the defaults, proxied responses are parsed, validated and
  re-serialised as before. With it on, bodies are validated per the mode (parsed for the check only).
- `docs/ADRS/006-validation-modes-proxy-passthrough-trusted-hop.md`: trust model, the modes' guarantees, how drift is
  caught per build; streaming, raw request bytes and trusted-hop validation are later phases.

### Changed (BFF proxy: JSON passes through)

- **A proxied 2xx JSON body is passed through as the downstream wrote it.** `proxy_untyped` parsed every
  downstream body into a `serde_json::Value`, the BFF validated it against the response schema and serialised it
  again: on `benches/request_path.rs` a 200 KB body cost 22,285 allocations and 1.83 ms per request. It is now
  written as received (252 allocations, 0.19 ms). The downstream validated it against the same schema. An
  operation that wants the BFF to check it sets `x-brrtrouter-validate-response: true` (the text is then parsed
  for the check and still written unchanged). Non-2xx bodies, non-JSON content types and invalid UTF-8 keep the
  parsed path. The proxied body buffer is sized from `Content-Length`.
- `RouteMeta` has a new field, `validate_proxied_response` (struct literals need it).

### Changed (idle CPU)

- **Idle `may` workers poll every 50 ms instead of 10 ms.** Each of a service's 32+ workers woke from `epoll_wait`
  every 10 ms to look for work, i.e. 3,200+ wake-ups a second in an idle pod. In pp that came to about 75 mcores per pod
  doing nothing. `BRRTR_MAY_POLL_TIMEOUT_MS` (default 50) sets it. `RuntimeConfig::from_env` applies it (a service that
  set `may::config().set_timeout_ns` itself keeps its value unless `BRRTR_MAY_POLL_TIMEOUT_MS` is set explicitly). Idle CPU on ms02, 50 workers: 19 → 5 mcores.
  Latency under load is unchanged (`benches/idle_cpu.rs`). I/O timeouts are not affected.
- `RuntimeConfig` has a new field, `may_poll_timeout_ms`.

### Changed (BFF proxy)

- **Downstream addresses are resolved once, not per proxied request.** `proxy_untyped`/`proxy_sse` called
  `getaddrinfo` (blocking the coroutine's worker thread) for every request, and `<svc>.<ns>.svc.cluster.local` sits
  under the pod resolver's `ndots:5`, so each lookup first walked the search domains. Addresses are now cached per
  host for `BRRTR_PROXY_DNS_TTL_SECS` (default 30; `0` resolves every request) and Service FQDNs are looked up with a
  trailing dot. A connect failure on a cached address re-resolves and retries once.

### Changed (breaking: reply channel type)

- **`HandlerRequest::reply_tx` is `may::sync::mpsc::Sender<Box<HandlerResponse>>`.** The dispatcher opens a
  reply channel per request and `may`'s queue allocates a 64-slot block up front: with bare `HandlerResponse`
  slots that was 46 KB per request (~40 % of all bytes the request path allocated), boxed it is 1 KB. Send with
  `req.reply(response)` (new) or `reply_tx.send(Box::new(response))`; name the channel
  `mpsc::channel::<Box<HandlerResponse>>()`. `reply_tx: may::sync::mpsc::channel().0` literals compile unchanged.

### Changed (performance: fewer copies on the request path)

Measured with the new `benches/request_path.rs` (allocations, bytes and µs per request through the whole HTTP
path); findings, numbers and what is still open: `docs/perf/copy-and-borrow-audit.md`.

- **JWKS bearer validation**: the claims cache is keyed by the token (a hit parses no header, formats no key and
  copies no claims: they are held as `Arc<Value>`); the key-rotation check no longer clones the key; the DEBUG
  span's token-version lookup runs only when that span is enabled; the "allowed" audit line is admitted (INFO
  enabled, rate limit) before its fields are extracted. Same checks, same log output.
- **Schema validators** are found per route by address (`ValidatorCache::get_or_compile_for_route`) instead of
  serializing and hashing the schema on every request and response.
- **Dispatcher** moves the request to the handler instead of deep-copying it twice; middleware `after` hooks
  receive `HandlerRequest::without_payload()` (method, path, handler, params, headers, cookies — **no body or
  JWT claims**). No built-in or known consumer middleware reads those in `after`; read them in `before`.
- **Typed handlers** move the JWT claims to `TypedHandlerRequest::jwt_claims` before converting the request, so a
  hand-written `TryFrom<HandlerRequest>` no longer sees `jwt_claims` (the generated one never read it). The worker
  pool path and `TypedHandlerFor::from_handler` convert by value instead of `try_from(req.clone())`.
- **Generated handlers** build each parameter's schema once (`static LazyLock<Value>`), not per request.
  Regenerate services to pick it up.
- `AppService::call` moves headers, cookies, query parameters, the route match and the response headers instead of
  cloning them; DEBUG-only values are computed only when DEBUG is on; access-log header JSON is written in one
  buffer (same output). New `Dispatcher::dispatch_with_id` takes a `RequestId` (no format-then-parse per request).
- `MemoryMiddleware::before` no longer takes a discarded `/proc` sample.

### Added

- **BFF client metrics.** Every call the BFF proxy makes to a downstream service (`proxy_untyped`, `proxy_sse`) is
  recorded as `http_client_request_duration_seconds{server_address,http_request_method,http_response_status_code,error_type}`
  (OpenTelemetry HTTP client semantic convention, the server histogram's buckets) and rendered on `/metrics`.
  `server_address` is the downstream Service; a call the downstream never answered has no status code and an
  `error_type` naming why (`connect`, `timeout`, `dns`, ...). Joins a slow BFF route to the service behind it.

### Fixed (memory; behaviour change for `BRRTR_STACK_SIZE`)

- **`BRRTR_STACK_SIZE` is bytes**, as documented (`262144`, `0x40000`, `256KiB`, `2MiB`). It used to reach
  `may::config().set_stack_size` unconverted, and `may` takes machine words, so `0x40000` ran 2 MiB stacks.
  `RuntimeConfig::stack_size` still holds the value `may` takes (now converted to words); `stack_size_bytes()` gives
  bytes. The unset default is unchanged (256 KiB). Deployments that set the variable should set the byte value they
  were running: `0x40000` → `2MiB`, `0x200000` → `16MiB`.
- **Coroutine pool capacity 100 by default, without each service setting it.** `may` allocates its whole pool
  (1,000 coroutines by default) when its scheduler starts. Services with their own `main` never called
  `set_pool_capacity`, so they kept 1,000 pooled stacks (1,042 stack mappings per Sesame pod). BRRTRouter now applies
  `BRRTR_COROUTINE_POOL_CAPACITY` (default 100) before its first coroutine (`RuntimeConfig::from_env`,
  `Dispatcher::new`, the handler spawners, `HttpServer::start`) unless the service chose a capacity itself.
  `RuntimeConfig::apply()` sets stack size, workers and pool capacity in one call.
- `brrtrouter_coroutine_stack_bytes` and the request span's stack size report bytes (they reported `may` words).
- microscaler-observability 2b7e35c (previous release): the Cloud Profiler agent clears the symbol cache after each
  profile and pprof's sample table exists only while a CPU profile runs.

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
