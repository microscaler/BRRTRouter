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
- ~~**Response header writing**~~: `format!("{k}: {v}")` per header. Done in pass 7:
  `Response::header_kv` in the may_minihttp fork, used by `write_handler_response`.
- **BFF proxy (PW trader-bff/platform-bff)**: `proxy_untyped` parses every downstream JSON body
  into a `Value`, the BFF then validates it against its own spec (105 of 113 trader-bff routes
  declare a 200 schema) and re-serializes it — parse + validate + serialize of a body the
  downstream service already validated. It also opened a new TCP connection per proxied request
  (pooled since pass 7).
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
becomes the response body) plus may_minihttp's write buffer. The new TCP connection per proxied
call is gone in pass 7 (keep-alive pool to the downstream).

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

## BRRTRouter — pass 3: idle CPU (26 Oct 2026)

Evidence: every BRRTRouter service in pp idles at ~74–80 mcores, including services that get no traffic
(feedback, areas, backtests), which is ~1.5 cores across pp. The release images have no shell and their
profiles carry thread names only, so this was reproduced off-cluster with the new `benches/idle_cpu.rs`. It starts
the pet_store service as a generated main does, sends nothing, and reads `/proc/self/stat`.

Cause: `may` (default feature `io_timeout`) gives each worker's `epoll_wait` a 10 ms timeout. An idle worker then
wakes, collects its global queue, tries to steal from three others and waits again. `BRRTR_MAY_WORKERS` defaults
to `max(32, cpus + DB_POOL_MAX + 16)`, which is 3,200+ wake-ups a second per pod. Idle CPU scales with
workers × 1/timeout (ms02, 20 s idle):

| workers | 10 ms (may default) | 50 ms | 100 ms | 1 s |
|---:|---:|---:|---:|---:|
| 8 | 7.0 | 2.0 | 1.0 | 0.0 |
| 32 | 18.5 | 5.0 | 3.0 | 1.0 |
| 50 | 19.0 | 5.0 | — | — |

(mcores; ms02 is bare metal, while GKE nodes pay more per wake-up, which explains the ~75 mcores seen in pp.)

Change: `BRRTR_MAY_POLL_TIMEOUT_MS`, default 50 ms, applied by `RuntimeConfig::from_env` before the scheduler
starts. Load latency is the same (`idle_cpu` load mode, 50 workers, `GET /pets/12345` answering 200;
three runs each for saturated, two for paced):

| poll | saturated, 64 conns: rps, p50 / p99 µs | paced, 16 conns × 0.5 ms gap: rps, p50 / p99 / p999 µs |
|---|---|---|
| 10 ms | 307–316 k, 172–173 / 922–979 | 23.2–23.6 k, 33–35 / 892–908 / 1,268–1,496 |
| 50 ms | 303–307 k, 176–179 / 940–1,005 | 23.0–24.0 k, 34–36 / 866–944 / 1,389–1,550 |

