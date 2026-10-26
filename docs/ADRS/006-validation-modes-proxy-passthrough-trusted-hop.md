# ADR 006: Response validation modes, BFF pass-through and trusted-hop request validation

**Status:** Proposed  **Date:** 2026-10-26
**Relates to:** [ADR 002](002-BFF-Implementations-for-microservices.md), [ADR 003](003-k8s-downstream-service-proxy.md),
`docs/perf/copy-and-borrow-audit.md` (passes 2 and 3).

## Context

A request a browser makes to a PriceWhisperer page crosses two BRRTRouter services: the BFF, then the service
that owns the route. Today the path looks like this:

| step | BFF | service |
|---|---|---|
| request body | parse to `Value`, JSON-Schema validate, re-serialise to forward | parse, validate again, deserialise to typed `Request` |
| response body | parse downstream bytes to `Value`, validate, re-serialise | typed `Response` → `Value`, validate, serialise |

Measured cost (`benches/request_path.rs`, `bff_proxy_*`, ms02): a proxied 20 KB response cost the BFF 2,476 allocations
and 290 µs; a 200 KB one 22,285 allocations and 1.83 ms, almost all of it parse + validate + serialise of bytes the
service had already validated against the same schema (the BFF spec is merged from the service specs). Large pages
(the wave page, the portfolio greeks) pay it on every call.

Validation also earns its keep. On 25 Oct 2026 the PriceWhisperer suites (OpenAPI 3.1.0) still declared nulls with the
3.0 keyword `nullable: true`, which 3.1 does not have, so `get_equity_curve`'s honest day-one nulls failed response
validation: **every call for such a caller was a 500** (load test v0.1.126: 15/15; fixed in PW `ab1d7059`). Response
validation turned spec/implementation drift into a loud failure in pp instead of a silent wrong answer in a browser.
Two lessons:

- Drift must be caught **per build**, not first noticed per request: the nullable keyword is a lint error and the
  returned shape is testable from the generated types.
- Where validation runs per request, a mismatch must be **counted and logged**, never silently passed.

BRRTRouter #508 (merged 26 Oct, not yet in any consumer) made proxied 2xx JSON pass through **and skipped its
response validation by default**. That changed the default guarantee without approval; this ADR puts the default back
to `full` and makes pass-through a configured choice.

## Decision

### 1. Response validation is a mode

`BRRTR_RESPONSE_VALIDATION` (environment, read once at start; a `config.yaml` key can follow):

