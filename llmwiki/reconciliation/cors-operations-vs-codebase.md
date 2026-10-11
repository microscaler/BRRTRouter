# CORS Operations vs Codebase Reconciliation

- Status: verified
- Source docs: `docs/CORS_OPERATIONS.md`, `docs/CORS_IMPLEMENTATION_AUDIT.md`, `docs/CORS.md`

## Verified implementation anchors

1. CORS middleware surface and route-policy merge helpers are implemented and exported:
   - `src/middleware/cors/mod.rs`
2. Builder options in operations docs (`trust_forwarded_host`, `allow_private_network_access`, credentials/origin constraints) are present:
   - `src/middleware/cors/builder.rs`
3. RFC 7239 `Forwarded` parsing + authority derivation behavior is implemented:
   - `src/middleware/cors/forwarded.rs`
   - `src/middleware/cors/mod.rs`
4. CORS metrics counters and sink integration points are present:
   - `src/middleware/metrics.rs`
   - `src/middleware/cors/mod.rs`
5. HTTP-level conformance tests cover forwarded host, PNA, IDN bytes, and preflight auth interactions:
   - `tests/cors_http_conformance_tests.rs`
   - `tests/cors_http_security_schemes_tests.rs`
   - `tests/middleware_tests.rs`

6. Preflights for paths without a declared `options` operation are synthesized after the routing
   miss (`src/server/service.rs`, `Dispatcher::preflight` in `src/dispatcher/core.rs`):
   - trigger: `OPTIONS` + `Origin` + `Access-Control-Request-Method`, and a route exists for the
     requested method on that path; otherwise the routing 404 stands;
   - answered under the *target* route's handler name, so per-route `x-cors` policy applies;
   - no security check (browsers never send credentials on a preflight) and no handler runs;
   - middleware lifecycle: `before` hooks run in order and stop at the first answer (CORS);
     `after` runs only for middleware that were entered. Metrics registered ahead of CORS
     therefore close out the request; middleware behind CORS (rate limiting) never sees it.
   - A declared `options` operation keeps the normal path (security first, full dispatch).
   - Tests: `http_cors_synthesized_preflight_*`, `http_cors_options_without_matching_route_is_404`
     in `tests/cors_http_conformance_tests.rs`.

## Reconciled conclusions

- `docs/CORS_OPERATIONS.md` is aligned with current CORS middleware and test coverage for:
  - trusted forwarded-host behavior,
  - preflight handling under global OpenAPI security,
  - PNA headers and `Vary` semantics,
  - metrics sink wiring expectations.
- Existing docs correctly emphasize that deployment origins are configured outside OpenAPI and merged into route policy.

## Gaps / drift

- No material drift found in the currently reconciled CORS docs.
- Follow-up (non-blocking): periodically re-check browser-behavior guidance against latest Fetch/PNA behavior and project test matrix.
