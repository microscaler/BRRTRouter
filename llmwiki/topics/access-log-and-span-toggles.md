# Access log and request span toggles

**Status:** verified (code + tests + bench, 2026-11-06)
**Code:** [`src/server/access_log.rs`](../../src/server/access_log.rs), `RequestLogger` and the `http_request` span in
[`src/server/service.rs`](../../src/server/service.rs)

## What and why

Every request leaves two records: the INFO access-log event `"Request completed"` (method, path, status, duration,
sizes, and the redacted request and response headers as JSON) and the `http_request` span. The access log is the
costlier part: both header passes (redaction plus JSON formatting) and the event itself run on every request. A
service with a high request rate can trade that record for throughput; the defaults keep it whole.

## Settings (read once per process)

| Variable | Values | Default |
|---|---|---|
| `BRRTR_ACCESS_LOG` | `full`, `sample:<rate>` (0 < rate ≤ 1), `off` | `full` |
| `BRRTR_ACCESS_LOG_SLOW_MS` | milliseconds | `100` |
| `BRRTR_REQUEST_SPAN` | `full`, `lean` | `full` |

- **`sample:<rate>`** decides when the request arrives: evenly spread requests at the rate (a process-wide count, no
  RNG), so `sample:0.1` is exactly one request in ten. The others never redact or format their headers.
- **Always logged**, whatever the mode: a 5xx, a 429 (the service's own back-pressure), and any request that took at
  least `BRRTR_ACCESS_LOG_SLOW_MS`. Failures and slow requests never disappear from the log. When such a request was
  not sampled its headers were never captured, so the event has `sampled = false` and no header fields. Other 4xx are
  not forced: scanners' 401s and 404s would swamp it.
- **`lean` span** drops the fields another field or the access log already carries: `method` and `path`
  (`http.request.method`, `http.route`), `header_count`, `stack_used_kb`. It keeps the OpenTelemetry HTTP fields,
  `tenant_id` (with a sampled access log the span is the only per-request record left, and traces are filtered by
  tenant first), `status`, `duration_ms` and the `brrtrouter.*` failure reasons.
- **Span sampling is not a router setting.** The router always creates the span; whether it is exported is the trace
  exporter's sampler's decision (the OpenTelemetry SDK's `Sampler`, configured where the exporter is installed).
- The DEBUG `"Request received"` event is unchanged: it formats the headers itself, and only when DEBUG is enabled for
  it.

## Cost (`benches/request_path.rs`, per request)

Measured 2026-11-06 with the bench's default subscriber (the `fmt` layer at INFO; no OpenTelemetry exporter), 20,000
requests per scenario, one connection. Allocations and bytes are exact; µs are wall time on a shared build host, so
read them as roughly ±10%.

| `BRRTR_ACCESS_LOG` / `BRRTR_REQUEST_SPAN` | `get_pet` allocs | bytes | µs | `bff_proxy_20k` allocs | µs |
|---|---:|---:|---:|---:|---:|
| `full` / `full` (default) | 254 | 24,712 | 38.2 | 2,530 | 236 |
| `sample:0.1` / `full` | 245 | 23,876 | 32.7 | 2,523 | 224 |
| `off` / `full` | 244 | 23,783 | 32.8 | 2,522 | 221 |
| `full` / `lean` | 213 | 19,454 | 33.2 | 2,500 | 222 |
| `off` / `lean` | 203 | 18,525 | 27.0 | 2,492 | 214 |

- The access log costs about 10 allocations, 0.9 KB and 5.5 µs per request; `sample:0.1` recovers nearly all of it.
- The span's dropped fields cost about 40 allocations and 5 KB per request under the `fmt` layer, which formats every
  span field into a string it keeps on the span. With a different subscriber stack the saving differs; it was not
  measured with an OTLP exporter.
- Together, `off` / `lean` is 20% fewer allocations and about 29% less server time on a simple route. On a proxied
  20 KB call the same absolute saving is a few percent: the body dominates.
- The other scenarios (`list_pets_query`, `get_post_2_params`, `add_pet_json_body`, `secure_jwt`) move by the same
  amounts.

## Tests

- `server::access_log` unit tests: parsing, the sampling cadence (exact counts, evenly spread, first request taken),
  which requests are forced, span modes.
- `tests/access_log_forced_tests.rs`: `off` with a 0 ms threshold logs every request with `sampled = false` and no
  headers; the `lean` span lacks the dropped fields and keeps `tenant_id` and the OpenTelemetry fields.
- `tests/access_log_sampled_tests.rs`: `sample:0.5` logs exactly two of four consecutive requests, with headers, and
  the others not at all; the default span has every field.

Each test binary sets its own variables before the first request, since the modes are read once per process.
