# Story 13.7 — SSE live flush streaming

**GitHub issue:** [#407](https://github.com/microscaler/BRRTRouter/issues/407)  
**Epic:** [Epic 13](README.md)  
**Wave:** 4  
**Effort:** M  
**Testing:** [TESTING_STANDARD.md](../TESTING_STANDARD.md)

## Overview

Upgrade `x-sse` from **buffered `receiver.collect()`** to **live flush** of
`text/event-stream` frames so clients observe events as they are produced.

## Delivery

- Response path writes SSE frames incrementally (flush per event or batch policy).
- Keep `text/event-stream` content-type and comment/ping keepalive option.
- Document backpressure: slow client → drop/disconnect policy (no unbounded RAM).
- README/feature table: remove “buffered only” caveat when done.
- Non-goal: WebSocket; bidirectional RPC.

## Functional requirements

| ID | Requirement |
|----|-------------|
| FR-1 | Client receives first event before stream completes (observable flush). |
| FR-2 | Multiple events arrive in order. |
| FR-3 | `Content-Type: text/event-stream`. |
| FR-4 | Stream end closes cleanly (no hang). |
| FR-5 | Non-SSE routes unchanged. |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| NFR-1 | Bounded queue / max buffered events when client is slow. |
| NFR-2 | No panic if client disconnects mid-stream. |
| NFR-3 | Keepalive does not starve event writes. |
| NFR-4 | Compatible with may_minihttp response constraints (document any fork needs). |

## Unit tests (required)

### Positive

| ID | Scenario | Assert |
|----|----------|--------|
| P1 | Two events flushed | client sees both in order |
| P2 | Content-Type event-stream | header |
| P3 | Early event visible before producer finishes | timing/order assert |
| P4 | Clean producer end | client EOF |
| P5 | Existing x-sse fixture route | regression smoke |
| P6 | Keepalive comment optional | documented/tested if shipped |

### Negative

| ID | Scenario | Assert |
|----|----------|--------|
| N1 | Client disconnect mid-stream | no panic; producer stops or errors cleanly |
| N2 | Unbounded buffer on slow client | forbidden (cap or disconnect) |
| N3 | Panic in frame encode | forbidden |
| N4 | Non-SSE JSON route returns event-stream | forbidden |
| N5 | Silent buffer-all until end (regression to old behavior) | forbidden when flush mode on |
| N6 | Credential in SSE comment/debug | forbidden |

### Acceptance criteria (tests)

- [ ] P1/P3 and N1/N5 mandatory.

## Acceptance criteria

- [ ] Live flush is default for `x-sse` or opt-in with docs.
- [ ] FR/NFR + unit tests complete.

## References

- `src/sse.rs`, `tests/sse_tests.rs`, README SSE caveat
