# Relayed event-stream metrics

**Status:** verified (code + tests, 2026-11-10)

**Code anchors:** `src/http/stream_observer.rs`; `src/http/sse_metrics.rs` (`opened`, `StreamGuard`, `CloseReason`, `principal_key`,
`render_prometheus`); `src/http/proxy.rs` (`proxy_sse_inner`, `supervise_relay`, `supervise`, `relay_frames`);
`src/sse.rs` (`SseSender::send_frame_checked`, `SseSender::is_closed`, `SendError`); `src/server/service.rs` (`/metrics`)

## What and why

At a BFF every browser stream on a proxied `text/event-stream` route is one relayed stream (`proxy_sse`). Until now
`/metrics` said nothing about them: how many are open, how long they live, why they end, how many people hold one.
These series are the edge's view of its live clients.

| Series | Type | Labels |
|---|---|---|
| `brrtrouter_sse_streams` | gauge | `handler` |
| `brrtrouter_sse_opened_total` | counter | `handler` |
| `brrtrouter_sse_closed_total` | counter | `handler`, `reason` |
| `brrtrouter_sse_duration_seconds` | histogram | `handler` |
| `brrtrouter_sse_principals` | gauge | `handler` |

- **`handler`** is the route's handler name as the router resolved it from the spec: its `operationId`, or an
  `x-handler` extension where one is set. It is the same label as `brrtrouter_worker_pool_*{handler}`. Bounded by the
  spec: at most 128 values (the rest fold into `_other`), and an empty one is `_unmatched`. A request path is never a
  label.
- **`reason`**: `client_closed`, `slow_client` (the bounded queue filled), `downstream_ended`, `idle_timeout`
  (`BRRTR_PROXY_SSE_IDLE_SECS` without a byte), `error` (a read error, an oversized frame, a relay that panicked or
  never reported).
- **Principals** come only from `HandlerRequest::jwt_claims`, which the router fills after the route's security check
  passed. Nothing in the proxy decodes a token. A stream without a verified `sub` counts as a stream, not as a
  principal. A principal is a 64-bit hash of `sub` with a count of its open streams, so the gauge is a set size; no
  subject is exported.

## Recording model

Recorded at open and close only: relaying a frame costs nothing extra. `opened()` returns a `StreamGuard`; the relay's
coroutine calls `close(reason)` when it ends, and a guard dropped without a close (the relay never started, or the
coroutine unwound) records `error`. The gauges therefore return to 0 on every path (tested under churn).

## A client that leaves a silent stream

The relay spends most of its life blocked in a downstream read. When the client disconnects while the downstream is
quiet, the server drops the `SseReceiver` after its socket write fails, but the relay would only learn of it when the
next frame failed to queue: up to the idle timeout later, reported as `idle_timeout`.

`supervise_relay` runs the reading loop on its own coroutine (`brrtr-proxy-sse-read`) and checks
`SseSender::is_closed()` (set by the receiver's `Drop`) once a second. Once the client has gone it cancels the reader
(may unwinds it at its blocking read, closing the downstream connection) and records `client_closed`. Cost: one timer
wake-up a second per open stream.

The receiver itself is dropped promptly too. Between frames the server peeks at the client's socket once a second
(`stream_sse`, `sse_client_probe`: a non-blocking peek on a clone of the connection). A reset ends the stream. End
of input alone does not: TCP lets a client close its sending side and keep reading, so the server then writes an SSE
comment (`:`), which clients ignore and which fails once the peer has really gone. A client that leaves a stream on
which nothing is being sent is noticed within about two seconds; a half-closed reader keeps its stream. Before this change it was noticed only when the next write failed.

> **Open:** both checks are polls (one timer wake-up a second per open stream on each side). An event-driven wake-up
> would cost nothing while the stream is open; it needs a waitable close signal in `sse.rs` and readiness on the
> client socket.

## Stream observer (`stream_observer`)

An application that needs to know who holds a stream registers one `StreamObserver` with `set_stream_observer`; a
BFF counting live clients per user in a shared store is the case it was built for. BRRTRouter itself knows nothing
about users, tabs or devices.

- `opened(&StreamOpen)` is called once the downstream has answered with an event stream. It receives the handler
  name, the verified claims (`subject()` is `sub`), and read-only request headers and query parameters. It may return
  a `StreamLease`.
- `alive()` runs every `BRRTR_SSE_OBSERVER_ALIVE_SECS` (default 30, at least 1), from the relay's supervisor.
- `closed(reason)` runs exactly once with the same reason as `brrtrouter_sse_closed_total`. A lease dropped unclosed
  (the relay panicked or never started) closes as `error`.
- **Cost.** With no observer: one atomic load per stream. With one: these calls, nothing per frame.
- **Failures.** Panics in `opened` and `alive` are caught and the stream goes on. `closed` may run during unwinding
  and must not panic.
- **Tests.** `stream_observer.rs` covers the lease contract. `proxy.rs`
  `the_lease_closes_once_on_every_exit_path` covers each reason. `tests/sse_stream_observer_tests.rs` covers the
  real-socket cases through may_minihttp: a quiet in-process stream, and a relayed stream left mid-stream.

## Related

- [`topics/http-server-metrics.md`](./http-server-metrics.md): the rest of `/metrics`.
- [`topics/bff-gateway-merge-and-proxy-headers.md`](./bff-gateway-merge-and-proxy-headers.md): the proxy itself.
