# ADR 005: Cloud Profiler in pre-production (amends ADR 004)

**Status:** Accepted  **Date:** 2026-10-23
**Amends:** [ADR 004](004-no-in-process-continuous-profiling.md), decision 1, for pre-production only.

## Context

ADR 004 kept every profiler out of the serving processes and left profiling to Pyroscope on the development cluster.
Two things changed:

- **Cost is now the question, not throughput.** Before launch the bill is set by the size of the pp footprint: every
  service runs one pod with almost no traffic, and idle services were measured at 40–100 millicores each (pp,
  23 Oct 2026) against 50-millicore requests. Finding where idle CPU and memory go needs profiles of the real images,
  with their real configuration, in the place they run, not a development reproduction under synthetic load.
- **A memory leak reached pp unseen.** Sesame's identity-login-service grew to ~720 MB because spans were entered
  across may yields (PriceWhisperer `docs/incidents/2026-10-23_otel_context_leak_PLAN.md`). Development load tests
  had not shown it: its trigger was pp's own traffic shape.

## Decision

1. **microscaler-observability ships a Google Cloud Profiler agent** behind the cargo feature `cloud-profiler`
   (Linux only), which BRRTRouter enables. It is a CPU profiler: `pprof` samples every thread at 99 Hz with SIGPROF
   for the duration Cloud Profiler asks for (10 s, about once a minute per deployment, one instance at a time),
   symbolizes in-process, and uploads the gzipped pprof profile from a background thread with the pod's Workload
   Identity token. Flame graphs appear in the Cloud console (Profiler), per service and version, kept 30 days.
2. **The agent runs only when the process has `MICROSCALER_CLOUD_PROFILER=1`.** pp sets it for every BRRTRouter
   service (gcp-infrastructure `clusters/hub/apps/pp`); **prod does not**, so in prod the code is present and inert:
   no thread, no signal handler, no network. ADR 004 stands for prod.
3. **IAM through Crossplane:** `cloudprofiler.googleapis.com` enabled in each runtime project and
   `roles/cloudprofiler.agent` granted to the application namespaces (`pw`, `sesame-idam`, `rerp`) as Workload
   Identity principal sets; no service-account keys and no KSA annotations.
4. **Heap profiles are not part of this.** They need jemalloc profiling (`prof:true`) in the image; revisit once CPU
   profiles have shown whether the agent is worth extending.

## Consequences

- pp pays a small, bounded cost: about one in six seconds of SIGPROF sampling for one instance of each deployment,
  and one HTTPS long-poll per process. Prod pays nothing beyond the compiled code.
- Coroutine stacks: a sample taken inside a may coroutine unwinds the coroutine's own stack and may stop at its base,
  so request-handler stacks can appear without their scheduler frames. That is enough to attribute CPU to handlers,
  middleware, serialization and the database layer; scheduler-level questions still go to `perf` in development.
- Images must keep their symbol table (no `strip`). The services' workspaces set no `strip`; the first profiles confirm whether the build keeps names.

## Revisit when

- Prod needs profiles: decide then, with the pp overhead measured, whether prod gets the variable on one instance.
- Google Cloud Observability accepts OTLP profiles (ADR 004's condition): the agent can then move out of the process.