| mode | what is checked | on a mismatch | guarantee |
|---|---|---|---|
| `full` (**default**) | every response with a schema | 500 `Response validation failed` (today's behaviour), logged, counted | no response leaves that contradicts the spec |
| `sample:<rate>` (0 < rate ≤ 1) | that fraction of responses, evenly spread (every `1/rate`-th, the first included) | response goes out unchanged; logged (first per route per minute) and counted | drift is **detected** within minutes at any real traffic; users are not failed by it |
| `off` | nothing | — | none at runtime; relies on build-time checks (§4) |

- Metrics: `brrtrouter_response_validation_total{handler, result="ok"|"mismatch"}` for every response checked, and
  `brrtrouter_response_validation_skipped_total{handler}` for responses with a schema that the mode skipped. A
  mismatch is never passed silently: in `full` it fails the request, in `sample` it is logged and counted.
- An operation can force `full` for itself with `x-brrtrouter-validate-response: true` (contract-critical routes:
  orders, billing).
- **Environments:** dev, CI and pp stay `full`. The PriceWhisperer load test runs against pp in `full`, and it fails
  a release when any response it receives is a validation failure or when `result="mismatch"` grew during the run.
  Prod moves to `sample:<rate>` **only with Charles's approval**, per service, once the build-time checks in §4 are in
  place.

### 2. BFF pass-through is a configured choice

`BRRTR_PROXY_PASSTHROUGH` (environment), default **off** = today's behaviour (parse, validate, re-serialise).

When **on**, for routes the BFF only proxies (`x-service`, the generated untyped proxy controller):

- **Responses:** a 2xx JSON body goes back as the downstream's bytes (status and the allowed headers kept). Response
  validation follows the mode in §1: in `full` the bytes are parsed for the check only (no re-serialisation); in
  `sample` only sampled responses are parsed; in `off` none.
- **Streaming (phase 2):** a body larger than `proxy.stream_threshold` (default 256 KiB), or one the downstream
  sends chunked, is streamed to the client as it arrives instead of buffered, with `Content-Length` or chunked
  framing preserved, as Envoy does. A response that the mode requires to be checked is buffered (a stream cannot
  be validated before it is sent), so streaming happens only in `sample` (unsampled) and `off`.
- **Requests (phase 2):** after the BFF has validated the browser's request, it forwards the original bytes, not a
  re-serialised `Value`. This needs the raw body on `HandlerRequest` (a public-struct change, done as its own
  consumer round like #506).
- Errors (non-2xx), non-JSON content types and invalid UTF-8 keep the parsed path: their reasons feed logs and
  metrics.

### 3. Trusted-hop request validation (phase 3, gated on measurement)

A service may skip the **second** JSON-Schema pass over a request body that an internal caller has already
validated. It still deserialises into its generated types (serde enforces types and required fields), and it still
authenticates and authorises the user (the bearer JWT), which trust does not replace.

Trust has to be cryptographic and bound to the request. A plain header would not do, because a client could set it.
Nor would network identity alone: there is no mesh or mTLS in pp, and pod IPs are not a per-request proof.

- The BFF adds `x-brrtrouter-hop`, a compact JWS (HS256, `kid` for rotation) with these claims:
  - `iss` = the BFF's service name;
  - `aud` = the target service;
  - `htm` and `htu`: method and path with query;
  - `bd`: SHA-256 of the forwarded body bytes;
  - `rid`: the request id;
  - `exp`: at most 30 s ahead.

  The signing key comes from a Kubernetes Secret shared by the BFF and the suite's services (Crossplane/Flux,
  never in git).
- The service skips request schema validation only if every check passes:
  - `BRRTR_TRUSTED_HOP=on`;
  - the signature verifies with a known `kid`;
  - `aud` is itself and `exp` has not passed;
  - `htm` and `htu` match the request;
  - `bd` matches the bytes it received.

  If any check fails, it validates in full and counts `brrtrouter_trusted_hop_total{result}`. An inbound
  `x-brrtrouter-hop` from the internet is stripped by the BFF before it signs.
- **Why gated:** PriceWhisperer's request bodies are small (orders, preferences). Hashing the body for `bd` costs
  about as much as validating a small body, so the gain may not pay for the key management. Implement only if the
  BFF/service CPU per request and p95 show it. Responses (§1–2) carry the large payloads.

### 4. Drift is caught per build

- **Spec lint (CI, every suite):** `brrtrouter-gen lint` fails on 3.0-only keywords in a 3.1 document (`nullable`,
  `example` at schema level where 3.1 needs `examples`, `exclusiveMinimum: true`). This would have stopped `ab1d7059`'s
  bug before it reached pp.
- **Contract tests (generated):** the generator emits a test per operation. For every status code with a JSON
  schema, it builds the typed `Response` from the spec example, falling back to a generated instance with every
  optional field `None`/`null`, serialises it, and validates it against the operation's schema. The test runs in
  the consumer's `cargo test`, so a type and schema that disagree fail the build.
- **Load test:** §1. With `full` in pp it checks every release against the spec on real data.

## Consequences

- Default behaviour is unchanged until a mode or flag is set. Every saving is measured and switched on by
  configuration, per environment.
- With pass-through **on** and `full`, the BFF still parses each proxied body once (for the check) but no longer
  re-serialises it. Most of the cost goes only with `sample`/`off`, i.e. only where Charles approves it.
- The guarantee moves from "every request, every hop" to "every build (lint + contract tests) + every release
  (load test in `full`) + sampled production". That is stronger than today's for drift: today a schema bug can be
  served as 500s until someone notices.
- Phase 2 (streaming, raw request bytes) and phase 3 (trusted hop) are separate PRs with their own measurements.

## Implementation order

1. Modes + metrics (§1) and `BRRTR_PROXY_PASSTHROUGH` default off (§2 responses), restoring `full` as the default
   for proxied bodies after #508. Measure the BFF in `full` + pass-through on vs off (bench and pp load test).
2. Load test fails a release on validation mismatches (PriceWhisperer).
3. Spec lint and generated contract tests (§4).
4. Streaming and raw request bytes (§2 phase 2).
5. Trusted hop (§3), if the measurement in step 1 shows the request side is worth it.