(A first version of these numbers measured 401 answers: the bench had no API-key provider. Review on #509.)

I/O deadlines are unaffected: `select` waits until the next I/O timer when one is sooner, and the timer
thread wakes the worker it schedules onto.

pp, after the consumer round (BRRTRouter 13d7ec7; median idle mcores per pod, same pods before and after):
PriceWhisperer 74.5 → 15.5 (all PW pods together 1,307 → 396), Sesame-IDAM 69 → 14.4, rerp 68 → 13; about
1.85 → 0.51 cores across the three.

## BRRTRouter — pass 5: every 5xx in pp, 19–26 Oct 2026

Source: Managed Prometheus `http_server_request_duration_seconds_count{http_response_status_code=~"5.."}` by job
and route over 7 days (hourly), the BRRTRouter "Request refused or failed" log events (route, status, reason) and
the services' own logs around each hour. Load-test runs on v0.1.138 had no 5xx.

| Where | Count | Cause | Fix |
|---|---:|---|---|
| trader-bff `POST /billing/test-subscriptions/reset` 502 | 78 (spread) | `proxy_untyped` sent a bodyless POST chunked; billing answered 200 and closed, and the BFF failed reading the answer (`reason: request`) | `Content-Length: 0` for bodyless requests (this PR); 65/400 → 0/400 in `tests/proxy_bodyless_tests.rs` |
| trader-bff orderbook, orders/risk, orders/stream, alerts/stream, portfolio/summary 502; platform-bff social sessions 502 | 49 | `connect` / `request` errors, all in rollout windows (24 Oct 08:00, 26 Oct 08:00–09:00) | connect retry after 100 ms; GET/HEAD resent once after a cut-off (this PR) |
| portfolio `/equity-curve` 500 (+ trader-bff 75) | 71 | response validation: `nullable: true` ignored by 3.1 | PriceWhisperer `ab1d7059` (25 Oct); the spec lint (#512) now flags it |
| trader-bff, every downstream route, 504 | 26 | 22 Oct 18:32–18:34: the node running the only trader-bff pod (and market) lost DNS (`node-local-dns` reload, ClickHouse "Temporary failure in name resolution", lifeguard slots wedged); downstreams logged no errors | none in BRRTRouter; the BFF runs one replica, so one node event fails every route. Recommendation: 2 replicas with a topology spread |
| platform-bff `/social/sessions*` 504/502 | 80 | 19 Oct 20:00–20:31: social's coroutines overflowed their stacks every 30–60 s until the 20:31 release | stack sizing (BRRTRouter 9bcb2db, `x-brrtrouter-stack-size`); none since |
| platform-bff `/archive/coverage(/summary)` 504 | 34 | 19 Oct 11:00–15:00: archive's coverage query ran past the BFF's 30 s timeout | fixed in archive that day; p99 now 0.44 s, all 200 |
| platform-bff `/team/members` 502 | 3 | 19 Oct 13:33: Sesame `/admin/users/query` answered 501 (not yet implemented) | implemented in Sesame; none since |
| trader-bff `/auth/start`, `/auth/callback` 503 `session_unavailable` | 10 (19, 20, 22, 25 Oct) | "session store: unexpected end of file": the BFF's kept Redis connection had been closed while idle, and the first command after the idle period failed | sesame-idam-client `RedisStore` retries once on a fresh connection when a reused one fails |

Not 5xx but seen while looking: platform-bff → billing 403 (×259, platform users without the operator role).

## BRRTRouter — pass 6: spec lint fixed in the consumers, and contract tests (26 Oct 2026)

**Lint.** Every `nullable: true` in a 3.1 spec is now a type list: PriceWhisperer market ×3 and orders ×1 (plus the
regenerated trader BFF), Sesame-IDAM ×111 across nine specs, and rerp in every 3.1 spec. The 3.0.3 specs (rerp invoice,
general-ledger) keep `nullable`; the BFF merge rewrites it (this PR). `brrtrouter-gen lint --fail-on-error` is now a
failing CI job in PriceWhisperer and Sesame-IDAM (`scripts/check-spec-lint.sh`). rerp's draft suites still have 1,596
`missing_schema_ref` and 225 `money_type_without_decimal_format` errors; its live specs (crm pipeline and accounts,
documents render) lint clean.

**Generator.** `type: [T, "null"]` now maps to `Option<T>` (#521), where it used to map to `serde_json::Value`. The
regenerated crates changed handler types in PriceWhisperer (billing, market, orders) and Sesame (api-keys, authz-core).
Each change was followed through the handlers, and the wire output is unchanged.

**Contract tests** (`brrtrouter::contract`, #521) on controllers that build JSON by hand:

| Where | Checked | Found |
|---|---|---|
| Sesame identity-session `/identity/me` | `profile_json` in four shapes; the 401 and 500 bodies | Types match. The body sends `phone`, `is_active` and `created_at`, none of which `UserProfile` declares (the spec has `phone_number`). The schema has no `required` and allows extra keys, so validation could not see this. Only `undeclared_fields` reports it. |
| PriceWhisperer billing | `list_plans`, `get_entitlements` (every plan, and none), checked strictly; `ApiError` against every declared error schema | **Bug:** `POST /plan-changes` 409 was declared as a fresh `Quote`, but `apply()` answers `{error, message}` (`quote_expired`, `subscription_changed`, `not_applicable`), which is what the UI reads. Under `full` validation every such 409 would have been a 500. The spec now declares the error. Only 1 of billing's error statuses has a schema at all. |
| rerp crm pipeline | `list_stages` checked strictly; the auth gate's 401/403 against 10 declared schemas | Nothing. |

What `check` alone misses: a hand-built body passes it whenever the schema leaves keys open and nothing is `required`,
which is the case in all three consumers. `assert_strict` (`check` plus no undeclared keys) is the test that pins field
names.

## BRRTRouter — pass 7: keep-alive pools and response header pairs (27 Oct 2026)

**Fork changes it builds on** (may_minihttp `integration/microscaler-fork`): request smuggling and
header-limit hardening, a liveness check on idle pooled connections before reuse, fewer client
allocations (header maps moved instead of cloned, buffered bodies read into one `Vec`, no
redundant `setsockopt` per request), and `Response::header_kv(name, value)`, which writes a
header pair into a reused buffer instead of a `format!`ed line.

**What changed here.**

- `fetch_*` and `proxy_untyped` send through process-wide pooled `Client`s (per origin: LIFO idle
  list, per-host cap, idle timeout, lifetime bound so kube-proxy can rebalance). SSE relays keep
  the one-connection path. Escape hatches: `BRRTR_HTTP_POOL=off` and `BRRTR_PROXY_POOL=off`
  restore the old path; the limits are `BRRTR_{HTTP,PROXY}_POOL_{MAX,MAX_PER_HOST,IDLE_SECS,LIFETIME_SECS}`
  (fetch 256 / 32 / 30 s / 120 s, proxy 1024 / 256 / 30 s / 60 s).
- A pooled connection the peer has already closed is dropped at checkout (a non-blocking peek),
  so the request, POST included, goes out on a fresh connection. A request cut off mid-flight on
  a reused connection is retried once only for GET/HEAD; a POST is answered 502 and not resent.
- `/metrics`: `brrtrouter_http_client_pool_{hits,misses,waits,stale_retries}_total{pool}` and
  `brrtrouter_http_client_pool_evictions_total{pool,reason="expired"|"discarded"}`.
- Response headers go through `header_kv`.
- The bench's downstream now keeps connections alive (it used to answer `Connection: close`,
  which made pool reuse impossible to measure).

**Numbers** (request_path bench, ms02, `BRRTR_BENCH_LOG=off`; allocations are deterministic,
µs/req is the median of three rounds on a shared host, load 18–57):

| scenario | allocs/req before | after |
|---|---:|---:|
| get_pet | 276.2 | 254.2 |
| list_pets_query | 302.2 | 280.2 |
| get_post_2_params | 279.2 | 257.2 |
| add_pet_json_body | 266.2 | 244.2 |
| secure_jwt | 287.1 | 268.1 |

Both columns use the same fork revision, so the −22 per request is this change: `header_kv`
(no `String` per response header).

| scenario | main | pool on | pool off |
|---|---:|---:|---:|
| bff_proxy_20k allocs/req | 2,469.3 | 2,461.3 | 2,449.3 |
| bff_proxy_20k µs/req (median) | 529 | 379 | 353 |
| bff_proxy_200k allocs/req | 22,276.8 | 22,267.4 | 22,256.0 |
| bff_proxy_200k µs/req (median) | 2,927 | 3,170 | 2,489 |

(These rows use the default validated proxy mode, not pass-through, hence the larger counts than
pass 2.) On loopback a TCP connect costs little, so the pool's latency gain here is within the
noise of a loaded host. What the pool saves in a cluster is the connect and kube-proxy/conntrack
work per call, and TIME_WAIT churn. In the fork's own client bench (16 coroutines, loopback),
fresh-connection vs pooled is 29.6k vs 151k req/s, p99 4.19 vs 0.30 ms.

**Rollout notes.** Idle timeouts must stay below the server side's keep-alive (Envoy/Contour
upstream idle timeout, Sesame's server keep-alive), or a pooled connection can be closed by the
peer just as it is reused. The liveness check catches a close that has already arrived; the
GET/HEAD retry covers the race. Per-host caps bound how many sockets one slow downstream can hold.

## For the PriceWhisperer pass

- benzinga-analyst-ingestor startup pass clones the whole JSON response 3–4× per 50-ticker
  batch (`microservices/ingestors/benzinga… main.rs` ~360–369) — its 94 Mi peak (from the
  memory agent). The `pw_alloc` crate (jemalloc + `/debug/heap` on every worker's metrics
  port) can measure allocation changes in workers.
