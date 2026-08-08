# 🚀 BRRTRouter

> **OpenAPI-first HTTP router for Rust** — generate a type-safe server (routing,
> validation, security, CORS, handlers, BFF proxy) from your OpenAPI 3.1 spec.
> Built on `may` coroutines.

[CI](https://github.com/microscaler/BRRTRouter/actions)
[Crate](https://crates.io/crates/brrrouter)
[Docs](https://docs.rs/brrrouter)

| | |
| -- | -- |
| **OpenAPI** | 3.1.x fleet default · QUERY dual-support — [version policy](docs/OPENAPI_VERSION_SUPPORT.md) |
| **Open reference** | [Sesame-IDAM](https://github.com/microscaler/sesame-idam) — [Building with BRRTRouter](docs/BUILDING_WITH_BRRTROUTER.md) |
| **In-repo demo** | `examples/pet_store` (Tilt dashboard, Swagger, Goose) |
| **Active roadmap** | [Epic 12 build board](docs/EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md) |

---

## What is BRRTRouter?

**BRRTRouter** treats your OpenAPI document as the source of truth. From one spec you get:

- O(k) **radix** routing (`PathCursor`) with path/query/header/cookie params
- Request/response **JSON Schema** validation and required-parameter checks
- **Security** providers wired from `securitySchemes` (+ `config.yaml`)
- **CORS** with global config and per-route `x-cors`
- Typed **handler codegen** / `#[handler]`, or zero-logic **BFF auto-proxy**
- Prometheus / OpenTelemetry, hot reload, SSE (`x-sse`)

Inspired by the *GAU-8/A Avenger* on the A-10 Warthog — precision dispatch at high
rate. On a **2-core CI runner** (typical K8s pod shape): **~1,500+ req/s**,
**~8 ms median**, **0% failures** (Goose). Hot-path JSF work has also sustained
much higher rates in dedicated load shapes — see [Performance](#-performance--scale-out).

---

## Why BRRTRouter?

| Traditional approach | With BRRTRouter |
| -------------------- | --------------- |
| Hand-written routes per endpoint | Routes + params compiled from OpenAPI |
| Ad-hoc validation | JSON Schema + required query/header/path → **400** |
| Bolt-on auth and CORS | `securitySchemes` + `config.yaml` + `x-cors` |
| Separate observability bootstrap | `/metrics`, OTEL, `/health` included |
| curl scripts as “docs” | Pet Store SolidJS dashboard + Swagger UI |
| Opaque local infra | Shared Kind + Tilt (`just dev-up`) |

---

## Status (MVP)

Early-stage MVP — APIs may still change; feedback welcome toward a stable `0.1`.

- ✅ Core server, radix routing, codegen, Pet Store + product suites
- ✅ Public reference: [Sesame-IDAM](https://github.com/microscaler/sesame-idam)
- ✅ Security + CORS production-hardened (see below)
- ✅ URI / request-target (Epic 10) + HTTP QUERY (Epic 11)
- 🔧 Framework maturity waves still open — [Epic 12 board](docs/EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md)
- ⏸ WebSocket **parked** (no `may_minihttp` upgrade path)

---

## Showcase: what you get from a spec

```yaml
# openapi.yaml (sketch)
openapi: 3.1.0
paths:
  /pets:
    get:
      operationId: listPets
      security: [{ ApiKeyHeader: [] }]
      parameters:
        - name: limit
          in: query
          schema: { type: integer }
      x-cors: inherit
      responses:
        "200":
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/PetList"
components:
  securitySchemes:
    ApiKeyHeader:
      type: apiKey
      in: header
      name: X-API-Key
```

From that shape BRRTRouter gives you: a radix route, required/optional query
checks, API-key enforcement, CORS policy merge, JSON response schema hooks, and
either a generated typed handler or a BFF proxy stub — without a second hand-written
router table.

**Try it:** [Quick start](#-quick-start) · dashboard at `http://localhost:8081/` ·
Swagger at `/docs`.

---

## Security

Security is **OpenAPI-driven**: declare `components.securitySchemes`, attach
requirements per operation (`security: []` = public). Providers auto-register from
the spec and can be refined in `config.yaml`.

| Provider / scheme | What it does |
| ----------------- | ------------ |
| API key (header / query / cookie) | Static key or `RemoteApiKeyProvider` (cached remote check) |
| `BearerJwtProvider` | Local JWT (HMAC) + scope checks |
| `JwksBearerProvider` | JWKS fetch (HS/RS families), readiness probes, fail-closed on timeout / key poisoning |
| `OAuth2Provider` | OAuth2 token + scopes |
| `SpiffeProvider` | SPIFFE JWT SVIDs (optional enterprise path) |
| PropelAuth / Auth0 / Cognito / Keycloak | Via JWKS URL (PropelAuth helper in config) |
| Custom | Implement `SecurityProvider` and register by scheme name |

**Request pipeline (secured route):**

1. Route match (radix)
2. **Auth** → 401 / 403 (before leaking param errors)
3. Required **parameter** validation → 400 + `fields[]`
4. Content-Type (**415**) / body schema (**400**) / body size (**413**)
5. Handler dispatch (panic → 500 via `catch_unwind`)

Details: [Security & Authentication](docs/SecurityAuthentication.md) ·
[fail-closed design](docs/DESIGN-security-scheme-fail-closed.md).

---

## CORS

RFC 6454–oriented middleware used in production dogfood:

| Capability | Detail |
| ---------- | ------ |
| Global policy | Origins, methods, headers, credentials, `maxAge`, expose headers from **`config.yaml`** |
| Per-route `x-cors` | `inherit` · `false` (disable) · object override (methods/headers/credentials/…) |
| Origins policy | Stay in **deployment config**, not OpenAPI (env-specific) |
| Preflight | OPTIONS short-circuit; credentials rules; exposed headers; `Vary` |
| Reverse proxies | `trust_forwarded_host` — `Forwarded` / `X-Forwarded-Host` / `X-Forwarded-Port` on a trusted path |
| Private Network Access | `allow_private_network_access` for Chrome PNA preflights |
| Metrics | `brrtrouter_cors_*` when chained with `MetricsMiddleware` |
| QUERY | Included in permissive allow-methods lists where configured |
| Startup cost | Policy compiled at startup (JSF-friendly; no per-request YAML parse) |

Guides: [CORS.md](docs/CORS.md) · [CORS operations](docs/CORS_OPERATIONS.md) ·
[implementation audit](docs/CORS_IMPLEMENTATION_AUDIT.md).

---

## Feature status

Legend: ✅ ready · 🚧 in progress / partial · ⏸ parked

### Runtime & routing

| Feature | Status | Notes |
| ------- | ------ | ----- |
| OpenAPI 3.1 loader | ✅ | `paths`, methods, params, bodies, `x-handler-*`, security |
| Component `$ref` | ✅ | schemas, parameters, **requestBodies**, **responses**, **pathItems** — [docs](docs/openapi_component_refs.md) |
| Radix routing (`PathCursor`) | ✅ | O(k) hot path; legacy regex table not used for dispatch |
| Coroutine server (`may` / `may_minihttp`) | ✅ | Lightweight concurrency |
| Dynamic handler dispatch | ✅ | Named handlers over coroutine channels |
| Request context | ✅ | Method, path, path/query/header/cookie params, JSON/form body |
| HTTP QUERY (RFC 10008) | ✅ | Router + CORS + proxy/fetch — [Epic 11](docs/EPICS/URI_REQUEST_TARGET/epic-11-http-query-method/README.md) |
| Request-target / URI limits | ✅ | Normalize, dual-stack checks, **414** — [Epic 10](docs/EPICS/URI_REQUEST_TARGET/BUILD_BOARD.md) |
| 404 / 500 / panic recovery | ✅ | Typed + untyped handlers use `catch_unwind` → 500 |
| Hot reload | ✅ | `hot_reload::watch_spec` rebuilds router + dispatcher |
| Dynamic route registration | ✅ | `Dispatcher::add_route` / `register_from_spec` |
| Stack sizing | ✅ | `BRRTR_STACK_SIZE`, `x-brrtrouter-stack-size`, heuristics — [stack_size.md](docs/stack_size.md) |
| WebSocket | ⏸ | No upgrade API in `may_minihttp`; use SSE or sidecar |

### Validation & safety

| Feature | Status | Notes |
| ------- | ------ | ----- |
| JSON Schema (request/response) | ✅ | Clear 400s; tests exercise path |
| Required query/header/path params | ✅ | Pre-handler **400** + `fields[]` — [parameter_validation.md](docs/parameter_validation.md) |
| Inbound body caps | ✅ | Global env + route estimate / `x-brrtrouter-body-size-bytes` → **413** — [request_body_limits.md](docs/request_body_limits.md) |
| Content-Type enforcement | ✅ | Undeclared type → **415** |
| Multipart form-data truth | 🚧 | Epic 12.6 |
| Multi-status typed / codegen | 🚧 | Epic 12.7 · [PRD](docs/PRD_TYPED_HANDLER_HTTP_STATUS.md) |

### Security & CORS

| Feature | Status | Notes |
| ------- | ------ | ----- |
| Pluggable `SecurityProvider` | ✅ | Custom schemes welcome |
| API key / Bearer JWT / JWKS / OAuth2 / remote API key | ✅ | OpenAPI auto-registration + config overlay |
| SPIFFE SVID validation | ✅ | Optional; independent of JWKS bearer |
| `security: []` public routes | ✅ | Does not inherit global security incorrectly |
| RFC-oriented CORS + `x-cors` | ✅ | See [CORS](#-cors) |
| Trusted forwarded host + PNA | ✅ | Edge / Chrome private-network flows |

### Codegen, BFF, UX

| Feature | Status | Notes |
| ------- | ------ | ----- |
| Typed handler codegen | ✅ | `TryFrom<HandlerRequest>`, request/response structs |
| `#[handler]` ergonomics | ✅ | Macro implements `Handler` |
| BFF auto-proxy | ✅ | Pure proxies skip unused stubs; `may_http` pooling |
| Swagger UI + `/openapi.yaml` | ✅ | Bundled at `/docs` |
| Interactive SolidJS dashboard | ✅ | Live data, SSE, API explorer, auth UI (Pet Store) |
| SSE (`x-sse`) | ✅ | Buffered channel helper (not byte-streaming upgrade) |
| Zero-I/O `load_spec_from_spec` | ✅ | Programmatic tests |

### Observability & ops

| Feature | Status | Notes |
| ------- | ------ | ----- |
| Prometheus `/metrics` | ✅ | Requests, latency, auth failures, CORS counters |
| OpenTelemetry tracing | ✅ | Jaeger in Tilt; structured tracing across lifecycle |
| Health endpoint | ✅ | `/health` |
| Fake OTEL collector in all tests | 🚧 | Used in some suites, not universal |
| crates.io packaging polish | 🚧 | Docs exist; publish packaging open |

---

## Roadmap

The live backlog is the epic boards (not the archived [ROADMAP.md](docs/ROADMAP.md)).

```text
Done     Epic 10 URI / request-target · Epic 11 QUERY · Epic 12 Wave 0–1
           (docs truth, body 413, component $ref, pre-handler params)
Now      Epic 12 Wave 2 — webhook outbound delivery kit (#396)
Next     Wave 3 — multipart truth · multi-status codegen
Later    Wave 4 — perf science (evidence before more radix churn)
Parked   WebSocket · fleet OpenAPI 3.2 cutover · full OAS callback runtime
```

| Wave | Stories | Theme |
| ---- | ------- | ----- |
| 0 ✅ | 12.1–12.2 | Doc truth · inbound body **413** |
| 1 ✅ | 12.3–12.4 | Component `$ref` · pre-handler params |
| 2 | 12.5 | Webhook outbound delivery kit (Sesame-shaped) |
| 3 | 12.6–12.7 | Multipart truth · multi-status typed/codegen |
| 4 | 12.8 | Perf science / Goose baselines |

Board: [docs/EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md](docs/EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md) ·
URI theme: [docs/EPICS/URI_REQUEST_TARGET/BUILD_BOARD.md](docs/EPICS/URI_REQUEST_TARGET/BUILD_BOARD.md).

Longer OAS surface gaps (callbacks, full 3.2 matrix):
[OPENAPI_3.1.0_COMPLIANCE_GAP.md](OPENAPI_3.1.0_COMPLIANCE_GAP.md).

---

## Quick start

**Goal: running in under five minutes**

1. Shared Kind (monorepo): `cd ../shared-kind-cluster && just dev-up`
2. This repo: `just dev-up` (Tilt)
3. Smoke:
   ```bash
   curl http://localhost:8081/health
   curl -H "X-API-Key: test123" http://localhost:8081/pets
   ```
4. Open the **dashboard** → http://localhost:8081/ · Swagger → `/docs`

Full setup: [CONTRIBUTING.md](CONTRIBUTING.md) · [LOCAL_DEVELOPMENT.md](docs/LOCAL_DEVELOPMENT.md).

---

## Performance & scale-out

Built for **cloud-native scale-out** (≤2 CPU / ~500 Mi pods), not giant single nodes.

### Goose — 2-core GitHub Actions (20 users)

| Metric | Value |
| ------ | ----- |
| Total requests | 190,434 |
| Aggregate throughput | **1,536 req/s** |
| Median latency | **8 ms** |
| Average latency | 12 ms |
| Failure rate | **0.00%** |
| Scenario throughput | 543 scenarios/s |

#### Latency by endpoint type (same run)

| Endpoint | Median | Avg | Max |
| -------- | ------ | --- | --- |
| Path-parameter routes | 9 ms | 12 ms | 121 ms |
| Static files | 3 ms | 5 ms | 112 ms |
| Health | 3 ms | 5 ms | 63 ms |
| Prometheus `/metrics` | 42 ms | 46 ms | 154 ms |
| CRUD (POST/DELETE) | 10 ms | 14 ms | 122 ms |

- Fail-fast shedding → **503** under overload (encourages HPA scale-out)
- JSF-inspired hot path (SmallVec, radix) keeps p50 stable
- Stateless replicas — add pods, add throughput

More: [docs/PERFORMANCE.md](docs/PERFORMANCE.md) · Goose guide: [docs/GOOSE_LOAD_TESTING.md](docs/GOOSE_LOAD_TESTING.md).

---

## JSF AV Rules (hot path)

Standards inspired by the
[JSF AV C++ Coding Standards](https://www.stroustrup.com/JSF-AV-rules.pdf):

- Prefer **zero heap** after init on the request hot path (`SmallVec` params/headers)
- **O(k) radix** routing with `PathCursor`
- **Result-based** errors in dispatch; handler panics recovered

See [docs/JSF_COMPLIANCE.md](docs/JSF_COMPLIANCE.md).

---

## Quick reference

### URLs (Tilt / local-dev)

| Service | URL | Purpose |
| ------- | --- | ------- |
| **Interactive dashboard** | http://localhost:8081/ | SolidJS UI — live data, SSE, API testing |
| Pet Store API | http://localhost:8081 | Main API (local-dev; k8s often still 8080) |
| Swagger UI | http://localhost:8081/docs | OpenAPI docs |
| Health | http://localhost:8081/health | Readiness |
| Metrics | http://localhost:8081/metrics | Prometheus |
| Grafana | http://localhost:3000 | Dashboards (admin/admin) |
| Prometheus | http://localhost:9090 | Metrics DB |
| Jaeger | http://localhost:16686 | Tracing |
| Tilt UI | http://localhost:10353 | Dev dashboard |

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
- [Roadmap archive pointer](docs/ROADMAP.md) · [Publishing](docs/PUBLISHING.md)

```bash
just docs
# or: cargo doc --open
```

---

## Testing

Large automated suite (unit, integration, CORS/security HTTP conformance, Goose in CI).

```bash
just nt          # nextest (preferred)
cargo test
```

Coverage target ≥80%. See [docs/TEST_DOCUMENTATION.md](docs/TEST_DOCUMENTATION.md).

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Prefer the
[Epic 12 board](docs/EPICS/FRAMEWORK_MATURITY/BUILD_BOARD.md) for current gaps.
WebSocket is parked — not an MVP contribution target.

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
