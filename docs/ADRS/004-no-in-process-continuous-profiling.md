# ADR 004: No in-process continuous profiling; profile with Pyroscope and Grafana in development

**Status:** Accepted  **Date:** 2026-10-14
**Context:** [`docs/PRD_OBSERVABILITY_AND_TRACING.md`](../PRD_OBSERVABILITY_AND_TRACING.md) (Phase O.12, Pyroscope),
[`docs/flamegraph.md`](../flamegraph.md), [`k8s/observability/pyroscope.yaml`](../../k8s/observability/pyroscope.yaml)

## Question

BRRTRouter services already send traces and logs over OTLP, and those reach Google Cloud Observability (Cloud Trace,
Cloud Logging) through an OpenTelemetry Collector. Could CPU and heap profiles of production services reach the same
place, either as OpenTelemetry profiles or into Google Cloud Profiler?

## What exists (October 2026)

| Route | State | Fit for BRRTRouter |
|---|---|---|
| **OpenTelemetry Profiles** (OTLP profiles signal) | Public alpha since March 2026. The Collector has an eBPF profiler receiver and a pprof receiver, with lossless pprof ↔ OTLP translation. The project's own announcement says it is not for critical production workloads and that no production backend exists yet. | Google Cloud does not accept OTLP profiles, and the Collector's `googlecloud` exporter has no profiles pipeline. Nothing on the Google side receives them. |
| **eBPF profiler** (whole-node, no code change) | Runs as a privileged DaemonSet. The alpha release notes cover symbolization for Go, Node.js, .NET, Ruby and BEAM; Rust is not mentioned. | Not allowed on GKE Autopilot, which forbids privileged workloads. Only possible on a GKE Standard node pool, with Rust symbolization unproven. |
| **Google Cloud Profiler** | Agents exist for Go, Java, Node.js and Python only. The API (`cloudprofiler.googleapis.com/v2`) is language-neutral: an agent long-polls `CreateProfile`, collects what the server asks for (CPU, HEAP, WALL, …) and uploads a gzipped pprof profile; `createOffline` uploads a profile directly ("direct use … is discouraged"). There is a Google Rust client (`google-cloud-profiler-v2`). 30 days of history. | Possible only by writing our own Rust agent: a sampling profiler in-process (the `pprof` crate, SIGPROF at ~100 Hz, in-process symbolization) and heap profiles from jemalloc (`jemalloc_pprof`), uploaded by a background thread with the pod's Workload Identity. |
| **Pyroscope + Grafana** | Already in the development stack (`k8s/observability/pyroscope.yaml`, the Grafana Pyroscope datasource and the performance dashboard). | Works today, outside the production path. |

## Decision

1. **No profiler runs inside BRRTRouter in production.** Not an in-process Cloud Profiler agent, not a Pyroscope push
   agent, not the `pprof` crate behind a feature flag in shipped images.
2. **Performance investigation uses Pyroscope and Grafana on the local development cluster**, together with
   `cargo flamegraph` / `perf` for targeted runs ([`docs/flamegraph.md`](../flamegraph.md)) and the Goose load tests
   ([`docs/LOAD_TESTING_GUIDE.md`](../LOAD_TESTING_GUIDE.md)). A regression seen in production metrics or traces is
   reproduced there under load and profiled there.
3. **Production keeps the signals that cost nothing on the hot path:** the request metrics
   ([`llmwiki/topics/http-server-metrics.md`](../../llmwiki/topics/http-server-metrics.md)), traces and logs over OTLP.

## Why

- **Throughput is the product.** BRRTRouter's value is its request rate and tail latency. Every option for production
  profiling puts work on the serving processes: a SIGPROF handler interrupting every worker thread about 100 times a
  second and unwinding its stack, jemalloc allocation sampling on every allocating path, and an extra thread holding
  connections to Google. The cost is small on average and not zero, and it lands on exactly the paths we benchmark.
  We will not trade measured speed for always-on profiles.
- **The may runtime makes in-process samples doubtful.** Handlers run on may coroutines with their own stacks. A
  signal-based sampler unwinding from inside a coroutine may stop at the coroutine boundary, so the profiles most worth
  having could be truncated. Proving otherwise is a spike we choose not to spend on now.
- **The non-invasive routes are not ready.** OpenTelemetry profiles are alpha with no Google backend, and the eBPF
  profiler cannot run on GKE Autopilot.
- **Development already answers the question.** Pyroscope and Grafana give flame graphs over time and comparisons
  between builds on the same code, under load we control, with no cost to production.

## Consequences

- Production has no continuous profiles. A production-only performance problem is investigated through metrics and
  traces first, then reproduced under load in development and profiled there.
- The `jemalloc` feature keeps its current role (accurate heap statistics for the memory metrics); heap profiling
  (`prof:true`) stays a development-only setting.
- No Cloud Profiler IAM, agent code or configuration is added to BRRTRouter or the services built on it.

## Revisit when

- Google Cloud Observability accepts OTLP profiles, or the Collector's Google exporter gains a profiles pipeline, **and**
  a profiler that runs outside the serving process (eBPF) symbolizes Rust and is allowed where the services run.
  A profiler that runs outside the process would leave the hot path untouched.
- Or a production-only performance problem cannot be reproduced in development.
