# Off-coroutine may I/O

**What:** a `may` socket (or anything that waits on a may primitive) used from a plain OS thread rather than a
coroutine. It works while the may workers are free, but its wakeups and its timeout are driven by a worker's
selector. If that worker is blocked - typically a handler that ran work on `std::thread::scope` threads and is
waiting in `join` - the call never wakes and never times out.

**Seen:** 2026-11-03, a dependent service. A handler ran its ClickHouse reads on scoped threads and joined them;
with a may-socket client those reads hung while the handler held database pool slots. A test that blocks the
only may worker in a scope join reproduced it; reads that pick a std socket off-coroutine fixed it.

## What BRRTRouter does

| call off a coroutine | behaviour |
|---|---|
| `net::resolve` | answers from the cache if fresh; else the system resolver on a helper thread, waited for on a std channel within the caller's timeout; on failure or timeout, the last good (expired) answer if there is one. Never waits on a may primitive. |
| `net::connect` | still a may socket; counted |
| `http::fetch_get_full` / `fetch_post` / `fetch_query` | still may_minihttp; counted once the URL is a valid http(s) URL |

Counter: `brrtrouter_off_coroutine_io_total{call="net_resolve"|"net_connect"|"http_fetch"}` on `/metrics`. The
first `net_connect` / `http_fetch` of the process is logged at `ERROR` with the thread name (the first
`net_resolve` at `DEBUG`, since it is safe). Known legitimate caller: the JWKS background refresh thread
(`http_fetch`).

No `debug_assert`: legitimate OS-thread callers exist, and an assertion would abort debug builds and tests.

## Rules for handler code

- Do not `join` a thread (or `std::thread::scope`) from a handler: it parks the may worker and everything
  queued on it. Run the work on its own thread and receive the result on a `may::sync::mpsc` channel, or run
  it on coroutines.
- On a plain OS thread use blocking clients (std sockets); on a coroutine use may-native ones.
- `clippy.toml` disallows `std::thread::scope` and `JoinHandle::join`; genuine OS-thread code carries an
  `#[allow]` with the reason.

Related: [`native-http-client-and-tls.md`](./native-http-client-and-tls.md), [`handler-stack-memory.md`](./handler-stack-memory.md).
