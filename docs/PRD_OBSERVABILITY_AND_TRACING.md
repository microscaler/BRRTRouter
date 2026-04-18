# PRD: BRRTRouter Observability & Tracing (O‑series)

**Document version:** 0.3 (DRAFT — for review)
**Date:** 2026-04-18
**Status:** Draft. Stepping-stone PRD that unblocks continued work on [`docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](./PRD_HOT_PATH_V2_STABILITY_AND_PERF.md). No phase of this PRD has landed yet. v0.2 folded in the Lifeguard composition contract. v0.3 replaces the stdout→Promtail→Loki log path with OTLP-native logs direct to the Collector, adds Phase O.12 (Pyroscope continuous profiling), and confirms the "stdout is startup-only" architectural directive.
**Owner:** BRRTRouter core
**Target branch:** `pre_BFF_work` (phases land as small PRs, same cadence as the hot-path PRD)
**Primary driver:** Jaeger currently receives **zero** BRRTRouter traces. The cluster observability stack (Jaeger, OTEL Collector, Prometheus, Grafana, Loki, Promtail, Pyroscope) is wired correctly, but the **application side never sends anything to OTLP** — `src/otel.rs::init_logging_with_config` only installs a `tracing_subscriber::fmt` layer to stdout, silently ignores its `_otlp_endpoint` argument, and the module's own doc-comment acknowledges *"OTLP export will be added in a future phase"* (`src/otel.rs:10‑11`, `src/otel.rs:338`). Without server-side spans and per-route metrics, PRD_HOT_PATH_V2 cannot differentiate router-vs-dispatcher-vs-handler regressions; Hauliage dev-env stability investigations cannot follow a request across services; and the in-cluster Goose bench we want to run will produce numbers we can't explain.

## 1. Motivation

The immediate trigger is the 2026-04-18 attempt to stand up an in-cluster Goose bench against pet_store. The plan was: deploy pet_store with OTEL wired, hit it from a Goose Job inside `brrtrouter-dev`, read truthful latency / throughput / failure signals from Jaeger + Prometheus + Loki rather than from Goose's ASCII-table regex parser on a bare-metal loopback.

Three layers of that plan are blocked:

1. **Jaeger is empty.** The cluster is fine; the app never emits. We cannot validate a single span-to-span trace, nor debug a single slow-dispatch event, until BRRTRouter emits OTLP.
2. **Per-route SLO queries are impossible.** `brrtrouter_request_duration_seconds_bucket` has no `path` label, and the dashboards' "Response Latency p50/p95/p99" panels aggregate globally. A per-route regression would vanish in the global aggregate. The previous Phase R.1 "thermal drift" confusion would have been caught instantly with per-route histograms.
3. **Log → trace navigation doesn't work.** Even if we wire OTLP tomorrow, `tracing-subscriber`'s JSON output doesn't carry `trace_id` / `span_id`, and Promtail doesn't parse the JSON to extract them. A user clicking "view logs for this span" in Grafana gets no matches.

This PRD is the fix. It is deliberately ordered so that **Phase O.1 alone resolves the Jaeger-is-empty complaint**, the rest layer coverage and correlation on top.

## 2. Goals

1. **G1 — OTLP egress works.** BRRTRouter emits OTLP spans to `OTEL_EXPORTER_OTLP_ENDPOINT`, visible in Jaeger within 30 s of a request being served.
2. **G2 — Trace context flows through.** Incoming `traceparent` / `tracestate` is honoured; handler-emitted outbound calls (where applicable) carry it forward; the response echoes `traceparent` when configured.
3. **G3 — Span tree is meaningful.** Every significant request phase (accept / parse / router.match / dispatcher.dispatch / handler.execute / schema.validate / response.encode / write) is a span with stable names and fields.
4. **G4 — Logs carry trace identity.** Every log record emitted while a span is active includes `trace_id` and `span_id`; Promtail extracts them as Loki labels; Grafana log → trace jump works both directions.
5. **G5 — Per-route metrics.** `http_server_duration_seconds_bucket{method,route,status}` histogram with stable bucket boundaries. Auth failures, schema failures, CORS rejections, worker-pool drops: all exposed as labeled counters.
6. **G6 — Graceful flush.** SIGTERM / drop of `ShutdownGuard` flushes the batch span processor, the log appender, and in-flight metric exports. Rolling restarts lose nothing.
7. **G7 — Dashboards reflect reality.** The coverage matrix (§4.3) has **zero NO** rows. Misleading panels ("Memory Usage" showing coroutine-stack bytes) are fixed or renamed. Currently unmounted ConfigMaps (memory + performance dashboards) are actually loaded.
8. **G8 — Opt-in and safe.** All changes default to off-when-unconfigured. A BRRTRouter service with no OTEL env vars behaves identically to today — same logs on stdout, same metrics on `/metrics`, zero OTLP network traffic, zero span allocation overhead.

## 3. Non-goals

- **N1** — Custom sampling policies beyond the standard OTEL-SDK samplers (parent-based, trace-id-ratio, always-on, always-off). Tail sampling is a future Collector-side concern.
- **N2** — ~~Log-aggregation rewrite. Loki / Promtail stay as-is; we add a JSON pipeline stage and tune retention, nothing deeper.~~ **RETIRED in v0.3.** Promtail's role is substantially narrowed (runtime logs go OTLP-native, Promtail only tails startup stdout) — that *is* a log-aggregation rewrite and is now IN scope for Phase O.8.
- **N3** — APM-vendor-specific integrations (Datadog, Honeycomb, New Relic). OTLP is the contract; vendor adapters are the Collector's job.
- **N4** — Custom tracing-UI. We use Jaeger today; Tempo is a candidate for G7 but swapping is out of scope.
- **N5** — Hauliage-side handler instrumentation. That belongs in a Hauliage PRD once BRRTRouter exposes the right span + metric shapes.
- **N6** — Performance regressions already captured in [`PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](./PRD_HOT_PATH_V2_STABILITY_AND_PERF.md). This PRD instruments those paths but does not re-litigate their design.
- **N7** — Kubernetes operator / auto-injection of OTEL SDK. The dependency sits inside BRRTRouter's own crate graph.
- **N8 — Installing an OTEL Metrics SDK (`MeterProvider`) in BRRTRouter.** Per §4.9, Lifeguard already owns `global::set_meter_provider` via `OnceCell`. BRRTRouter MUST NOT contest that global. All BRRTRouter metrics stay in the hand-rolled Prometheus-text `/metrics` endpoint, concatenated with Lifeguard's `prometheus_scrape_text()` (Phase O.6). OTLP metrics export, if ever needed downstream, is the OTEL Collector's job via its `prometheus` receiver → `otlp` exporter — out of scope here.
- **N9 — Refactoring Lifeguard's observability setup.** This PRD only modifies BRRTRouter. Any Lifeguard-side change (e.g. accepting an externally-provided `MeterProvider`) is a Lifeguard PRD. The constraint here is "don't break Lifeguard when it's embedded" (§Phase O.0).

## 4. Current state — audit findings (2026-04-18)

### 4.1 Cluster observability stack is correct

All critical wiring for receiving and storing telemetry is in place. Confirmed against `k8s/observability/*.yaml` and `k8s/app/base/*.yaml`:

- **Jaeger** (`jaegertracing/all-in-one:1.52`) — badger storage, `COLLECTOR_OTLP_ENABLED=true`, OTLP gRPC `:4317`, OTLP HTTP `:4318`, UI `:16686`. Service DNS `jaeger` in `brrtrouter-dev` resolves.
- **OTEL Collector** (`otel/opentelemetry-collector-contrib:0.93.0`) — OTLP receivers on `:4317` (gRPC) + `:4318` (HTTP), traces pipeline `[otlp] → [memory_limiter, batch] → [otlp/jaeger, logging]`. Jaeger export goes to `jaeger:4317`, `tls.insecure: true`. No sampling processor. Memory limiter is 512 MiB.
- **Prometheus** (`prom/prometheus:v2.48.0`) — static scrape `petstore:8080/metrics`.
- **Grafana** (`grafana/grafana:10.2.2`) — Prometheus / Loki / Pyroscope / Jaeger datasources provisioned. No Tempo datasource. Anonymous admin enabled for dev.
- **Loki** (`grafana/loki:2.9.3`) — monolithic, filesystem chunks, compactor retention enabled but no `retention_period` set.
- **Promtail** (`grafana/promtail:2.9.3`) — DaemonSet scraping `brrtrouter-dev` pods. **No `pipeline_stages` with `json` parsing** — trace / span IDs are not extracted as labels.
- **Pyroscope** (`grafana/pyroscope:latest`) — deployed, no scrape config, pet_store not wired.

Pet_store deployment (`k8s/app/base/deployment.yaml`):

- `OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317`
- `OTEL_SERVICE_NAME=petstore`
- ConfigMap `observability.tracing_enabled: true, otlp_endpoint: "http://otel-collector:4317"`
- `BRRTR_LOG_FORMAT=json`, `BRRTR_LOG_SAMPLING_MODE=sampled`, `BRRTR_LOG_ASYNC=true`

All of this would work if the application honoured any of it.

### 4.2 Application code is a stub

`src/otel.rs` — 791 lines, file name is misleading. It configures `tracing`-based logging and implements two `Layer`s (`SamplingLayer` for log-event sampling, `RedactionLayer` for credential scrubbing), but **has no OpenTelemetry integration at all**:

- **No `TracerProvider`** — no `opentelemetry_sdk::trace::TracerProvider::builder()`, no OTLP exporter, no `BatchSpanProcessor`.
- **No bridge from `tracing` → OTEL** — `tracing_opentelemetry::layer()` is *not* composed into the subscriber stack. The stack is `Registry + EnvFilter + SamplingLayer + RedactionLayer + fmt::Layer(JSON or Pretty)` (lines 417‑479), writing to stdout.
- **`init_logging(_service_name, log_level, _otlp_endpoint)` ignores the endpoint argument** (line 338) and always delegates to `init_logging_with_config`.
- **Module doc says it's provisional**: *"OTLP export will be added in a future phase"* (lines 10‑11). Tests use `OpenTelemetryLayer` in `tests/tracing_util.rs`; production does not.
- **`shutdown()` is a no-op** (lines 485‑491). Comment is honest: *"reserved for future OTLP flush"*.
- **Resource attributes** (`service.name`, `service.version`, `deployment.environment`) are **not set** anywhere.

