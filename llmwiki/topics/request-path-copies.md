# Request-path copies and allocations

- **Status**: `active`
- **Code anchors**: `src/server/service.rs` (`AppService::call`), `src/dispatcher/core.rs`
  (`dispatch_with_id`, `HandlerRequest::without_payload`, `HandlerRequest::reply`),
  `src/security/jwks_bearer/validation.rs` (`cached_claims`), `src/validator_cache.rs`
  (`get_or_compile_for_route`), `src/sanitize.rs` (`headers_for_log`), `templates/handler.rs.txt`,
  `benches/request_path.rs`
- **Log / numbers**: [`docs/perf/copy-and-borrow-audit.md`](../../docs/perf/copy-and-borrow-audit.md)
- **Last updated**: 2026-10-25

## Measuring

`cargo bench --bench request_path` serves pet_store in-process with the generated middleware chain
(metrics, CORS), an API-key and a JWKS bearer provider, and drives browser-like keep-alive requests.
A counting allocator reports **allocations, bytes and µs per request**; `BRRTR_BENCH_LOG=off` drops
the INFO subscriber, `BRRTR_BENCH_SITES=<n>` samples allocation call sites, `BRRTR_BENCH_TRACE=<bytes>`
prints backtraces of large allocations. Allocation counts are deterministic; compare those first.

Release images carry no stacks for Cloud Profiler (thread names only); use `-debug` images or this
bench to rank functions.

## Invariants (do not regress)

- **No deep copy of the request between server and handler.** The dispatcher moves the
  `HandlerRequest` to the handler; middleware `after` hooks get `without_payload()` — method, path,
  handler, params, headers, cookies, **no body, no JWT claims**. Read those in `before`.
- **Typed handlers take the claims, not a copy**: `TryFrom<HandlerRequest>` does not see
  `jwt_claims`; they are on `TypedHandlerRequest::jwt_claims`.
- **JWKS claims cache is keyed by the token.** A token determines its header; only fully verified
  tokens are cached; provider policy is fixed once built. A hit parses no header, formats no key and
  clones an `Arc<Value>`; it still checks the key is in the JWKS, expiry, dynamic status and scopes.
  `extract_claims` makes the one deep copy the `HandlerRequest` API needs.
- **Validators are found per route by address** (`get_or_compile_for_route`), guarded by a
  `Weak<RouteMeta>` that pins the allocation. Never serialize or hash a schema per request.
- **Generated handlers build parameter schemas once** (`static LazyLock<Value>` per parameter).
- **DEBUG-only values are computed inside the `debug!`** (tracing evaluates fields only when the
  callsite is enabled), never in a `let` before it.
- **Reply channel is boxed** (`Sender<Box<HandlerResponse>>`): `may_queue` blocks are 64 slots.
- The access-log header JSON is written in one buffer, byte-identical to a sorted `serde_json::Map`
  (tested); keep it that way if the redaction rules change.

## Open

> **Open:** claims copy into every `HandlerRequest` (`Option<Value>` in a public API used ~350 times
> by consumers), CORS `after` string joins, `format!` per response header (needs a name/value API in
> the may_minihttp fork), BFF parse → validate → re-serialize of proxied JSON and a connection per
> proxied request. Details and evidence in the audit log.
