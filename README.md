# 🚀 BRRTRouter

> **OpenAPI-first HTTP router for Rust** — generate routing, validation, security,
> CORS, and handlers from your spec. Built on `may` coroutines.

[CI](https://github.com/microscaler/BRRTRouter/actions)
[Crate](https://crates.io/crates/brrrouter)
[Docs](https://docs.rs/brrrouter)

**OpenAPI:** 3.1.x fleet default · QUERY dual-support (3.1 + 3.2 promote) —
[version policy](docs/OPENAPI_VERSION_SUPPORT.md)

**Open reference product:** [Sesame-IDAM](https://github.com/microscaler/sesame-idam) —
[Building with BRRTRouter](docs/BUILDING_WITH_BRRTROUTER.md)

---

## What is BRRTRouter?

**BRRTRouter** turns an OpenAPI document into a type-safe HTTP server: radix routing,
schema validation, OpenAPI-driven auth, RFC-oriented CORS, metrics/tracing, and
codegen for typed handlers / BFF proxies.

Inspired by the *GAU-8/A Avenger* on the A-10 Warthog — precision dispatch with high
throughput. On a **2-core CI runner** (typical pod shape): **~1,500+ req/s**,
**sub-10 ms median**, **0% failures** (Goose). Hot-path work targets much higher
match rates under JSF constraints — see [Performance](#-performance--scale-out).

---

## Why BRRTRouter?

| Traditional approach | BRRTRouter |
| -------------------- | ---------- |
| Hand-written routes per endpoint | Routes + params from OpenAPI |
| Ad-hoc validation | JSON Schema + required params before handler |
| Bolt-on auth/CORS | `securitySchemes` + `config.yaml` + `x-cors` |
| Separate observability setup | Prometheus, OTEL, `/health`, `/metrics` |
| curl scripts forever | Pet Store dashboard + Swagger UI |
| Opaque local infra | Shared Kind + Tilt (`just dev-up`) |

---

## Status

Early-stage MVP — API may still change; seeking feedback toward a stable `0.1`.

| | |
| -- | -- |
| In-repo example | `examples/pet_store` |
| Public product reference | [Sesame-IDAM](https://github.com/microscaler/sesame-idam) |
| Active board | [Epic 12 — Framework maturity](docs/EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md) |
| Recent URI / QUERY work | [Epics 10–11](docs/EPICS/URI_REQUEST_TARGET/BUILD_BOARD.md) |

---

## Key capabilities

- **OpenAPI-first** — `paths`, params, bodies, `securitySchemes`, vendor extensions
- **O(k) radix routing** — `PathCursor` segment walk (not regex on the hot path)
- **Coroutine server** — `may` + `may_minihttp`
- **Security** — pluggable `SecurityProvider`s from OpenAPI + `config.yaml` (see below)
- **CORS** — global config + per-route `x-cors`; proxy/`Forwarded` + Private Network Access (see below)
- **Validation** — JSON body schema; required query/header/path params → **400**; body caps → **413**
- **HTTP QUERY (RFC 10008)** — router, CORS, proxy/fetch helpers; declare on `openapi: 3.1.0`
- **BFF auto-proxy** — transparent proxies with `may_http` connection pooling
- **Observability** — Prometheus, OpenTelemetry, structured tracing
- **Hot reload** — rebuild router on spec change
- **SSE** — `x-sse` + `sse::channel` (buffered responses; not WebSocket)
- **Typed handlers** — codegen + `#[handler]`; panics recovered via `catch_unwind`

**Parked:** native WebSocket upgrade (`may_minihttp` has no upgrade path). Prefer SSE or a sidecar.

---

## Security

Security is **OpenAPI-driven**: schemes in `components.securitySchemes`, requirements on
operations (including `security: []` for public routes). Providers auto-register from
the spec and can be refined in `config.yaml`.

| Scheme / provider | Role |
| ----------------- | ---- |
| API key (header / query / cookie) | Static or `RemoteApiKeyProvider` (cached remote check) |
| `BearerJwtProvider` | Local JWT (HMAC) + scopes |
| `JwksBearerProvider` | JWKS fetch (HS/RS), readiness, fail-closed on timeout/poisoning |
| `OAuth2Provider` | OAuth2 + scope checks |
| `SpiffeProvider` | SPIFFE JWT SVIDs (optional enterprise path) |
| PropelAuth / Auth0 / Cognito / Keycloak | Via JWKS URL (or PropelAuth helper in config) |

**Pipeline order (secured route):** match → **auth (401/403)** → param validation →
Content-Type / body schema → handler.

Details: [Security & Authentication](docs/SecurityAuthentication.md) ·
fail-closed design notes in `docs/DESIGN-security-scheme-fail-closed.md`.

---

## CORS

Production-oriented CORS middleware (RFC 6454-oriented):

- Global origins / methods / headers / credentials / `maxAge` from **`config.yaml`**
  (origins stay in deployment config, not the OpenAPI file)
- Per-route **`x-cors`**: `inherit` · `false` (disable) · object override
- Preflight short-circuit, credentials rules, exposed headers, `Vary`
- **`trust_forwarded_host`** — same-origin via `Forwarded` / `X-Forwarded-*` behind a trusted edge
- **Private Network Access** — `allow_private_network_access` for Chrome PNA preflights
- Metrics: `brrtrouter_cors_*` when wired to `MetricsMiddleware`
- QUERY included in permissive method lists where configured

Guides: [CORS.md](docs/CORS.md) · [CORS_OPERATIONS.md](docs/CORS_OPERATIONS.md) ·
[CORS_IMPLEMENTATION_AUDIT.md](docs/CORS_IMPLEMENTATION_AUDIT.md)

---

## Feature status (condensed)

### Ready

| Area | Notes |
| ---- | ----- |
| Spec load + radix router | OpenAPI 3.1; component `$ref` for schemas/parameters/**requestBodies/responses/pathItems** |
| Dispatch + typed codegen | Coroutine handlers; panic → 500 |
| Auth providers | API key, Bearer JWT, JWKS, OAuth2, remote API key, SPIFFE |
| CORS | Global + `x-cors`; forwarded host; PNA |
| Validation | Body schema; required params; **413** body caps; **415** Content-Type |
| URI / QUERY | Epic 10 request-target; Epic 11 QUERY method |
| Proxy / BFF | Downstream path resolve, connection cache |
| Observability | `/metrics`, OTEL, tracing middleware |
| Dev loop | Tilt + kind; hot reload; Pet Store UI + Swagger |

### In progress / gaps

| Area | Notes |
| ---- | ----- |
| Epic 12 waves 2–4 | Webhook outbound kit, multipart truth, multi-status codegen, perf science — [board](docs/EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md) |
| Broader OAS surface | Callbacks, full 3.2 feature matrix — [gap doc](OPENAPI_3.1.0_COMPLIANCE_GAP.md) |
| Fake OTEL in all tests | Partial |
| crates.io polish | Docs exist; publish packaging still open |
| WebSocket | **Parked** |

Stack sizing: [docs/stack_size.md](docs/stack_size.md) (`BRRTR_STACK_SIZE`,
`x-brrtrouter-stack-size`). Body limits: [docs/request_body_limits.md](docs/request_body_limits.md).
Params: [docs/parameter_validation.md](docs/parameter_validation.md).

---

## Quick start

1. Shared Kind (monorepo): `cd ../shared-kind-cluster && just dev-up`
2. This repo: `just dev-up` (Tilt)
3. Smoke: `curl http://localhost:8081/health` ·
   `curl -H "X-API-Key: test123" http://localhost:8081/pets`

Full steps: [CONTRIBUTING.md](CONTRIBUTING.md) · [LOCAL_DEVELOPMENT.md](docs/LOCAL_DEVELOPMENT.md)

**Goal:** running in under five minutes.

---

## Performance & scale-out

Engineered for **cloud-native scale-out** (≤2 CPU / ~500 Mi pods), not giant single nodes.

**Goose (2-core GHA, 20 users):** ~190k requests · **~1,536 req/s** · **~8 ms median** ·
**0% failures**. Details: [docs/PERFORMANCE.md](docs/PERFORMANCE.md).

- Fail-fast shedding → `503` under overload (helps HPA)
- JSF-inspired hot path (SmallVec, radix) for stable p50
- Stateless replicas — scale horizontally

---

## JSF AV Rules (hot path)

Standards inspired by the
[JSF AV C++ Coding Standards](https://www.stroustrup.com/JSF-AV-rules.pdf):

- Prefer **zero heap** on the request hot path (`SmallVec` params/headers)
- **O(k) radix** routing with `PathCursor`
- **Result-based** errors (no panics in dispatch; handler panics caught)

See [docs/JSF_COMPLIANCE.md](docs/JSF_COMPLIANCE.md).

---

## Quick reference

### URLs (Tilt / local-dev)

| Service | URL |
| ------- | --- |
| Dashboard / API | http://localhost:8081/ |
| Swagger | http://localhost:8081/docs |
| Health | http://localhost:8081/health |
| Metrics | http://localhost:8081/metrics |
| Grafana | http://localhost:3000 (admin/admin) |
| Prometheus | http://localhost:9090 |
| Jaeger | http://localhost:16686 |
| Tilt UI | http://localhost:10353 |

### Environment variables

| Variable | Purpose |
| -------- | ------- |
| `BRRTR_STACK_SIZE` | Coroutine stack (decimal or `0x8000`) — [stack_size.md](docs/stack_size.md) |
| `BRRTROUTER_MAX_REQUEST_BODY_OCTETS` | Global body ceiling (default 16 MiB) — [request_body_limits.md](docs/request_body_limits.md) |
| `BRRTROUTER_MAX_REQUEST_TARGET_OCTETS` | Max request-target length (default 8192) |
| `BRRTR_JWKS_FETCH_TIMEOUT_MS` | JWKS HTTP timeout (fail-closed) |

---

## Documentation

### Start here

- [Local Development](docs/LOCAL_DEVELOPMENT.md) — Tilt + kind
- [Building with BRRTRouter](docs/BUILDING_WITH_BRRTROUTER.md) — Sesame-IDAM reference
- [Development Guide](docs/DEVELOPMENT.md)
- [Testing](docs/TEST_DOCUMENTATION.md) · [Goose load testing](docs/GOOSE_LOAD_TESTING.md)

### Core

- [Overview](docs/BRRTRouter_OVERVIEW.md) · [Architecture](docs/ARCHITECTURE.md)
- [Request lifecycle & codegen](docs/RequestLifecycle.md)
- [Security & Authentication](docs/SecurityAuthentication.md)
- [CORS](docs/CORS.md) · [CORS operations](docs/CORS_OPERATIONS.md)
- [OpenAPI version support](docs/OPENAPI_VERSION_SUPPORT.md)
- [Component `$ref`](docs/openapi_component_refs.md) · [Parameter validation](docs/parameter_validation.md)
- [Request body limits](docs/request_body_limits.md) · [Stack size](docs/stack_size.md)
- [Epic 12 board](docs/EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md)
- [URI / QUERY epics](docs/EPICS/URI_REQUEST_TARGET/BUILD_BOARD.md)
- [OpenAPI 3.1 compliance gap](OPENAPI_3.1.0_COMPLIANCE_GAP.md)
- [Typed handlers & HTTP status PRD](docs/PRD_TYPED_HANDLER_HTTP_STATUS.md)

### Ops / contrib

- [Tilt implementation](docs/TILT_IMPLEMENTATION.md) · [Performance](docs/PERFORMANCE.md)
- [JSF compliance](docs/JSF_COMPLIANCE.md) · [Contributing](CONTRIBUTING.md)
- [Roadmap (archive pointer)](docs/ROADMAP.md) · [Publishing](docs/PUBLISHING.md)

```bash
just docs
# or: cargo doc --open
```

---

## Testing

Large automated suite (unit + integration + CORS/security HTTP conformance). Prefer:

```bash
just nt          # nextest
cargo test       # standard
```

Coverage target ≥80%. See [docs/TEST_DOCUMENTATION.md](docs/TEST_DOCUMENTATION.md).

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Prefer the
[Epic 12 board](docs/EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md) for current gaps.
Do not treat WebSocket as an MVP contribution target.

---

## Community

- [Issues](https://github.com/microscaler/BRRTRouter/issues)
- [Discussions](https://github.com/microscaler/BRRTRouter/discussions)

Bug reports: repro steps, expected vs actual, `just dev-status` + relevant logs.

---

## License

See [LICENSE](LICENSE).

---

## Logo & theme

Stylized **A-10 Warthog nose cannon** — precision routing, no stray shots.