Net effect: a request served by BRRTRouter produces logs on stdout (JSON when configured) but emits zero network bytes to the OTLP endpoint. Jaeger's receiver never sees a single span for the pet_store service.

### 4.3 Span emission inside BRRTRouter is minimal

Across the entire `src/` tree:

- **Zero `#[instrument]` attributes** on any function — no automatic spanning of request-handling code paths.
- **Three `info_span!` sites total:**
  - `src/middleware/tracing.rs` ~70‑84: `http_request` span in `before`; entered and dropped inside `before` — **does not wrap downstream work**.
  - `src/middleware/tracing.rs` ~103‑124: `http_response` span in `after`; entered for the `info!` only.
  - `src/server/service.rs` ~920‑929: `info_span!("http_request", …)` with fields `method`, `path`, `header_count`, plus `status`/`duration_ms`/`stack_used_kb` declared as `Empty` and recorded by `RequestLogger::drop`.
- **Parse phase runs outside the main span.** `parse_request` (`server/service.rs` ~895‑917) runs *before* the `info_span!` is created (~919+). Per-request `debug!` logs in parse are not children of `http_request`.
- **Span `status` is declared but never recorded** — only `duration_ms` and `stack_used_kb` are filled in.
- **No propagation.** No code reads `traceparent` / `tracestate` / `baggage` from the request, no code injects them into responses or handler-originated outbound calls.

Net effect: even if OTLP were wired tomorrow, the resulting trace would be a single opaque `http_request` span per request with no sub-phases. We could not tell whether a 400 ms request spent 395 ms in the handler or in schema validation.

### 4.4 Metrics shape does not support SLO queries

From `src/server/service.rs::metrics_endpoint` (~444‑729) + `src/middleware/metrics.rs`:

- **`brrtrouter_request_duration_seconds` is a histogram, but has only `le` buckets — no `method` / `route` / `status` labels.** Per-route p95 is impossible; `histogram_quantile(0.95, rate(..._bucket[1m]))` returns one global series.
- **Per-path latency is three separate gauges**: `brrtrouter_path_latency_seconds_{avg,min,max}{path}`. Gauges cannot be re-aggregated across scrape intervals; they cannot produce percentiles; they cannot be windowed cleanly.
- **Schema validation**: no counter. A 400 response from a validation failure is indistinguishable from any other 400 in metrics.
- **Auth failures**: `brrtrouter_auth_failures_total` exists but has no `scheme` / `route` labels.
- **CORS**: `brrtrouter_cors_origin_rejected_total`, `brrtrouter_cors_preflight_total`, etc. exist — good shape, but dashboards don't surface them.
- **Worker pool / dispatcher queue**: `brrtrouter_worker_pool_queue_depth{handler}` exists (Phase 5.1) — good shape, dashboards don't surface it.

### 4.5 Dashboards are partially broken

Three dashboard ConfigMaps exist:

| ConfigMap | File | Mounted into Grafana? |
|---|---|---|
| `grafana-dashboards-config` (provisioning config) | `grafana.yaml` | ✅ |
| `grafana-dashboard-unified` (unified JSON) | `grafana.yaml` + `grafana-dashboard.yaml` | ✅ |
| `grafana-dashboard-petstore` (pet_store Quick View) | `grafana.yaml` | ✅ |
| `grafana-dashboard-pyroscope` (flamegraph) | `grafana.yaml` | ✅ |
| `grafana-dashboards` (brrtrouter-memory) | `grafana-dashboards.yaml` | ❌ **not mounted** |
| `grafana-dashboard-performance` (brrtrouter-performance) | `grafana-dashboard-performance.yaml` | ❌ **not mounted** |

So two of the most BRRTRouter-specific dashboards exist only as disconnected YAML. Grafana never loads them.

Misleading panels on the ones that *do* load:

- Unified "Memory Usage" (panel 11) queries `brrtrouter_coroutine_stack_bytes` — that's coroutine stack sizing, not process memory.
- Performance "Resource Usage vs Limits" panel labels itself as-if it knows cgroup limits; it actually queries `(rss_bytes / 1 GiB) * 100` with no real limit comparison.
- Performance "CPU vs Latency Correlation" overlays CPU rate with a scalar `brrtrouter_request_latency_seconds` gauge (not a histogram quantile) — the latency line is a single running average, not a percentile.

