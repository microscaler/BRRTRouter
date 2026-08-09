# SSE live flush (Epic 13.7)

`x-sse: true` routes can stream `text/event-stream` frames as they are produced
instead of buffering with [`SseReceiver::collect`].

## Handler API

Prefer [`HttpSse`](../src/typed/core.rs) (or [`HandlerResponse::sse_live`]):

```rust
use brrtrouter::sse;
use brrtrouter::typed::HttpSse;

let (tx, rx) = sse::channel_bounded(sse::queue_bound_from_env());
may::go!(move || {
    tx.send("tick 0");
    tx.send("tick 1");
});
HttpSse::new(rx)
```

Legacy `rx.collect()` → `Response(String)` still works but buffers until the
producer finishes (not live flush).

## Wire path

BRRTRouter uses `may_minihttp::Response::begin_chunked_stream` (Microscaler
fork) so each event is a flushed HTTP chunk. Content-Type remains
`text/event-stream`.

## Backpressure

- Soft bound via `BRRTR_SSE_QUEUE_BOUND` (default 256) — producers should use
  `SseSender::try_send` and stop when the client is gone.
- Client disconnect mid-stream: write errors end the loop; no panic (N1).

## Keepalive

Optional comment frames: `sse::format_comment("ping")` (`: ping\n\n`).

## Non-goals

WebSocket / bidirectional RPC remain parked.
