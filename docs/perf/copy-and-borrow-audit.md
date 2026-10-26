# Copy and borrow audit (running log)

Started 25 Oct 2026. Goal: Rust code that does not copy or allocate where a borrow, a move or a
build-once would do — measured first, changed only where it costs CPU or allocations on a hot
path. Order: BRRTRouter (generated code matters most: every service inherits it), then each
service codebase (Sesame-IDAM, PriceWhisperer BFFs → services → ingestors/workers, RERP,
Lifeguard, microscaler-observability). This file is the log that survives across sessions:
findings ranked by measured cost, what changed, before/after, and what is still open.

## How it is measured

- **`benches/request_path.rs`** (`cargo bench --bench request_path`): the pet_store example
  served in-process with the middleware a generated service installs (metrics, CORS), an
  API-key provider and a JWKS bearer provider (claims-cache hit, the steady state), driven
  over keep-alive connections with prebuilt browser-like requests (10 headers + auth). A
  counting global allocator gives **allocations and bytes per request**; wall time gives
  **µs per request** (one connection, so latency, not throughput). INFO-level events are
  formatted as JSON into a sink (production logs at INFO); `BRRTR_BENCH_LOG=off` measures
  without. Diagnostics: `BRRTR_BENCH_SITES=<n>` samples every n-th allocation and prints the
  top call sites; `BRRTR_BENCH_TRACE=<bytes>[,<count>]` prints backtraces of large ones.
  Runs on ms02 (24 cores), same binary layout before/after.
- pp: Cloud Profiler CPU/heap for release images carries **no stacks** (only the thread
  name: `clone`/`Unknown`), so it cannot rank functions; use `-debug` images for that. The
  PW goose suite (`microservices/loadtest`, ClickHouse `loadtest`) is the end-to-end check
  per release.

## BRRTRouter — pass 1 (25 Oct 2026)

### Findings, ranked by measured cost (per request, request_path bench)