Coverage matrix (from the audit's dashboard walk):

| Concern | Coverage | Notes |
|---|---|---|
| Request rate total + per-route | ✅ | Unified + Memory |
| Latency **per-route** p50/p95/p99 | ⚠️ **Partial** | Only global; labels don't exist on the histogram |
| Status code **per-route** | ⚠️ **Partial** | Global pie; top-paths-by-failure table exists |
| Router match / cache | ❌ | No panel |
| Dispatcher queue depth | ❌ | Metric exists, no panel |
| Worker-pool saturation | ❌ | Metric exists, no panel |
| Schema validation errors | ❌ | No metric, no panel |
| Auth failures per scheme | ❌ | Counter unlabeled, no panel |
| CORS rejections | ❌ | Metric exists, no panel |
| Memory (RSS/heap/growth/peak) | ✅ | Memory dashboard (not mounted) + Performance (not mounted) |
| Jemalloc allocation rate | ⚠️ **Partial** | Only rate of generic `heap_bytes` |
| Coroutine count / stack usage | ⚠️ **Partial** | Stack bytes yes, count no |
| Accept latency / conn age | ❌ | — |
| Client disconnects (BrokenPipe) | ⚠️ **Partial** | Via `connection_closes_total` — combined |
| Hot-reload events | ❌ | — |
| Shutdown / drain progress | ❌ | — |
| Panic / handler error rate | ⚠️ **Partial** | Only surfaces as 5xx, no explicit counter |
| OTEL exporter health | ❌ | N/A today; gap once O.1 lands |
| Log volume by level | ⚠️ **Partial** | Loki explore only; no recorded rate |
| Trace volume (spans/sec, drops) | ❌ | N/A today; gap once O.1 lands |

Eighteen concerns. **Eight NO, seven PARTIAL, three YES.**

### 4.6 Log → trace correlation is impossible

Two failures compound:

1. Application side: `tracing_opentelemetry::layer()` is not installed, so `tracing::info!` records emitted while a span is active do *not* carry `trace_id` / `span_id` fields. The JSON output includes `span` metadata (the tracing span hierarchy) but no OTEL IDs.
2. Collector side: Promtail has no JSON-parsing pipeline stage, so even the existing `span` metadata doesn't become a Loki label. LogQL filtering must use `| json` at query time; no indexed label means no cheap filter.

Together: a user clicking "show logs for this trace" in Grafana Explore gets nothing useful.

### 4.7 Cluster config: two non-blocking issues

- `k8s/app/base/service.yaml` annotations say `prometheus.io/port: "9090"` but the container listens on `8080`. Prometheus uses a **static** scrape to `petstore:8080`, so metrics still land — but annotation-driven scraping (if turned on) would miss the service.
- `k8s/app/overlays/shared/patch-shared-infra.yaml` points OTLP to `otel-collector.observability.svc.cluster.local:4317`. The actual collector is in `brrtrouter-dev`. Only bites if the shared overlay is applied without a namespace override.

Neither blocks Phase O.1. Called out for tidying.

### 4.8 Memory middleware (carryover from hot-path PRD)

`src/middleware/memory.rs::log_stats` currently emits `tracing::warn!("High memory growth detected")` when `growth_bytes > 500 MB`. The original `> 100 MB` trigger was demoted to `> 500 MB` in `2b54c66` because it was firing on every legitimate 2000u ramp (coroutine stacks + connection buffers), but user feedback confirms there's a real RSS drift signal happening in soak that the new threshold now hides. Design tweak captured here as §Phase O.10; not the main focus.

### 4.9 Lifeguard already owns part of the observability contract 🔑

Lifeguard (at `../lifeguard/`) is embedded as a library in every microservice that uses BRRTRouter for data access. Auditing `lifeguard/src/metrics.rs`, `lifeguard/src/logging/`, and `lifeguard/docs/OBSERVABILITY_APP_INTEGRATION.md` reveals a **pre-existing, explicit contract** between the two crates that this PRD must honour rather than invent around:

**What Lifeguard already does:**

1. **Emits 7 `tracing::span!` sites on hot paths** (connect / execute_query / begin_transaction / commit_transaction / rollback_transaction / health_check / pool_slot_heal). Level `Level::INFO`, `target = "lifeguard"`, stable names `lifeguard.acquire_connection`, `lifeguard.execute_query` (with `query = %query` field), etc. Callers get span nesting for free — Lifeguard's executor/pool/transaction layers are already attributable.

2. **Prometheus-text metrics with `lifeguard_*` prefix.** `LifeguardMetrics::init()` installs a `SdkMeterProvider` backed by `opentelemetry_prometheus`, registers `lifeguard_pool_size`, `lifeguard_query_duration_seconds`, `lifeguard_acquire_latency_seconds`, etc. Exposes `lifeguard::metrics::prometheus_scrape_text()` for the host to concat onto its own `/metrics` response.

3. **Optional `lifeguard::channel_layer()`** — a `tracing_subscriber::Layer` that enqueues events onto a `may` mpsc channel drained to stderr by a background coroutine. Intended for hosts that want may-runtime-native log backpressure. Purely additive; does not contest the subscriber stack.

   **⚠️ Ambiguity under OTLP-native logs (introduced by v0.3 of this PRD):** when Phase O.1 installs `OpenTelemetryTracingBridge` as the primary log sink and removes the stdout `fmt::Layer`, `channel_layer()`'s "drain to stderr" semantics start producing a parallel log stream that *does* hit stdout — exactly what v0.3's architecture says should never happen under load. Three defensible resolutions (pick in a follow-up, not this PRD): (a) keep the feature flag default-off and document it as legacy-only in `OBSERVABILITY_APP_INTEGRATION.md`; (b) refactor Lifeguard's channel path to emit via an OTLP log client instead of stderr, so it becomes "OTLP with a may-mpsc-backed queue"; (c) retire it in favour of the `OpenTelemetryTracingBridge`. Recommendation: **(a) for now**, revisit in a Lifeguard PRD once this one lands.

4. **Documented contract** — `lifeguard/docs/OBSERVABILITY_APP_INTEGRATION.md` lays out four rules that this PRD must preserve verbatim:
   > (1) **One `TracerProvider` per process.** (2) **One `tracing` subscriber.** (3) **Lifeguard does not own OTel globals.** (4) **`channel_layer()` is optional.**

   It names `BRRTRouter/src/otel.rs` as **the** single place to own provider init. The contract is half-specified; this PRD is the other half.

**What Lifeguard does that breaks naive OTEL init:**

1. **`LifeguardMetrics::init()` calls `opentelemetry::global::set_meter_provider(...)` via `OnceCell::call_once`.** First caller wins. If BRRTRouter also calls `set_meter_provider` (which v0.1 of this PRD proposed in Phase O.1), ordering determines whose meter backs `global::meter(...)` — the loser silently emits into a dropped provider. This is the meter-provider race that v0.2 fixes.

2. **Lifeguard is on `may = "0.3"`** (same as BRRTRouter). No tokio is pulled in by Lifeguard; any tokio runtime we install for `BatchSpanProcessor` is BRRTRouter's alone. This is a constraint, not a conflict — we must not require Lifeguard consumers to accept a tokio dep.

3. **Lifeguard pins `opentelemetry = "0.29.1"`, `opentelemetry_sdk = "0.29.0"` (metrics feature only), `opentelemetry-prometheus = "0.29.1"`.** BRRTRouter must pin the same 0.29 line or we get two parallel OTEL stacks in the binary with silently incompatible globals. The 0.27 versions v0.1 of this PRD proposed are wrong.

**What Lifeguard deliberately leaves to the host:**

- All OTLP export (traces / metrics / logs).
- All propagator installation (W3C tracecontext / baggage).
- The subscriber `.try_init()` call.
- SIGTERM / flush handling for OTEL.
- All logic around incoming-request span parenting.

Everything in the left column is BRRTRouter's job; everything in the right column is Lifeguard's. The PRD must maintain that split — see Phase O.0.

## 5. Target architecture

### 5.1 Three telemetry streams, all direct-to-Collector over OTLP/gRPC

The previous `stdout → Promtail → Loki` log path is an anti-pattern under load: every `tracing::info!` takes a stdout FD lock that every coroutine contends for; Promtail then tails the same bytes off disk or docker-log rotation; inotify events pile up; chunked pushes to Loki introduce their own latency. Phase O.1 replaces it with **OTLP logs direct from the process to the OTEL Collector over gRPC**, alongside the OTLP traces it already sets up. Stdout stays as a **startup-and-panic-only** surface: what `tilt logs` shows is what the SRE actually wants to see — "did the server start?" and "did it abort?" — nothing else.

```
BRRTRouter process (embeds Lifeguard)
 │
 ├─ startup + panics (pure println!/eprintln!, bypasses tracing entirely)
 │     │
 │     └──► stdout / stderr ──► kubectl logs / tilt logs  (human surface, startup only)
 │
 ├─ tracing::span! + tracing::event!   ◄──── Lifeguard's `tracing::span!` sites
 │    │                                       (lifeguard.execute_query, .acquire_connection,
 │    │                                        .begin_transaction, …) emit into the same
 │    │                                        subscriber — BRRTRouter never touches
 │    │                                        Lifeguard's emission path.
 │    │
 │    ├── EnvFilter ──► RedactionLayer ──► SamplingLayer ──►  (split: spans vs events)
 │    │                                                            │
 │    │                                            ┌───────────────┴───────────────┐
 │    │                                            ▼                               ▼
 │    │                             tracing_opentelemetry::layer   opentelemetry_appender_tracing::OpenTelemetryTracingBridge
 │    │                                            │                               │
 │    │                                            ▼                               ▼
 │    │                              opentelemetry_sdk::TracerProvider   opentelemetry_sdk::logs::LoggerProvider
 │    │                                            │                               │
 │    │                                    BatchSpanProcessor              BatchLogRecordProcessor
 │    │                                            │                               │
 │    │                                  opentelemetry_otlp::SpanExporter  opentelemetry_otlp::LogExporter
 │    │                                            │                               │
 │    │                                            └─────────────┬─────────────────┘
 │    │                                                          │ gRPC over HTTP/2 (multiplexed on single connection)
 │    │                                                          ▼
 │    │                                              OTEL Collector :4317
 │    │                                                          │
 │    │                              ┌──── pipeline: traces ─────┼─── pipeline: logs ────┐
 │    │                              ▼                           │                       ▼
 │    │                        jaeger:4317 (OTLP) ─► Jaeger UI   │                       loki:3100 (Loki exporter)
 │    │                                                          │
 │    │                         [OPTIONAL dev-only fallback: if OTEL_EXPORTER_OTLP_ENDPOINT is unset,
 │    │                          install fmt::Layer to stdout so `cargo test` / `cargo run` still show logs]
 │    │
 │    └── (optional, behind `lifeguard-integration` feature) lifeguard::channel_layer()
 │                                          │
 │                                          ▼
 │                                  may mpsc channel → stderr drain   ⚠️ bypasses OTLP, see §4.9
 │
 ├─ /metrics (hand-rolled Prometheus text, BRRTRouter owns)
 │     ├── brrtrouter_* series (request rate, duration hist, worker pool, …)
 │     ├── process_memory_* series (from MemoryMiddleware)
 │     └── concat lifeguard::metrics::prometheus_scrape_text()  ← Lifeguard contributes lifeguard_* series
 │                    │
 │                    └── Lifeguard SEPARATELY owns global::set_meter_provider via OnceCell
 │                         (BRRTRouter must never call set_meter_provider — §Phase O.0)
 │                    ▼
 │              scraped by Prometheus  →  Grafana (Prometheus datasource)
 │                    │
 │                    └── (optional) OTEL Collector Prometheus receiver re-exports via OTLP
 │
 └─ (Phase O.12) pyroscope-rs continuous profiling ──► pyroscope:4040 ──► Grafana (Pyroscope datasource)
        Flamegraphs per service / pod / time-window. Forces stdout to be useless without it
        — matching the user directive "stdout only shows startup, force observability maturity".
```

Three data planes, all OTLP-native:

- **Logs** — `tracing::event!` → `OpenTelemetryTracingBridge` → `LoggerProvider` → OTLP/gRPC → Collector → Loki. `trace_id` / `span_id` are first-class OTLP log-record fields automatically when an event fires inside a span; no post-hoc JSON extraction required. Promtail's role is eliminated for runtime logs (Phase O.8 radically descoped).
- **Traces** — `tracing::span!` → `tracing_opentelemetry::layer` → `TracerProvider` → OTLP/gRPC → same connection → Collector → Jaeger. Lifeguard's spans ride the same pipeline for free.
- **Metrics** — Prometheus text via `/metrics` endpoint (BRRTRouter's + Lifeguard's via `prometheus_scrape_text()` concat). No OTEL Metrics SDK in BRRTRouter (§Phase O.0 contract). Collector's Prometheus receiver re-exports via OTLP if downstream consumers need it.
- **Profiles (Phase O.12)** — `pyroscope-rs` push-mode continuous profiling direct to `pyroscope:4040`. Separate pipeline from OTLP (Rust's OTLP-profiles signal is still experimental as of `opentelemetry-rust` 0.29).

Stdout serves exactly three lifetime events: **"binary started"**, **"routes registered, listening on :port"**, and **"panic" / "abort"** / graceful-shutdown completion. Anything else you'd want to know about a running service lives in Grafana, Jaeger, or Pyroscope.

### 5.2 Env var contract (OTLP-standard only)

| Variable | Default | Effect |
|---|---|---|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | **unset** → OTLP disabled | Collector endpoint, e.g. `http://otel-collector:4317` |
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `grpc` | `grpc` / `http/protobuf` / `http/json` |
| `OTEL_EXPORTER_OTLP_TIMEOUT` | `10s` | Exporter request timeout |
| `OTEL_SERVICE_NAME` | crate name | `service.name` resource attr |
| `OTEL_SERVICE_VERSION` | `CARGO_PKG_VERSION` | `service.version` resource attr |
| `OTEL_RESOURCE_ATTRIBUTES` | empty | Extra `k=v,k=v` resource attrs (e.g. `deployment.environment=dev`) |
| `OTEL_TRACES_SAMPLER` | `parentbased_always_on` | `parentbased_always_on` / `parentbased_traceidratio` / `always_off` |
| `OTEL_TRACES_SAMPLER_ARG` | `1.0` | Ratio for the ratio sampler |
| `OTEL_PROPAGATORS` | `tracecontext,baggage` | W3C propagators to install |
| `OTEL_BSP_SCHEDULE_DELAY` | `5s` | Batch span processor flush interval |
| `OTEL_BSP_MAX_EXPORT_BATCH_SIZE` | `512` | Batch span processor batch size |

Everything reads from env-standard OTLP variables. No BRRTR-prefixed variants for trace config. BRRTRouter's existing `BRRTR_LOG_*` variables stay and control only the log stream.

### 5.3 Safe default when OTEL unset

If `OTEL_EXPORTER_OTLP_ENDPOINT` is not set, `init_logging_with_config` behaves exactly as today: no `TracerProvider`, no `BatchSpanProcessor`, no network traffic, no span-allocation overhead. Only the stdout log subscriber runs. This keeps CI, local-dev, and tests unchanged; only the cluster (where the env is set via the deployment) gets OTLP.

## 6. Phases

Each phase ships as its own PR. Each PR must include the acceptance criteria below and a link back to this PRD.

### Phase O.0 — Lifeguard composition contract (documentation only) 🔑

**Scope:** codify the BRRTRouter ↔ Lifeguard observability contract in both repos so future changes on either side have a ruleset to be measured against. No Rust code changes in this phase.

**Ownership matrix — who installs what, exactly once per process:**

| Concern | Owner | Rationale |
|---|---|---|
| `tracing_subscriber::Registry::try_init()` | **BRRTRouter** | The host service calls `brrtrouter::otel::init_logging_with_config(&LogConfig::from_env())` once in `main()`. Lifeguard must never `try_init`. Lifeguard-feature-flagged library code only adds `Layer`s to an already-existing registry when asked. |
| `EnvFilter`, `fmt::Layer`, redaction, sampling | **BRRTRouter** | Composed inside `init_logging_with_config`. |
| `opentelemetry::global::set_text_map_propagator` | **BRRTRouter** | Single install during `init_logging_with_config`; `TraceContextPropagator` by default. |
| `opentelemetry::global::set_tracer_provider` | **BRRTRouter** | Single install during `init_logging_with_config`. Lifeguard explicitly declines (`lifeguard/docs/OBSERVABILITY_APP_INTEGRATION.md` rule 3). |
| `tracing_opentelemetry::OpenTelemetryLayer` | **BRRTRouter** | Composed into the subscriber in the same init call. Bridges `lifeguard::*` spans into BRRTRouter's `TracerProvider` automatically — Lifeguard needs no changes. |
| `opentelemetry::global::set_meter_provider` | **Lifeguard** (⚠️ by existing design) | Lifeguard's `LifeguardMetrics::init()` calls it via `OnceCell::call_once`. **BRRTRouter must NOT call `set_meter_provider`** — if BRRTRouter needs OTEL metrics, it obtains a `Meter` from whatever global provider is currently set (Lifeguard's if embedded, NoOp if not). See Phase O.6 for the metric-stream design that avoids this race entirely. |
| `/metrics` endpoint (Prometheus text) | **BRRTRouter** serves, **Lifeguard** contributes | `AppService::metrics_endpoint` writes `brrtrouter_*` and `process_memory_*` series; it MUST also call `lifeguard::metrics::prometheus_scrape_text()` when the `lifeguard` feature is enabled on the host and concatenate the result. Single scrape target for Prometheus. |
| OTLP span export (BatchSpanProcessor, SpanExporter) | **BRRTRouter** | Lives entirely inside BRRTRouter's SDK instance; Lifeguard emits `tracing` spans that flow through the bridge into this exporter. |
| OTLP metrics export | **Neither (Phase O.6 decision)** | See Phase O.6 — BRRTRouter's `/metrics` is scraped by Prometheus in-cluster; the OTEL Collector's `prometheus` receiver re-exports via OTLP if downstream OTLP metrics are needed. Avoids the `set_meter_provider` race. |
| OTLP log export | **Neither (for now)** | Logs continue via stdout → Promtail → Loki. Future phase if needed. |
| SIGTERM / flush orchestration | **BRRTRouter** | `ShutdownGuard` returned from `init_logging_with_config`; flushes BSP + log appender + closes tracer provider. Lifeguard's own `flush_log_channel()` (for its may-channel log path) is idempotent and can be called in any order. |

**Init order invariants:**

1. `main()` calls `brrtrouter::otel::init_logging_with_config(...)` **first**, returns `ShutdownGuard`.
2. BRRTRouter server + Lifeguard can initialise in any order after that. Lifeguard's first `METRICS` touch installs its Prometheus-backed meter provider; BRRTRouter never touches `set_meter_provider`.
3. `main()` holds `ShutdownGuard` for process lifetime; its `Drop` runs the flush chain before `std::process::exit`.

**Dependency pin:** BRRTRouter MUST track Lifeguard's OTEL major.

- `opentelemetry = "0.29"` (currently `0.29.1`)
- `opentelemetry_sdk = "0.29"` with features `rt-tokio-current-thread` (tracer + BSP)
- `opentelemetry-otlp = "0.29"` (span exporter; gRPC + HTTP/protobuf features as needed)
- `tracing-opentelemetry = "0.30"` (the 0.30 line pairs with otel 0.29 — confirm at PR time)

Any bump is a coordinated change across both repos. Captured in BRRTRouter's `Cargo.toml` with a comment pointing at this PRD and at Lifeguard's dep.

**Docs deliverables:**

1. This PRD (§4.9 + §Phase O.0) — already written.
2. `lifeguard/docs/OBSERVABILITY_APP_INTEGRATION.md` — add a "See also: BRRTRouter PRD_OBSERVABILITY_AND_TRACING.md §Phase O.0" cross-link. One-line edit.
3. `BRRTRouter/src/otel.rs` module-level rustdoc — replace the current "OTLP export will be added in a future phase" with a summary of the contract from this table.

**Acceptance criteria:**
- Lifeguard cross-link landed (`lifeguard/docs/OBSERVABILITY_APP_INTEGRATION.md`).
- `BRRTRouter/src/otel.rs` module docs reflect the Phase O.0 contract.
- No code changes merged; this phase is purely documentation to pin the contract before Phase O.1's implementation.

**Commit scope:** two small doc-only commits, one in each repo. Same day.

### Phase O.1 — OTLP exporter + tracing_opentelemetry bridge + OTLP logs appender 🚨 UNBLOCKS JAEGER AND ELIMINATES PROMTAIL RUNTIME PATH

**Scope:** make the current `http_request` span visible in Jaeger **and** make all runtime `tracing::event!`s land in Loki as OTLP-native log records. Startup println!s stay on stdout. Under this design, `tilt logs petstore` shows startup + panic only; everything else is in Grafana.

**Code:**

1. `Cargo.toml` — add (versions pinned to match Lifeguard's 0.29 line per §Phase O.0; diverging creates two parallel OTEL stacks with silently incompatible globals):
   - `opentelemetry = "0.29"` (API crate — must match `lifeguard`'s `0.29.1`)
   - `opentelemetry_sdk = { version = "0.29", features = ["rt-tokio-current-thread", "trace", "logs"] }` — note the `logs` feature addition vs v0.1
   - `opentelemetry-otlp = { version = "0.29", features = ["grpc-tonic", "http-proto", "tls", "logs"] }` — `logs` feature gates the `LogExporter`
   - `opentelemetry-appender-tracing = "0.29"` — **new in v0.3**; the `tracing::Subscriber` → OTLP logs bridge. Crate tracks the opentelemetry major exactly.
   - `tracing-opentelemetry = "0.30"` (pairs with otel 0.29 — verify at PR time; the tracing-opentelemetry major trails otel by 1)
   - `opentelemetry-semantic-conventions = "0.29"`

   Inline Cargo.toml comment: `# pinned to lifeguard's opentelemetry 0.29.x line (see docs/PRD_OBSERVABILITY_AND_TRACING.md §Phase O.0). Bumping is a coordinated cross-repo change.`

2. `src/otel.rs::init_logging_with_config` — build **both** a `TracerProvider` and a `LoggerProvider` against the same OTLP endpoint (gRPC connection is multiplexed, so it's literally one TCP connection for both signals):

   **Shared resource + endpoint setup** (executed once if `OTEL_EXPORTER_OTLP_ENDPOINT` is set):
   - Resource: `service.name` / `service.version` / `deployment.environment` from env + `OTEL_RESOURCE_ATTRIBUTES`.
   - OTLP client: a single `opentelemetry_otlp::new_exporter().tonic().with_endpoint(...)` base, reused for both exporters.

   **TracerProvider side** (unchanged from v0.1 of this PRD except version pin):
   - `opentelemetry_otlp::SpanExporter::builder()...build_span_exporter()?` honouring `OTEL_EXPORTER_OTLP_PROTOCOL` + `OTEL_EXPORTER_OTLP_TIMEOUT`.
   - `BatchSpanProcessor` honouring `OTEL_BSP_SCHEDULE_DELAY` + `OTEL_BSP_MAX_EXPORT_BATCH_SIZE`.
   - Sampler: `OTEL_TRACES_SAMPLER` + `OTEL_TRACES_SAMPLER_ARG`.
   - `opentelemetry::global::set_tracer_provider(provider.clone())` — **BRRTRouter owns this global** (Lifeguard declines per §Phase O.0).
   - `opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new())` — **BRRTRouter owns this global** too.
   - Compose `tracing_opentelemetry::layer().with_tracer(provider.tracer("brrtrouter"))` into the subscriber stack after `RedactionLayer`.

   **LoggerProvider side** (new in v0.3):
   - `opentelemetry_otlp::LogExporter::builder()...build_log_exporter()?` honouring `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` (falls back to `OTEL_EXPORTER_OTLP_ENDPOINT`) + protocol + timeout env vars.
   - `BatchLogRecordProcessor` honouring `OTEL_BLRP_SCHEDULE_DELAY` + `OTEL_BLRP_MAX_EXPORT_BATCH_SIZE` (OTEL-standard env var names).
   - Build `LoggerProvider` with the same resource as the tracer.
   - Compose `opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&logger_provider)` into the subscriber stack **in place of** the previous `fmt::Layer` that wrote JSON to stdout.
   - `trace_id` / `span_id` are automatically attached to each log record when a span is active — the OTLP log-record schema has them as first-class fields; no custom injection layer needed.

   **⚠️ Do NOT call `opentelemetry::global::set_meter_provider`** — per §Phase O.0, Lifeguard owns that global via `OnceCell::call_once`. BRRTRouter's metrics stay in the Prometheus-text `/metrics` endpoint (Phase O.6).

   **Dev-mode fallback** — if `OTEL_EXPORTER_OTLP_ENDPOINT` is unset, install a `fmt::Layer` to stdout in its place. This keeps `cargo test` / `cargo run` showing logs on the terminal for local development. Toggling is automatic based on env var presence.

3. **Startup logs stay on stdout via plain `println!`/`eprintln!`** — no `tracing::info!` calls in the startup path. The pet_store template and `AppService::start` already do this; any stray `tracing::info!` in boot code gets demoted to plain `println!`. The invariant is: *"if you see a log line in `kubectl logs`, either the process is starting, it panicked, or it's graceful-shutting-down — nothing else"*.

4. Module-level rustdoc in `src/otel.rs` — replace the current "OTLP export will be added in a future phase" stub with an explicit reference to §Phase O.0's ownership table and the log-routing invariant in (3).

**Acceptance criteria (updated from v0.1 to cover both signals):**

- Set `OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317` on pet_store deployment, `kubectl rollout restart deploy/petstore`, hit `/pets` a handful of times.
- Within 30 s, a `http.server.request` span appears in the Jaeger UI for `service=petstore`, with `http.request.method=GET`, `url.path=/pets`, `http.response.status_code=200`.
- Within 30 s, log records for the same requests are queryable in Grafana's Loki datasource; each record has `trace_id` / `span_id` attributes populated and links back to the matching Jaeger span.
- `kubectl logs deploy/petstore` output contains **only** startup lines (route registration, "server listening on :8080", OTEL init acknowledgement) — no per-request logs.
- Unset `OTEL_EXPORTER_OTLP_ENDPOINT`, restart, confirm: no OTLP outbound traffic, logs fall back to stdout JSON fmt layer, `cargo test` still shows terminal logs.

**Commit scope:** ~400 LOC in `src/otel.rs` + `Cargo.toml` + `tests/otel_exporter_integration.rs` (now covering both span and log receivers). One PR.

3. New type `ShutdownGuard` returned by `init_logging_with_config`. `Drop` calls `provider.shutdown()` (flushes BSP) and `non_blocking` appender guard's drop. Callers must `std::mem::forget` or keep it for process lifetime — documented.

4. Propagators: `opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new())` (tracecontext by default; baggage too if in `OTEL_PROPAGATORS`).

**Semantic conventions:** The span created by `AppService::call` renamed to follow `http.server.request` with fields per OTEL semconv 1.26:
- `http.request.method`
- `url.path`
- `url.scheme`
- `server.address` / `server.port`
- `user_agent.original`
- `http.response.status_code` (filled by `RequestLogger::drop`)
- `http.route` (filled after routing) — stable low-cardinality label.

Field renaming is a breaking change for any external log consumer that reads the old JSON field names; tests + dashboards updated in this phase.

**Tests:**
- `tests/otel_exporter_integration.rs` — spin up a local OTLP gRPC receiver fixture (use `opentelemetry-stdout` or a minimal in-proc `tonic` server), assert spans arrive with expected fields.
- Existing tests stay green without env vars set.

**Acceptance criteria:**
- Set `OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317` on pet_store deployment, `kubectl rollout restart deploy/petstore`, hit `/pets` a handful of times.
- Within 30 s, a `http.server.request` span appears in the Jaeger UI for `service=petstore`, with `http.request.method=GET`, `url.path=/pets`, `http.response.status_code=200`.
- Unset the env, restart, confirm no OTLP traffic (no inbound spans on collector logs, no outbound connections from pet_store to port 4317).

**Commit scope:** ~250 LOC across `Cargo.toml`, `src/otel.rs`, `tests/otel_exporter_integration.rs`, `tests/tracing_util.rs` (align test harness with production). One commit, one PR.

### Phase O.2 — W3C trace-context propagation

**Scope:** BRRTRouter participates in distributed tracing.

**Change:**

1. `src/server/service.rs::call` — before creating the `http.server.request` span, extract `traceparent` / `tracestate` / `baggage` from the request headers using `opentelemetry::global::get_text_map_propagator(...).extract(&extractor)`. Set the extracted context as the parent of the `http.server.request` span.
2. `src/server/response.rs::encode` — inject the span's trace context into the response under `traceparent` if `BRRTR_EMIT_TRACEPARENT_RESPONSE=1` (default off — most servers don't emit; Hauliage may opt in).
3. Extractor/injector shim crates: `opentelemetry-http` or hand-rolled. Prefer hand-rolled to avoid pulling `http-body`.

**Acceptance criteria:**
- Upstream client emits `traceparent: 00-<trace-id>-<span-id>-01`; BRRTRouter's span appears as a child of that `trace-id` in Jaeger.
- `BRRTR_EMIT_TRACEPARENT_RESPONSE=1` causes the response header to be set; Hauliage can then continue the trace onward.

**Commit scope:** ~120 LOC, new helper module `src/server/trace_context.rs`.

### Phase O.3 — Span catalog

**Scope:** the `http.server.request` span tree becomes useful. Every phase that can cost > ~100 µs at 2000u is a child span.

**Spans to add** (all children of `http.server.request`):

| Span name | Emitted from | Fields | Rationale |
|---|---|---|---|
| `brrtrouter.parse_request` | `server/request.rs::parse_request` | `header_count`, `body_bytes`, `method`, `url.path` | Today, parse logs run *outside* the main span (§4.3). This fixes it and gives visibility into parse-vs-handler cost split. |
| `brrtrouter.router.match` | `router/core.rs::route` | `http.route` (after match), `path_params_count`, `match_algorithm=radix_tree` | The "Slow route matching detected" log condition becomes a span attribute `duration_us`. No per-request `warn!`. |
| `brrtrouter.middleware.before` / `.after` | generic middleware wrapper | `middleware_name` | Auth/CORS/metrics cost visible. |
| `brrtrouter.dispatcher.dispatch` | `dispatcher/core.rs` | `handler_name`, `queue_depth_at_enqueue`, `backpressure_mode` | Handoff latency from accept → worker. |
| `brrtrouter.handler.execute` | inside the worker coroutine | `handler_name`, `http.response.status_code` (set on drop) | Actual handler business logic. This is the span Hauliage will extend. |
| `brrtrouter.schema.validate_request` | `spec/` validator | `schema_id`, `field_count`, `validation_errors_count` | Request schema cost. Needed to attribute 400s. |
| `brrtrouter.schema.validate_response` | `spec/` validator | `schema_id`, `status_code`, `validation_errors_count` | Response schema cost. |
| `brrtrouter.response.encode` | `server/response.rs::encode` | `status`, `body_bytes`, `header_count`, `content_type` | Serialise + write cost. |

**Style:**
- Use `#[tracing::instrument(level = "debug", skip_all, fields(…))]` on functions where possible; `info_span!` at request-lifecycle boundaries. Level `debug` for internal spans keeps them out of the exporter at default sampler — but attach-to-parent still works so if the outer `http.server.request` is sampled, the children are sampled.
- Never emit `warn!`/`info!` inside a per-request hot-path span (Phase 2.2 hygiene). Put signal on the span's fields; let the exporter / Jaeger do the aggregation.

**Acceptance criteria:**
- A single request to `GET /pets/{id}` produces a 7-span tree in Jaeger: `http.server.request` → `parse_request`, `router.match`, `middleware.before` (per registered middleware), `dispatcher.dispatch`, `handler.execute` → (if applicable) `schema.validate_request` → `schema.validate_response`, `middleware.after`, `response.encode`.
- Wall-clock sum of child span durations ≤ parent duration + 5 % (no missing time).
- `cargo test --lib` passes; no hot-path throughput regression > 2 % vs `b1fc30b` at `BRRTR_BENCH_SCOPE=openapi`.

**Commit scope:** ~400 LOC across `src/router/core.rs`, `src/dispatcher/core.rs`, `src/server/service.rs`, `src/server/request.rs`, `src/server/response.rs`, `src/spec/`, `src/middleware/`. Three commits, one PR (split per module).

### Phase O.4 — Resource attributes & service metadata

**Scope:** every span / log / metric is attributable to the right `service.name` / `service.version` / `deployment.environment`.

**Change:**

1. `src/otel.rs` reads:
   - `service.name` ← `OTEL_SERVICE_NAME` or crate name from `env!("CARGO_PKG_NAME")`.
   - `service.version` ← `OTEL_SERVICE_VERSION` or `env!("CARGO_PKG_VERSION")`.
   - `deployment.environment` ← `OTEL_RESOURCE_ATTRIBUTES` parsed; fall back to the value of `ENVIRONMENT` or literal `"unknown"`.
   - `host.name` ← `hostname` syscall.
   - `container.id` ← read `/proc/self/cgroup` when present.
2. These apply to traces (via `TracerProvider.resource(…)`) and to logs (via tracing-subscriber fmt layer's base fields — write a tiny custom layer).
3. Prometheus exposition: already has `{instance}` label per scrape; add a stable `service` label via relabel_configs in `prometheus.yaml` — no app change.

**Acceptance criteria:**
- Jaeger service list shows `petstore` (not `unknown-service`).
- Each span's `service.version` equals the Cargo version.
- Loki log entries carry a `service_name="petstore"` label after Phase O.8.

**Commit scope:** ~60 LOC; mostly in `src/otel.rs`.

### Phase O.5 — Graceful shutdown

**Scope:** SIGTERM loses no telemetry.

**Change:**

1. `ShutdownGuard` (introduced in O.1) is returned by `init_logging_with_config` and stored by the binary in `main()`.
2. `ShutdownGuard::Drop`:
   - `provider.force_flush().await` (or sync variant) with 5 s timeout.
   - `provider.shutdown()`.
   - Drop the `tracing_appender::non_blocking` guard last (flushes the log appender).
3. `pet_store/src/main.rs` installs a signal handler (SIGTERM / SIGINT) that drops the guard before `std::process::exit`.
4. `Cargo.toml` — add `signal-hook = "0.3"` or equivalent if not already pulled in transitively.

**Acceptance criteria:**
- `kubectl rollout restart deploy/petstore` while under 500u Goose load: Jaeger still shows the spans for the last batch of in-flight requests (no truncated trace windows); Loki shows the matching log records.
- `cargo test --lib` passes; no test regression.

**Commit scope:** ~80 LOC across `src/otel.rs`, `examples/pet_store/src/main.rs`, `templates/main.rs.txt` (so generated services get the same treatment).

### Phase O.6 — Metric improvements (Prometheus-text, no OTEL Metrics SDK)

**Scope:** make `/metrics` support per-route SLO queries and surface the operational counters we already emit but don't aggregate well. **Explicitly NOT scope:** installing an OTEL Metrics SDK in BRRTRouter. Per §Phase O.0 and §4.9, Lifeguard already owns `global::set_meter_provider` via `OnceCell`; BRRTRouter calling `set_meter_provider` would race that install. Instead, BRRTRouter keeps its existing hand-rolled Prometheus-text `/metrics` response, adds the labels it's missing, and concatenates Lifeguard's `prometheus_scrape_text()` output. The OTEL Collector's `prometheus` receiver scrapes `/metrics` and re-exports via OTLP if downstream consumers want OTLP-native metrics.

**Add to `AppService::metrics_endpoint` (Prometheus text, hand-written — no OTEL Metrics SDK calls):**

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `http_server_request_duration_seconds` | **histogram** | `method`, `http_route`, `status_class` | Per-route latency, OTEL semconv-aligned. Replaces the unlabeled `brrtrouter_request_duration_seconds`. |
| `http_server_active_requests` | gauge | `method`, `http_route` | In-flight per route. |
| `brrtrouter_schema_validation_errors_total` | counter | `http_route`, `direction` (`request`/`response`), `error_kind` | Schema failures, previously only logged. |
| `brrtrouter_auth_failures_total` | counter | `scheme`, `http_route` | Auth failures with dimension to find misconfigured clients. |
| `brrtrouter_dispatcher_queue_depth` | gauge | `handler` | Already exists; formalise label name + meaning. |
| `brrtrouter_worker_pool_shed_total` | counter | `handler`, `mode` (`shed` / `block_timeout`) | Phase 5.1 backpressure. |
| `brrtrouter_otel_exporter_queue_depth` | gauge | — | BSP queue depth; detects OTLP export backpressure. Read from `BatchSpanProcessor` (BRRTRouter-local — no global meter). |
| `brrtrouter_otel_exporter_dropped_spans_total` | counter | — | BSP drop counter. |
| `brrtrouter_handler_panics_total` | counter | `handler` | Panic-caught events; complements 5xx. |
| `brrtrouter_client_disconnect_total` | counter | `phase` (`read` / `write`) | BrokenPipe / ECONNRESET. |

**Deprecate (keep emitting for one minor release):** `brrtrouter_request_duration_seconds` (unlabeled), `brrtrouter_path_latency_seconds_{avg,min,max}` (gauges — cannot re-aggregate).

**Histogram bucket set:** `[1ms, 2.5ms, 5ms, 10ms, 25ms, 50ms, 100ms, 250ms, 500ms, 1s, 2.5s, 5s, 10s, +Inf]` — geometric sequence, OTEL-recommended for HTTP.

**Lifeguard integration (the Phase O.0 concat pattern):**

```rust
// In AppService::metrics_endpoint, after writing all brrtrouter_* series:
#[cfg(feature = "lifeguard-integration")]
if let Some(lg_text) = lifeguard::metrics::prometheus_scrape_text() {
    body.push_str(&lg_text);
}
```

Gated behind a new `lifeguard-integration` cargo feature (default **off**); pet_store and host services that actually embed Lifeguard enable it in their own `Cargo.toml`. BRRTRouter core stays Lifeguard-agnostic.

**Acceptance criteria:**
- `histogram_quantile(0.95, sum by (http_route, le) (rate(http_server_request_duration_seconds_bucket[1m])))` returns a series per route in Prometheus.
- Schema validation failure → exactly one increment of `brrtrouter_schema_validation_errors_total`.
- With `lifeguard-integration` feature on, `/metrics` response contains both `brrtrouter_*` and `lifeguard_*` series in one scrape.
- BRRTRouter MUST NOT call `opentelemetry::global::set_meter_provider` in any code path reachable from `init_logging_with_config`. Enforced via a `deny(clippy::disallowed_methods)` lint rule.
- Deprecated metrics documented in release notes; dashboards updated in the same PR (Phase O.9).

**Commit scope:** ~300 LOC across `src/middleware/metrics.rs`, `src/server/service.rs::metrics_endpoint`, `src/middleware/auth.rs`, `src/spec/validator.rs`, plus a new `src/server/exporter_health.rs` for reading BSP queue depth / drop counts from the tracer provider (no OTEL Metrics — just Prometheus text).

### Phase O.7 — Log → trace correlation (trivial under OTLP-native logs)

**Scope:** confirm `trace_id` and `span_id` are present on every log record arriving at Loki and that Grafana's log→trace / trace→log navigation works end-to-end.

**What v0.1 of this PRD proposed:** a custom `tracing_subscriber` layer that copies `SpanContext::trace_id()` / `span_id()` into JSON log fields so Promtail could extract them.

**Why v0.3 collapses it:** OTLP log records have `trace_id` and `span_id` as **first-class schema fields** (not JSON attributes). When `OpenTelemetryTracingBridge` (installed in O.1) converts a `tracing::event!` into an OTLP `LogRecord`, if there's an active OTEL span on the current context, the bridge populates `LogRecord.trace_id` + `LogRecord.span_id` automatically. No custom layer, no JSON extraction, no regex.

**What's left in this phase:**

1. `k8s/observability/grafana.yaml` — update the Loki datasource's `derivedFields` to point to Jaeger using the OTLP log record's native `trace_id` attribute (not a parsed JSON field). LogQL `| json | __trace_id__ = "..."` or equivalent; exact syntax depends on Grafana ≥ 10.2's OTLP-log support.

2. In the reverse direction, configure the Jaeger datasource's `tracesToLogsV2` mapping to use `service.name` + the span's time range to query Loki for records matching the span.

**Acceptance criteria:**
- In Grafana Explore → Loki, a log line for `service_name=petstore` during a request shows `trace_id` / `span_id` as surfaced OTLP attributes (not JSON fields).
- Click "View trace" → opens the matching Jaeger span in the trace panel.
- From any Jaeger span → "View logs" → opens a Loki query that returns the same record.

**Commit scope:** YAML-only (Grafana datasource config).

### Phase O.8 — Promtail eviction (REMOVED; replaced with narrow startup-only scrape)

**Scope change vs v0.1:** v0.1 of this PRD proposed extending Promtail with JSON pipeline stages to extract `trace_id`. v0.3 deletes that entirely. Under O.1's OTLP-native log emission, **runtime logs never hit stdout or disk** — they go direct to the Collector over gRPC. Promtail has no runtime role.

**What Promtail retains:**

- Scrapes pod stdout for the few **startup lines** (`println!` from boot code) and **panic** / **abort** output (writes that happen when the OTLP pipeline isn't up or the process is dying).
- Ships them to Loki under a separate label (`source="stdout-startup"`) so they don't collide with the OTLP-native log stream.
- Retention on this stream can be very short (24 h) — they're only useful for "why didn't the pod come up" debugging.

**Change:**

1. `k8s/observability/promtail.yaml` — relabel the `kubernetes-pods` job to set `source="stdout-startup"` on every record, so operators can filter runtime OTLP logs vs startup-console output in the same Loki instance.
2. `k8s/observability/loki.yaml` — set a short retention (`retention_period: 24h`) for the `source="stdout-startup"` stream via a stream-level retention rule.
3. No pipeline_stages JSON extraction — not needed.

**Acceptance criteria:**
- Under load, `kubectl logs deploy/petstore | wc -l` grows only during startup / shutdown. Steady-state load produces **zero** new lines on stdout.
- `{source="stdout-startup"}` in Loki shows exactly the startup banner for each pod restart.
- `{source="otel"}` (or equivalent) in Loki shows the OTLP-delivered runtime stream.

**Commit scope:** YAML-only; much smaller than v0.1's proposal.

### Phase O.9 — Dashboard overhaul

**Scope:** the coverage matrix (§4.5) has no NO rows. Misleading panels are fixed. Currently unmounted ConfigMaps are actually loaded.

**Changes:**

1. `k8s/observability/grafana.yaml` — mount the two currently-disconnected ConfigMaps:
   - `grafana-dashboards` (brrtrouter-memory.json)
   - `grafana-dashboard-performance` (brrtrouter-performance.json)
   as `subPath` mounts under `/var/lib/grafana/dashboards/`.

2. **Fix misleading panels:**
   - Unified "Memory Usage" → rename to "Coroutine Stack" and keep the `brrtrouter_coroutine_stack_bytes` query; or replace with `process_memory_rss_bytes`.
   - Performance "Resource Usage vs Limits" → either query the cgroup `container_memory_working_set_bytes{pod=~"$pod"}` vs `kube_pod_container_resource_limits{pod=~"$pod",resource="memory"}`, or rename to "RSS vs 1 GiB".
   - Performance "CPU vs Latency Correlation" → replace `brrtrouter_request_latency_seconds` scalar with a histogram-quantile p95.

3. **Add new dashboard "BRRTRouter — Request-level"** (new file `k8s/observability/grafana-dashboard-request.yaml`):
   - Per-route request rate: `sum by (http_route) (rate(http_server_request_duration_seconds_count[1m]))`
   - Per-route latency heatmap: `sum by (http_route, le) (rate(http_server_request_duration_seconds_bucket[1m]))`
   - Per-route p50/p95/p99: `histogram_quantile(…, sum by (http_route, le) (rate(...)))`
   - Per-route status mix: `sum by (http_route, status_class) (rate(http_server_request_duration_seconds_count[1m]))`
   - Schema validation failure rate (by route + direction + error_kind).
   - Auth failure rate (by scheme + route).
   - CORS decision counters (origin_rejected / preflight).
   - Worker-pool queue depth + shed counter (per handler).
   - Client disconnect rate (read / write phase).

4. **Add new dashboard "BRRTRouter — Telemetry health"**:
   - OTEL BSP queue depth over time.
   - OTEL BSP dropped spans counter.
   - Log volume by level per service: `sum by (level, service_name) (count_over_time({service_name=~".+"} [1m]))`.
   - Prometheus scrape health: `up{job="petstore"}`, scrape duration, series count.

5. **Variables on every dashboard:** `$namespace`, `$pod`, `$http_route` (pulled from the histogram labels) — so each dashboard can filter to a single service/pod/route.

6. **Grafana 10.2 panel type updates:** replace legacy `graph` panels with `timeseries`; replace old heatmap with the new Grafana Heatmap panel.

**Acceptance criteria:**
- All 6 ConfigMaps mount; Grafana loads 5 dashboards (unified, petstore quick-view, memory, performance, request-level, telemetry-health, pyroscope).
- Coverage matrix (§4.5) has no NO rows (measured against a fresh snapshot).
- Screenshot of each dashboard attached to the PR.

**Commit scope:** YAML + large JSON blobs; ~1200 lines combined. One PR.

### Phase O.10 — Memory middleware tuning

**Scope:** restore sensitivity to real slow leaks without flooding on ramp.

**Design:** replace the single-threshold `if growth_mb > 500` check with:

1. **Warmup window** — first 5 min of process life (tracked via `started_at: Instant` added to `MemoryTracker`), *never* emit the "High memory growth" warn. Coroutine stacks + connection buffers ramp freely during warmup; real leaks take minutes-to-hours to materialise anyway.
2. **Bucket-crossing warn** — after warmup, emit `warn!("Sustained memory growth detected")` *once* per 100 MB bucket crossed, not on every 10 s poll. Track `last_warn_growth_bucket: AtomicU64`. Crossing 100 MB fires once; crossing 200 MB fires once; etc. Warns include `growth_mb`, `rate_mb_per_min`, `uptime_sec`.
3. **Rate-of-growth metric** — new `brrtrouter_memory_growth_rate_bytes_per_second` gauge (EMA over the last 60 s). Alerting by rate rather than absolute is the cleaner pattern; the warn is a backstop.

**Acceptance criteria:**
- 2000u ramp produces zero warns in the first 5 min. If the process does leak to +100 MB after warmup, exactly one warn.
- Dashboards show the growth-rate gauge.

**Commit scope:** ~80 LOC in `src/middleware/memory.rs` + a couple of unit tests.

### Phase O.11 — Perf-PRD integration: per-span before/after

**Scope:** the Phase R bench harness records mean span durations per phase, not just aggregate req/s.

**Change:**

1. `scripts/run_goose_tests.py` — after each Goose run, query the OTEL Collector's Prometheus endpoint (`:8889`) for span-derived metrics:
   - `histogram_quantile(0.95, sum by (span_name, le) (rate(otelcol_exporter_sent_spans_bucket[30s])))` — but the right form depends on whether we use `spanmetrics` processor in the Collector. Add it in this phase.
2. Output a comparison table: `span_name | p50 | p95 | p99 | count | delta_vs_baseline`.
3. Hot-path PRD's future Phase R measurements become "the router.match span p95 dropped from 8 µs to 5 µs", not "throughput went from 81.5k to 83.2k".

This closes the loop: perf work finds the right spans to attack, observability work provides the ruler, and the next perf measurement uses the ruler.

**Commit scope:** Collector config change + Python script update.

### Phase O.12 — Continuous profiling via Pyroscope (flamegraphs)

**Scope:** once stdout is silent under load (Phases O.1 + O.8) and Jaeger shows where in the request tree the time goes (Phases O.3 + O.11), the last unknown is **where in the CPU stack a hot span burns its cycles**. Pyroscope is already deployed in the cluster (`k8s/observability/pyroscope.yaml`), unwired from pet_store. This phase wires it.

**Rationale:** the user directive that triggered this PRD section — *"this will force us to get mature about our observability stack, including flamegraphs"* — is satisfied by having continuous profiling as a first-class panel alongside metrics and traces. Without it, a regression where `router.match` p95 goes from 5 µs to 15 µs offers no guidance on *which line* of the radix walk is slower; Pyroscope's flamegraph tells you.

**Design choice — push vs scrape:**

- **Push-based (`pyroscope-rs`)** — the pet_store process runs a background sampler thread, pushes pprof profiles to `pyroscope:4040` every N seconds. Pros: no auth/plumbing on the cluster side, works identically inside + outside kind. Cons: introduces a profiler thread into the may runtime.
- **Scrape-based (`parca-agent` eBPF or Pyroscope Kubernetes integration)** — sidecar or DaemonSet samples kernel stacks by cgroup. Pros: zero in-process code. Cons: needs privileged host access, awkward on macOS-hosted kind.

**Recommendation: push-based `pyroscope-rs` for Phase O.12.** Add the sampler as a feature-flagged dep; the sampler runs on a dedicated OS thread (not a `may` coroutine — avoids stack-usage contention on the coroutine runtime).

**Code:**

1. `Cargo.toml` — add (all optional, behind a new `profiling` feature — default off):
   - `pyroscope = { version = "0.5", optional = true }`
   - `pyroscope-pprofrs = { version = "0.2", optional = true }` — the pprof sampler backend.

2. `src/otel.rs` — new `init_profiling(config: &ProfilingConfig) -> Option<PyroscopeHandle>`:
   - Reads `PYROSCOPE_SERVER_ADDRESS` (e.g. `http://pyroscope.observability:4040`), `PYROSCOPE_APPLICATION_NAME` (defaults to `OTEL_SERVICE_NAME`), `PYROSCOPE_SAMPLE_RATE` (default 100 Hz).
   - If the server address is unset, returns `None` — opt-in, zero overhead when not configured.
   - Returns a handle that, when dropped, shuts down the sampler.

3. `examples/pet_store/src/main.rs` — call `init_profiling(...)` after `init_logging_with_config(...)`, keep the handle in `main()` alongside the `ShutdownGuard`.

4. `k8s/app/base/deployment.yaml` — add `PYROSCOPE_SERVER_ADDRESS=http://pyroscope.observability:4040` and `PYROSCOPE_APPLICATION_NAME=petstore` env vars.

5. `k8s/observability/grafana.yaml` — the Pyroscope datasource is already configured; no change needed. Verify `/Explore → Pyroscope → petstore` shows an updating flamegraph.

**Acceptance criteria:**
- With the `profiling` feature enabled and `PYROSCOPE_SERVER_ADDRESS` set, Grafana's Pyroscope datasource shows a live flamegraph for `service_name=petstore` within 30 s of a request being served.
- With either the feature off or the env unset, no sampler runs — measurable via `ps -T` showing no extra thread.
- Overhead benchmark: 100 Hz sampling should cost <1 % of aggregate CPU time; verify by running the `openapi` Goose bench with profiling on vs off.

**Commit scope:** ~150 LOC across `src/otel.rs`, `Cargo.toml`, pet_store main, deployment YAML.

## 7. Risk & trade-offs

| Risk | Likelihood | Mitigation |
|---|---|---|
| OTLP exporter adds allocator / CPU overhead on the hot path | Medium | `BatchSpanProcessor` is the standard OTEL pattern (one allocation per span, async flush). Benchmark before/after in Phase O.3 — require throughput delta ≤ 2 % at `BRRTR_BENCH_SCOPE=openapi`. |
| Adding `opentelemetry` + `tracing-opentelemetry` pulls a large dep graph | High | Accept it. This is the minimum viable OTEL footprint. **Must pin to Lifeguard's 0.29 line** — see next row. |
| **Version skew with Lifeguard's OTEL deps** | **High** | Lifeguard uses `opentelemetry = "0.29.1"`, `opentelemetry_sdk = "0.29.0"`, `opentelemetry-prometheus = "0.29.1"`. If BRRTRouter pins a different major, the binary gets two parallel OTEL stacks with incompatible globals — traces may emit from BRRTRouter's provider but Lifeguard's meter provider can't be seen from BRRTRouter's code (different `global::` slots). Pin BRRTRouter to the same 0.29 line (§Phase O.0). Any bump is a coordinated cross-repo change. |
| **`set_meter_provider` race with Lifeguard** | **High** (happens on literally every service that embeds both) | Lifeguard's `LifeguardMetrics::init()` calls `global::set_meter_provider` via `OnceCell`. If BRRTRouter also calls it, ordering determines the winner. Mitigation: **BRRTRouter never calls `set_meter_provider`** — enforced via `deny(clippy::disallowed_methods)` rule (§Phase O.6). All BRRTRouter metrics stay in hand-rolled Prometheus text. |
| `tracing_opentelemetry::layer` doubles Lifeguard's span cost | Medium | Lifeguard spans already flow through whatever subscriber the host installs. Adding the OTEL layer means each Lifeguard span also allocates an OTEL `Span` record. Benchmark impact on a Lifeguard-heavy path (execute_query in a loop) before signing off on Phase O.1. Expected overhead: a few µs per span; acceptable for the diagnostic win. |
| W3C propagation introduces a per-request HashMap lookup | Low | `opentelemetry` propagators are cheap; baseline + measure. |
| Per-route labels on histograms explode cardinality | Medium | Use `http.route` (the OpenAPI-matched pattern — low cardinality) not `url.path` (user-supplied, unbounded). Enforce via explicit `route` field set only after router match. |
| Existing dashboards break when underlying metric names change | Medium | O.6 deprecates the old metrics alongside the new; both emit for one release; dashboards ship the new queries as part of O.9. |
| Promtail structured-metadata requires Loki ≥ 2.9 | Low | We're on 2.9.3 (confirmed in audit). Note minimum version in PRD. |
| `ShutdownGuard` lifetime misuse (drop too early → lost telemetry) | Low | Document; keep guard in `main()` as last statement before `exit()`. |
| Rust `opentelemetry` crate is still young — API churn | High | Pin to the 0.29 family (coupled to Lifeguard). Plan a minor crate bump per quarter as a coordinated cross-repo change. |
| Lifeguard's `prometheus_scrape_text()` returns a text body with its own `# HELP` / `# TYPE` preamble — potential duplicate series if we naively concat | Low | `opentelemetry-prometheus` is well-behaved here (emits unique metric names with unique `# HELP`). Test: scrape endpoint through `promtool check metrics` as part of Phase O.6 CI. |
| **OTLP log export backpressure drops log records** | Medium | `BatchLogRecordProcessor` has a fixed queue size; under sustained WARN/ERROR burst (e.g. 404 storm) the exporter will drop on queue overflow. Default queue size is 2048; tune via `OTEL_BLRP_MAX_QUEUE_SIZE`. Expose a `brrtrouter_otel_log_dropped_records_total` Prometheus counter (Phase O.6) so operators can alert on sustained drops. Contrast with Promtail-on-disk which has effectively unbounded queue (at cost of disk IO). Acceptable trade for the stdout-silence benefit. |
| **Silencing stdout makes `kubectl logs` useless for debugging in-flight incidents** | **Medium-High** — this is the deliberate directive from the user ("force observability maturity"), not an accident | Mitigation by process, not by code: (a) incident runbooks reference Grafana / Jaeger, not `kubectl logs`; (b) Phase O.12 ships flamegraphs so the new "primary debugging surface" is rich enough; (c) panics / aborts / OTEL-init-failures still write to stderr via plain `eprintln!`, so catastrophic failures remain visible in `kubectl logs`; (d) a `BRRTR_DEV_LOGS_TO_STDOUT=1` escape hatch forces the `fmt::Layer` path for break-glass local dev. |
| **Pyroscope sampler thread on `may` runtime** | Low | `pyroscope-rs` runs on a dedicated OS thread, not a `may` coroutine — no coroutine-stack-usage collision. Overhead capped at 100 Hz sampling. Measure during Phase O.12 against the `openapi` Goose bench; expect <1 % of aggregate CPU. |

## 8. Open questions

1. **Q1 — Async runtime for BSP.** BRRTRouter uses `may` coroutines, not `tokio`. `opentelemetry_sdk::trace::BatchSpanProcessor` defaults to a tokio runtime feature. Options: (a) require `tokio` as a dep just for the exporter thread, (b) use the `rt` feature with a standalone thread (no runtime), (c) build a `may`-compatible processor. Recommendation: **(b)** — spin up a dedicated OS thread for the BSP loop; simplest, matches BRRTRouter's "no tokio in core" stance. Decide in O.1 review.
2. **Q2 — Log record export via OTLP?** Logs can go through OTLP (Collector → Loki exporter) instead of Promtail → Loki. Simpler collector-side, adds another dep + processor path. **Recommendation: keep Promtail → Loki for now**; revisit after Phase O.8.
3. **Q3 — OTEL-native profiling via pprof/OTLP-profiles?** Pyroscope is deployed but not wired to pet_store. Separate PRD; mention but don't scope.
4. **Q4 — Tempo vs Jaeger?** Tempo is lighter and integrates better with Grafana log→trace via structured metadata. Jaeger is what we have deployed. **Recommendation: keep Jaeger for now**; switch is a follow-up.
5. **Q5 — Sampling policy per environment.** `always_on` in dev / `traceidratio` in prod. Where does the env-split live — deployment patch files, or code? **Recommendation: deployment patches** (already how `BRRTR_LOG_*` split works).

## 9. Cross-references

### BRRTRouter (this repo)

- [`docs/PRD_HOT_PATH_V2_STABILITY_AND_PERF.md`](./PRD_HOT_PATH_V2_STABILITY_AND_PERF.md) — the PRD this one unblocks. Explicit link in §Phase R rerun "Lessons for the benchmark harness" item 3 (client-side bottleneck diagnosis needs server-side spans) → this PRD's Phase O.11.
- `src/otel.rs` — the module that gets rewritten across O.1 / O.4 / O.5 / O.7. Phase O.0 pins its rustdoc to reference the ownership table.
- `k8s/observability/` — config that gets updated in O.8 and O.9.
- `scripts/run_goose_tests.py` — gets the span-metric comparison in O.11.
- [`examples/api_load_test.rs`](../examples/api_load_test.rs) — the `BRRTR_BENCH_SCOPE` control (commit `721da13`) is the first tool that will actually consume O.6's per-route histograms.

### Lifeguard (sibling repo at `../lifeguard/`)

- [`lifeguard/docs/OBSERVABILITY_APP_INTEGRATION.md`](../../lifeguard/docs/OBSERVABILITY_APP_INTEGRATION.md) — the contract this PRD honours. Spells out the four rules (one `TracerProvider`, one subscriber, Lifeguard declines OTel globals, `channel_layer()` is optional) and names `BRRTRouter/src/otel.rs` as the sibling integration point. Phase O.0 adds the reverse cross-link.
- `lifeguard/src/metrics.rs` — `LifeguardMetrics::init()` (the `OnceCell`-guarded `set_meter_provider` call) and `prometheus_scrape_text()` (the text-concat entry point Phase O.6 consumes).
- `lifeguard/src/logging/tracing_layer.rs` — `channel_layer()` (optional may-channel log sink — Phase O.1 exposes it behind the `lifeguard-integration` cargo feature).
- `lifeguard/src/metrics.rs::tracing_helpers` — the 7 span names Lifeguard emits (`lifeguard.execute_query`, `lifeguard.acquire_connection`, `lifeguard.begin_transaction`, etc.). These become child spans of `http.server.request` automatically via the `tracing_opentelemetry::layer` installed in Phase O.1 — no Lifeguard code change needed.
- `lifeguard/Cargo.toml` — the authoritative dep pins (`opentelemetry = "0.29.1"`, `opentelemetry_sdk = "0.29.0"`, `opentelemetry-prometheus = "0.29.1"`, `may = "0.3"`). Phase O.0 and Phase O.1 pin BRRTRouter to the same line.

## 10. Success criteria for the PRD as a whole

- **G1** — A production request produces a trace in Jaeger within 30 s.
- **G3** — That trace has at least 5 child spans (parse, route, dispatch, handler, encode).
- **G4** — A log line from the same request can be looked up in Loki by `trace_id` and linked back to Jaeger.
- **G5** — `histogram_quantile(0.95, … by (http_route))` produces a series per route.
- **G6** — A rolling restart under load loses no spans.
- **G7** — The coverage matrix (§4.5) has zero NO rows.
- **G8** — `cargo test --lib` + `cargo test --lib --no-default-features` both green with and without OTEL env vars set; no behaviour change when unset.
- A follow-on Phase R bench at `BRRTR_BENCH_SCOPE=openapi` shows throughput delta ≤ 2 % compared to `b1fc30b` — we pay for observability, but not dearly.

## 11. Execution order proposal

Sequential where correctness requires it; parallel where safe:

```
O.0 (doc-only ownership contract — Lifeguard cross-link + BRRTRouter rustdoc)
 │
 ▼
O.1 (OTLP exporter + tracing_opentelemetry bridge — pins deps to 0.29)
 │    NOTE: O.1 is safe only after O.0 ships; O.0 locks the "BRRTRouter
 │    never calls set_meter_provider" invariant before we add the SDK.
 │
 ├─► O.2 (W3C propagation)
 │
 ├─► O.4 (resource attributes)
 │
 ├─► O.5 (graceful shutdown — ShutdownGuard)
 │
 └─► O.3 (span catalog — 7 child spans including handler.execute which
      │   becomes the parent for Lifeguard's lifeguard.execute_query spans)
      │
      ├─► O.11 (perf-PRD integration: per-span p95 table from spanmetrics)
      │
      └─► O.6 (metrics — Prometheus text only, concat Lifeguard scrape)
            │
            └─► O.7 (log→trace correlation — trace_id in JSON)
                 │
                 └─► O.8 (Promtail JSON + Loki structured metadata)
                      │
                      └─► O.9 (dashboards — mount + fix + new)

O.10 (memory middleware tune) — independent of O.0–O.9, can land any time.

O.12 (Pyroscope continuous profiling) — independent of O.1–O.11; can land any time after
     O.1 if you want flamegraphs correlated with OTLP traces by the same resource attrs.
     Satisfies the user directive "force observability maturity incl. flamegraphs".
```

Estimated total effort: **1 same-day doc commit for O.0, then 4 PRs for O.1 + O.2 + O.4 + O.5 (one engineer, two weeks)** to make Jaeger work end-to-end; remaining phases layer in over the following month while Phase R perf work continues.

---

**Revision history**

| Version | Date | Change |
|---|---|---|
| 0.1 | 2026-04-18 | Initial DRAFT — 11 phases (O.1–O.11) + audit sections 4.1–4.8. Proposed OTEL deps at 0.27 line. Missed the Lifeguard composition contract. |
| 0.2 | 2026-04-18 | **Folded in Lifeguard composition findings** from readonly audit of `../lifeguard/`. Added §4.9 (Lifeguard already owns part of the contract — the `set_meter_provider` race and the sibling `OBSERVABILITY_APP_INTEGRATION.md` doc). Added Phase **O.0** (documentation-only ownership contract + dep-version pin). Bumped OTEL dep pins **0.27 → 0.29** to match Lifeguard. Rewrote **Phase O.6** to stay on hand-rolled Prometheus text + `lifeguard::metrics::prometheus_scrape_text()` concat — BRRTRouter never calls `set_meter_provider`. Added **N8 + N9** non-goals. Extended §7 risk table with version-skew + meter-provider-race + Lifeguard-span double-cost rows. Redrew §5.1 architecture diagram to show Lifeguard's emission path flowing through the BRRTRouter-owned subscriber and its separate meter-provider ownership. Extended §9 cross-references with a dedicated Lifeguard subsection. Updated §11 DAG so O.0 is the gate in front of O.1. |
| 0.3 | 2026-04-18 | **Eliminated the `stdout → Promtail → Loki` runtime log path** per user directive "stdout/disk tailing is slow; OTLP direct to Collector is the better implementation; only startup on stdout; force observability maturity incl. flamegraphs." Phase O.1 now builds a `LoggerProvider` + `BatchLogRecordProcessor` + OTLP `LogExporter` alongside the tracer provider; composes `opentelemetry_appender_tracing::OpenTelemetryTracingBridge` as the log sink in place of the stdout `fmt::Layer`. Phase O.7 collapsed to trivial (OTLP log records carry `trace_id`/`span_id` natively). Phase O.8 radically descoped — Promtail retains only startup-stdout scraping under a short-retention stream. Added **Phase O.12** (Pyroscope continuous profiling; the "flamegraphs" piece). Redrew §5.1 diagram for OTLP-native three-stream architecture. Added §4.9 notes on `channel_layer()` ambiguity under OTLP-native logs. Added risk rows for OTLP log backpressure, silenced-stdout impact on `kubectl logs`, Pyroscope sampler thread. Retired N2 non-goal (log-aggregation rewrite is now IN scope). Added `opentelemetry-appender-tracing`, `pyroscope`, `pyroscope-pprofrs` to the dep list. Corrected user's "UDP" framing (OTLP is gRPC/HTTP only; direct gRPC achieves the same goal). |
