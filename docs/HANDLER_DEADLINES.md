# Handler / request deadlines (Epic 13.6)

When enabled, the dispatcher waits for a handler reply with a timeout. If the
handler does not reply in time, the client receives **504**
`application/problem+json` with `reason: handler_deadline_exceeded` and the
Prometheus counter `brrtrouter_handler_deadline_timeouts_total` increments.

**Default: off** (legacy unbounded wait). Existing apps are unchanged until you
opt in.

## Configuration

| Source | Key | Semantics |
|--------|-----|-----------|
| Env | `BRRTR_HANDLER_DEADLINE_MS` | Global wait limit in milliseconds. `0` / unset / invalid → disabled. |
| `config.yaml` | `http.handler_deadline_ms` | Same. When set (including `0`), wins over the env var at startup. |
| OpenAPI | `x-brrtrouter-deadline-ms` | Per-operation override (see ceiling policy). |

Example:

```yaml
http:
  handler_deadline_ms: 5000
```

```yaml
# OpenAPI operation
x-brrtrouter-deadline-ms: 1000
```

## Global ceiling

- Global set + route set → effective = `min(global, route)` (route may only shorten).
- Global unset + route set → route deadline applies for that route only.
- Either value `0` → treated as disabled for that layer.

## Non-goals / cleanup

- We **do not** cancel arbitrary handler CPU mid-flight; we stop waiting on the
  reply channel. A late send after timeout is ignored when the reply receiver is
  dropped (oneshot closed).
- Distinct from **proxy / upstream** timeouts (HTTP client to a downstream): those
  may also yield 504 via existing proxy error paths. Handler deadline is the
  wait between dispatcher and the local handler coroutine.

## Metrics

`brrtrouter_handler_deadline_timeouts_total` — counter of deadline misses.

## See also

- [`PROBLEM_DETAILS.md`](PROBLEM_DETAILS.md) — `…/gateway-timeout`
- Story: [`story-13.6-handler-request-deadlines.md`](EPICS/FRAMEWORK_MATURITY/epic-13-framework-completeness/story-13.6-handler-request-deadlines.md)