| # | Finding | Cost before | Change |
|---|---------|-------------|--------|
| 1 | `JwksBearerProvider`: every authorized request decoded the JWT header 3× (validate, the DEBUG span's token-version lookup, `extract_claims`), built `format!("{token}\|{kid}")` cache keys 3×, deep-cloned the cached claims `Value` 2×, cloned the `DecodingKey` and opened a span for the key-rotation check, and built `JwtLogFields` plus 30 lowercase copies (PII check) and a `serde_json::Map` for the "allowed" log line | JWT route minus API-key route: **+176 allocs/req** before, **+12** after (log on); +168 → +7 (log off) | Claims cache keyed by the token (`&str` lookup, no key alloc; the token determines its header, and only fully verified tokens are cached), claims held as `Arc<Value>` (one deep copy left: into `HandlerRequest`, which the API requires), `has_key` for the rotation check, the span lookup only when the DEBUG span is on, the "allowed" log admitted (enabled + rate limit) before its fields are built and serialized from borrowed fields |
| 2 | `ValidatorCache::get_or_compile` on every request **and** every response: `serde_json::to_vec(schema)` + SHA-256 + 8 `format!`s to find the cache key, plus 2 more key `format!`s and a `SpecVersion` clone | proportional to schema size; 7 % of allocs on the bench's small schemas | `get_or_compile_for_route`: memo keyed by (schema address, status), valid while the entry's `Weak<RouteMeta>` is the caller's route (a `Weak` pins the allocation, so the address cannot be reused). Hit = one map lookup on two integers |
| 3 | Dispatcher deep-cloned the whole `HandlerRequest` (body `Value`, claims `Value`, headers, cookies, params) twice per request: once to set `queue_guard`, once to send it to the handler | ~60 allocs + body/claims size | The request is moved to the handler; the dispatcher keeps `HandlerRequest::without_payload()` (no body/claims) for the `after` hooks and logs. The queue-depth guard `Arc` is shared by both, as before |
| 4 | `AppService::call` cloned what it then dropped: query params into the route match, `route_match` into the dispatcher, request headers, `raw_query`, response headers, the request-header log string, plus sanitized cookie/query copies built only for a DEBUG event | ~40 allocs | Moves instead of clones; DEBUG-only values computed inside the `debug!` (tracing evaluates fields only for enabled callsites); the request id passed as `RequestId` (`dispatch_with_id`) instead of formatted and re-parsed |
| 5 | Access log: `Sanitizer::headers_for_log` (twice per request) cloned every header into a `HeaderVec`, then into a `serde_json::Map` (key + value copies), then serialized; `should_redact` lowercased every header name into a new `String` | ~4 allocs per header | Written straight into one buffer with serde_json's escaping; identical output (sorted keys, last duplicate wins — tested against the old map); ASCII case-insensitive matching without copies |
| 6 | Typed handlers: `jwt_claims` deep-cloned before `TryFrom`; the worker-pool path and `TypedHandlerFor::from_handler` called `try_from(req.clone())` (a full request copy) | claims size; whole request on pool routes | Claims moved (`take`) to the typed request; `try_from(req)` by value everywhere |
| 7 | Generated `TryFrom<HandlerRequest>` built each parameter's schema with `serde_json::json!(…)` on every request (a `Value` tree per path/query/header parameter) | ~6 allocs per parameter | Template: `static SCHEMA: LazyLock<Value>` per parameter. Every regenerated service inherits it |
| 8 | `parse_request` formatted the HTTP version and collected header/cookie name vectors for DEBUG events on every request | 3 allocs | Computed inside the `debug!` |
| 9 | `MemoryMiddleware` as a dispatcher middleware read `/proc/self/smaps` three times per request (one result discarded) | 3 MB allocated and ~1.4 ms per request with ~100 coroutine stacks mapped | Generated services do **not** install it as middleware (only for `/metrics` + the 10 s monitor), so no production impact; the discarded read is removed and the per-call handler-name copy avoided. Don't add it to the chain |

### Open (measured, not changed in pass 1)

- ~~**Reply channel block: 46 KB allocated per request.**~~ Done in pass 1b (below).
- **Claims copy into every request** (~11 % of allocs on JWT routes): `HandlerRequest.jwt_claims:
  Option<Value>` and `SecurityProvider::extract_claims -> Option<Value>`; `Arc<Value>` would make
  it free. Public API change; same PR as the reply channel if consumers agree.
- **Tracing span records** (~45 % of allocs with the JSON subscriber): ~8 `span.record` calls
  per request (status twice, route twice, `otel.name` via `format!`, auth fields,
  `stack_used_kb` — which records the configured size, not usage). Logging/telemetry policy is
  Charles's call; numbers with `BRRTR_BENCH_LOG=off` below show the ceiling.
- **CORS `after`** (~4 % of allocs on cross-origin requests): joins the allowed
  methods/headers/expose lists into new `String`s per request from immutable config;
  `set_header` allocates `Arc<str>` names. Precompute the joined values per policy.
- **Response header writing**: `format!("{k}: {v}")` per header (may_minihttp takes a whole
  line). Needs a name/value API in the may_minihttp fork.
- **BFF proxy (PW trader-bff/platform-bff)**: `proxy_untyped` parses every downstream JSON body
  into a `Value`, the BFF then validates it against its own spec (105 of 113 trader-bff routes
  declare a 200 schema) and re-serializes it — parse + validate + serialize of a body the
  downstream service already validated. It also opens a new TCP connection per proxied request.
  DNS per request is fixed (pass 1c below). Raw pass-through (and a pooled client) is the
  biggest BFF item; it changes what a BFF validates, so it is a decision for Charles — to be
  measured in the PW pass.
- `MetricsMiddleware::record_status` allocates a `(String, u16)` key per request for the
  lookup (small).

### Before / after (request_path bench, ms02)

Allocations per request are deterministic (the same on every run); µs/req is one connection on a shared
24-core box and moves ±20 % between runs, so read it as direction, not a figure.

INFO subscriber on (production logs at INFO). µs: median of three interleaved base/after runs.

| scenario | allocs before | allocs after | Δ | bytes before | bytes after | µs before | µs after | Δ |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| get_pet (1 path param) | 434.2 | 273.2 | −37 % | 123,590 | 114,999 | 44.3 | 38.2 | −14 % |
| list_pets_query (2 query params) | 478.2 | 299.2 | −37 % | 125,166 | 115,835 | 48.1 | 38.4 | −20 % |
| get_post_2_params | 440.2 | 276.2 | −37 % | 124,029 | 115,782 | 42.0 | 39.0 | −7 % |
| add_pet_json_body (request + response validation) | 465.2 | 263.2 | −43 % | 124,584 | 114,766 | 44.0 | 38.6 | −12 % |
| secure_jwt (JWKS bearer, cache hit) | 602.9 | 284.2 | −53 % | 139,716 | 117,307 | 52.0 | 40.6 | −22 % |

`BRRTR_BENCH_LOG=off` (no subscriber: BRRTRouter's own work only):

| scenario | allocs before | allocs after | Δ | µs before | µs after |
|---|---:|---:|---:|---:|---:|
| get_pet | 299.2 | 138.2 | −54 % | 42.7 | 29.5 |
| list_pets_query | 345.2 | 166.2 | −52 % | 38.4 | 37.7 |
| get_post_2_params | 304.2 | 140.2 | −54 % | 41.9 | 38.3 |
| add_pet_json_body | 331.2 | 129.2 | −61 % | 45.4 | 38.4 |
| secure_jwt | 466.7 | 145.2 | −69 % | 47.2 | 34.1 |

Bytes per request barely move: 46 KB of the remaining ~101–115 KB is the reply channel's block
(open item 1); the rest is still to be attributed (`BRRTR_BENCH_TRACE=8192`).

## BRRTRouter — pass 1c: BFF proxy DNS (25 Oct 2026)

Evidence (pp, v0.1.132 release-profile load test, 18:54–18:59 UTC): trader-bff used 0.0988
cores against a 0.0759 idle floor for 1,142 proxied requests: **~6 ms of CPU per proxied
request** for a hop that only forwards. Per request it ran a blocking `getaddrinfo` on the
coroutine's worker thread for `<svc>.<ns>.svc.cluster.local` — four dots, under the pod
resolver's `ndots:5`, so each lookup first walked the search domains (an NXDOMAIN round trip
per domain, A and AAAA) — then connected.

Change: the resolved address is cached per host (`BRRTR_PROXY_DNS_TTL_SECS`, default 30,
`0` = off) and Service FQDNs are looked up with a trailing dot (one query). A connect failure
on a cached address drops it, re-resolves and retries once (nothing has been sent yet). To
verify in pp: trader-bff CPU per proxied request and the BFF-only latency of fast routes
(`list_plans`, `get_payment_method`, ~6 ms p50 at v0.1.132).

## BRRTRouter — pass 1b: boxed reply channel (25 Oct 2026)

`may::sync::mpsc::channel::<HandlerResponse>()` allocates a 64-slot block (`may_queue::mpsc`,
`BLOCK_SIZE = 64`) of ~720-byte `HandlerResponse` slots, and the dispatcher opens one channel per
request: **46,144 bytes allocated (and freed) per request**, ~40 % of all bytes the request path
allocated. `HandlerRequest::reply_tx` is now `mpsc::Sender<Box<HandlerResponse>>` (a 1 KB block),
as `HandlerSender` already does for requests; `HandlerRequest::reply(response)` sends one.

| scenario (INFO log on) | bytes/req pass 1 | bytes/req 1b | allocs/req |
|---|---:|---:|---:|
| get_pet | 114,999 | 25,599 | 273 → 274 |
| list_pets_query | 115,835 | 26,435 | 299 → 300 |
| get_post_2_params | 115,782 | 26,382 | 276 → 277 |
| add_pet_json_body | 114,766 | 25,366 | 263 → 264 |
| secure_jwt | 117,307 | 27,882 | 284 → 285 |

Bytes per request −78 % (one extra small allocation: the box). Breaking for code that names the
channel type (`mpsc::channel::<HandlerResponse>()`, `Sender<HandlerResponse>`) or sends on
`reply_tx` directly; `reply_tx: may::sync::mpsc::channel().0` literals still compile.

## BRRTRouter — pass 2: BFF proxied JSON passes through (26 Oct 2026)

Evidence: the performance agent found every BFF parses, validates and re-serialises each proxied
response (scratch measurements on ms02, per body: 20 KB 94 / 24 / 24 µs, 200 KB 953 / 233 / 257 µs
for parse / validate / serialise). `proxy_untyped` read the body, parsed it into a `Value`
(`response_body_value`), the service validated that `Value` against the operation's response schema
and `write_handler_response` serialised it again. The downstream is a BRRTRouter service that already
validated the same body against the same schema (the BFF spec is merged from the service specs).

Change: a 2xx body with a JSON Content-Type is kept as text (`Value::String` plus the internal
`x-brrtrouter-raw-encoding: json` marker) and written as received. ~~Response validation skips it unless
the operation sets `x-brrtrouter-validate-response: true`.~~ **Superseded by ADR 006:** pass-through is
`BRRTR_PROXY_PASSTHROUGH` (default off) and response validation is a mode (`full` default); see pass 4.
Errors (non-2xx) still parse, because their reason is read for logs and metrics. The read buffer is sized
from Content-Length (no doubling reallocations).

`benches/request_path.rs` gained `bff_proxy_20k` / `bff_proxy_200k`: a BFF-shaped service (metrics +
CORS, untyped proxy handlers) in front of an in-process downstream answering a 200- / 2,000-row JSON
array (one TCP connection per proxied call, as in production). INFO log on, ms02, n = 3,000 × 5:

| scenario | allocs/req | bytes/req | µs/req |
|---|---:|---:|---:|
| bff_proxy_20k before | 2,476 | 351,143 | 290 |
| bff_proxy_20k after | 252 | 62,449 | 95 |
| bff_proxy_200k before | 22,285 | 2,857,571 | 1,830 |
| bff_proxy_200k after | 252 | 263,750 | 189 |

Allocations per proxied call no longer depend on the body size (−90 % at 20 KB, −99 % at 200 KB);
latency −67 % / −90 %. Of the remaining 264 KB at 200 KB, the body is held once (the read buffer
becomes the response body) plus may_minihttp's write buffer. Still open: a
new TCP connection per proxied call (no keep-alive pool to the downstream).

## BRRTRouter — pass 4: validation modes and pass-through as configuration (26 Oct 2026)

ADR 006 (`docs/ADRS/006-…`) restores today's guarantee as the default. #508 skipped response
validation for proxied bodies by default; now `BRRTR_RESPONSE_VALIDATION=full|sample:<rate>|off`
(default `full`) decides, with `brrtrouter_response_validation_total{handler,result}` and
`…_skipped_total{handler}` counters, and pass-through is `BRRTR_PROXY_PASSTHROUGH` (default off).

`bff_proxy_*` (ms02, INFO log, n = 3,000 × 5), allocs / bytes / µs per request:

| configuration | 20 KB | 200 KB |
|---|---|---|
| defaults (pass-through off, `full`) | 2,466 / 308 KB / 279 | 22,272 / 2.56 MB / 1,656 |
| pass-through on, `full` | 2,460 / 243 KB / 246 | 22,263 / 2.03 MB / 1,799 |
| pass-through on, `sample:0.1` | 474 / 80 KB / 102 | 2,456 / 441 KB / 304 |
| pass-through on, `off` | 253 / 62 KB / 93 | 254 / 264 KB / 154 |

The cost is parse + validate, not serialise: with `full`, pass-through saves ~20 % of bytes and
nothing measurable in time. The 5–10× saving needs `sample` or `off`, which the ADR keeps for an
environment Charles approves, with build-time contract checks in place first.

## For the PriceWhisperer pass

- benzinga-analyst-ingestor startup pass clones the whole JSON response 3–4× per 50-ticker
  batch (`microservices/ingestors/benzinga… main.rs` ~360–369) — its 94 Mi peak (from the
  memory agent). The `pw_alloc` crate (jemalloc + `/debug/heap` on every worker's metrics
  port) can measure allocation changes in workers.
