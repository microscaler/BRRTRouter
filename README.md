![BRRTRouter](docs/images/BRRTRouter.png)

# BRRTRouter

**BRRTRouter** is a high-performance, coroutine-powered request router for Rust, driven entirely by an [OpenAPI 3.1.0](https://spec.openapis.org/oas/v3.1.0) specification.

Inspired by the *GAU-8/A Avenger* on the A-10 Warthog, this router is designed to deliver precision request dispatch with massive throughput and low overhead.

---

## 🚀 Status & Badges

[![CI](https://github.com/microscaler/BRRTRouter/actions/workflows/ci.yml/badge.svg)](https://github.com/microscaler/BRRTRouter/actions)
[![Crate](https://img.shields.io/crates/v/brrrouter.svg)](https://crates.io/crates/brrrouter)
[![Docs](https://docs.rs/brrrouter/badge.svg)](https://docs.rs/brrrouter)

---

## ⚠️ Alpha Stage Notice

**This library is currently in alpha stage (v0.1.0-alpha.1).**

This documentation is published for **review and feedback purposes**, not for production adoption. 

**Status:**
- ✅ Core functionality working
- 🔧 API may change (breaking changes expected)
- 🔧 Performance optimization ongoing
- 🧪 Seeking early feedback and testing

**We welcome:**
- 📝 Documentation feedback
- 🐛 Bug reports
- 💡 API suggestions
- 🧪 Testing and experimentation

**Not recommended for production use yet.** Wait for v0.1.0 stable release.

---

## 🚀 Quick Start

### Option 1: Local Development with Tilt + kind ⭐ **RECOMMENDED**

**Fast iteration with full observability stack** (Prometheus, Grafana, Jaeger):

```bash
# Prerequisites: Docker, kind, kubectl, tilt (see docs/LOCAL_DEVELOPMENT.md)

# Clone the repository
git clone https://github.com/microscaler/BRRTRouter.git
cd BRRTRouter

# One-time setup: Create kind cluster
./scripts/dev-setup.sh

# Start Tilt (press 'space' for web UI)
tilt up

# 🎉 Services are now live!
# - Pet Store API: http://localhost:8080 (standard HTTP)
# - Grafana:       http://localhost:3000 (admin/admin)
# - Prometheus:    http://localhost:9090 (standard Prometheus)
# - Jaeger UI:     http://localhost:16686
# - PostgreSQL:    localhost:5432 (user: brrtrouter, db: brrtrouter)
# - Redis:         localhost:6379

# Test the API
curl -H "X-API-Key: test123" http://localhost:8080/pets
curl http://localhost:8080/health

# Query PostgreSQL
psql -h localhost -U brrtrouter -d brrtrouter

# Connect to Redis
redis-cli -h localhost -p 6379
```

**Why Tilt + kind?**
- ✅ **1-2 second iteration cycle** - edit code, see changes instantly
- ✅ **Cross-platform** - works reliably on macOS (Apple Silicon), Linux, Windows
- ✅ **Production-like** - Full Kubernetes environment locally
- ✅ **Observability built-in** - Prometheus, Grafana, Jaeger, OTEL
- ✅ **Multi-service testing** - PostgreSQL, Redis included
- ✅ **No port conflicts** - Isolated in kind cluster

📚 **[Complete Setup Guide →](docs/LOCAL_DEVELOPMENT.md)** | **[Architecture Details →](docs/TILT_IMPLEMENTATION.md)**

### Option 2: Simple cargo run

For quick testing without Kubernetes (single-service only):

```bash
# Clone the repository
git clone https://github.com/microscaler/BRRTRouter.git
cd BRRTRouter

# Run the pet store example
just start-petstore

# In another terminal, test the API
curl -H "X-API-Key: test123" http://localhost:8080/pets
curl http://localhost:8080/health
curl http://localhost:8080/metrics

# Visit Swagger UI
open http://localhost:8080/docs
```

### Generate Your Own Service

```bash
# Install the generator
cargo install --path . --bin brrtrouter-gen

# Generate a new service from your OpenAPI spec
brrtrouter-gen generate --spec your-api.yaml

# Your service is ready in the generated directory!
cd your_service
cargo run -- --spec doc/openapi.yaml --port 8080
```

---

## 📈 Recent Progress (October 2025)

- **🎉 Tilt + kind Local Development**: Fast iteration (~1-2s) with full observability stack (Prometheus, Grafana, Jaeger)
  - Cross-compilation support for Apple Silicon → x86_64 Linux
  - Live binary syncing without container rebuilds
  - Production-like Kubernetes environment locally
  - PostgreSQL and Redis included for multi-service testing
- **🎉 100% Documentation Coverage**: All public APIs, impl blocks, complex functions, and test modules comprehensively documented
- **✅ Parallel Test Execution**: Fixed Docker container conflicts for nextest parallel execution (219 tests pass)
- **🦆 Goose Load Testing**: Comprehensive CI load tests covering ALL OpenAPI endpoints (unlike wrk)
  - Tests authenticated endpoints with API keys
  - Detects memory leaks via sustained 2-minute tests
  - Per-endpoint metrics with ASCII output for CI/CD
  - HTML reports with interactive visualizations
- **🔐 Production-Ready Security**: 
  - `JwksBearerProvider` with full JWKS support (HS256/384/512, RS256/384/512)
  - `RemoteApiKeyProvider` with caching and configurable headers
  - OpenAPI-driven auto-registration of security providers
- **📊 Enhanced Metrics**: Request counts, latency tracking, auth failure counters, stack usage monitoring
- **🔥 Hot Reload**: Live spec reloading with filesystem watching
- **📝 Code Generation**: Complete typed handler generation from OpenAPI schemas


## 📈 Performance Benchmarks (Sep 2025)

### BRRTRouters requests **≈ 40 k req/s** 

| Stack / “hello-world” benchmark          | Test rig(s)*                               | Req/s (steady-state) | Comments                                |
| ---------------------------------------- | ------------------------------------------ | -------------------- | --------------------------------------- |
| Node 18 / Express                        | Same class HW                              | 8–15 k               | Single threaded; many small allocations |
| Python / FastAPI (uvicorn)               | Same                                       | 6–10 k               | Async IO but Python overhead dominates  |
| **Rust / BRRTRouter**                    | M-class laptop – 8 wrk threads / 800 conns | **≈ 40 k**           | Average latency ≈ 6 ms                  |
| Go / net-http                            | Same                                       | 70–90 k              | Go scheduler, GC in play                |
| Rust / Axum (tokio)                      | Same                                       | 120–180 k            | Native threads, zero-copy write         |
| Rust / Actix-web                         | Same                                       | 180–250 k            | Pre-allocated workers, slab alloc       |
| Nginx (static)                           | Same                                       | 450–550 k            | C, epoll, no JSON work                  |

*Community figures taken from TechEmpower round-20-equivalent and recent blog posts; all on laptop-grade CPUs (Apple M-series or 8-core x86).

---

### Interpretation

* **40 k req/s** with JSON encode/parse on every call is respectable for a coroutine runtime that **doesn’t** use a thread-per-core model.
* The concept of a Hello World is not really possible with BRRTRouter, as you always have a complete controller/handler path. Tests against the health endpoint match Axum; however, this is not a valuable example.
* It is, however, ~4–6× slower than the fastest Rust HTTP frameworks that exploit per-core threads, `mio`/epoll, and pre-allocated arenas.
* Socket-level errors (`connect 555`, `read 38 307`) show the client saturated or the server closed connections under load – this artificially deflates RPS a bit.

---

### Why BRRTRouter is currently a bit slower

| Factor                                                                                                                    | Impact |
| ------------------------------------------------------------------------------------------------------------------------- | ------ |
| **may_minihttp** does its own tiny HTTP parse; not as tuned as hyper/actix.                                               |        |
| Each request still goes through **MPSC** channel -> coroutine context switch -> `serde_json` parse even for small bodies. |        |
| Default coroutine **stack size** = 1 MB; 800 concurrent requests ⇒ 800 MB virtual memory ⇒ λ minor kernel pressure.       |        |
| No **connection pooling / keep-alive tuning** yet.                                                                        |        |


---

## 🔭 Vision

Build the fastest, most predictable OpenAPI-native router in Rust — capable of **millions of requests per second**, entirely spec-driven, and friendly to coroutine runtimes.

> We aim for **100K route match requests/sec on a single-core**, with sub-millisecond latency.  
> This excludes handler execution cost and assumes coroutine-friendly request handling.

---

## 👁️ Logo & Theme

The logo features a stylized **A-10 Warthog nose cannon**, symbolizing BRRTRouter’s precision and firepower. This reflects our goal: maximum routing performance with zero stray shots.

---

## ✨ Key Features

- **📜 OpenAPI-First**: Your API spec is the single source of truth - routing, validation, and handlers generated automatically
- **⚡ Coroutine-Powered**: Built on `may` coroutines for lightweight concurrency (800+ concurrent connections on 1MB stack)
- **🔐 Security Built-In**: JWT/JWKS, OAuth2, API Keys with auto-registration from OpenAPI `securitySchemes`
- **📊 Observability**: Prometheus metrics, OpenTelemetry tracing, health checks out of the box
- **🔥 Hot Reload**: Live spec reloading without server restart
- **🎨 Swagger UI**: Built-in interactive API documentation at `/docs`
- **✅ Validated**: Request/response validation against JSON Schema with clear error messages
- **🧪 Well-Tested**: 219 tests, 80%+ coverage, parallel execution support

---

## ✅ Feature Status

### 🎯 Production-Ready (October 2025)
| Feature                                          | Status | Description                                                                                                                                                               |
|--------------------------------------------------|--------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **OpenAPI 3.1 Spec Parser**                      | ✅      | Parses `paths`, `methods`, parameters, and `x-handler-*` extensions                                                                                                       |
| **Routing Table Construction**                   | ✅      | Compiles OpenAPI paths into regex matchers with param tracking                                                                                                            |
| **Coroutine-Based Server**                       | ✅      | Fully integrated with `may_minihttp` and `may` coroutine runtime                                                                                                          |
| **Dynamic Handler Dispatch**                     | ✅      | Request is dispatched to named handlers via coroutine channels                                                                                                            |
| **Full Request Context Support**                 | ✅      | Request path, method, path params, query params, and JSON body all passed into the handler                                                                                |
| **`echo_handler` Coroutine**                     | ✅      | Mock handler that serializes and returns all request input data                                                                                                           |
| **Query Parameter Parsing**                      | ✅      | Fully extracted from the request URI and passed to handler                                                                                                                |
| **Request Body Decoding (JSON)**                 | ✅      | JSON body is read and deserialized for POST/PUT/PATCH handlers                                                                                                            |
| **404 and 500 Handling**                         | ✅      | Fallback responses for unknown routes or missing handlers                                                                                                                 |
| **Verbose Mode for CLI**                         | ✅      | `--verbose` flag enables OpenAPI parsing debug output                                                                                                                     |
| **Modular Design**                               | ✅      | Clean separation of `spec`, `router`, `dispatcher`, and `server` logic                                                                                                    |
| **Composable Handlers**                          | ✅      | Coroutine-safe handler registry for runtime dispatch                                                                                                                      |
| **Regex-Based Path Matching**                    | ✅      | Path parameters are extracted using fast regex matchers                                                                                                                   |
| **Zero I/O Testing Support**                     | ✅      | `load_spec_from_spec()` allows programmatic spec testing                                                                                                                  |
| **Test Coverage**                                | ✅      | Minimal Unit test suite covering all HTTP verbs, paths, and fallback routing                                                                                              |
| **Swagger UI & Spec Endpoints**                  | ✅      | Bundled Swagger UI at `/docs` and spec served from `/openapi.yaml` |
| **Health & Metrics Endpoints**                   | ✅      | Built-in `/health` and `/metrics` for readiness and Prometheus scraping |
| **Pluggable Security Providers**                 | ✅      | `SecurityProvider` trait enables custom authentication schemes |
| **Server-Sent Events**                           | ✅     | `x-sse` extension with `sse::channel` helper; streaming fixes pending |
| **JWT/OAuth2 & API Key Auth**                    | ✅      | `BearerJwtProvider`, `OAuth2Provider`, `JwksBearerProvider` (JWKS HS/RS algs), and `RemoteApiKeyProvider`; scope checks, cookie support, metrics, and OpenAPI-driven registration |
| **Schema validation**                            | ✅      | Request and response validation against OpenAPI JSON Schema with clear 400 errors; exercised in tests.                                                                    |
| **Improved handler ergonomics**                  | ✅     | Use `#[handler]` to implement the `Handler` trait automatically. |
| **Fix flaky tests / deterministic startup**      | ✅     | Tests use a fixed sleep to wait for server readiness and cancel the coroutine abruptly.                                                                                   |
| **Investigate config context**                   | ✅     | A pragmatic way to pass Configuration across the entire code base, possibly with an immutable global config that is loaded at start time                                  |
| **Panic recovery for handlers**                  | ✅     | Un-typed handlers recover from panics using `catch_unwind`; typed handlers do not.                                                                                        |
| **Extend fake otel collector across all tests**  | 🚧     | Fake OpenTelemetry collector is used in just tests, but not all tests utilize it.                                                                                         |
| **handler coroutine stack size**                 | 🚧     | Coroutine stack size is set via `BRRTR_STACK_SIZE` env var, but not dynamically adjustable or measured.                                                                   |
| **implement tracing across entire codebase**     | 🚧     | Tracing is implemented in some places, but not consistently across the entire codebase.                                                                                   |
| **Deep dive into OpenAPI spec**                  | 🚧     | OpenAPI spec parsing is basic; does not handle all features like `callbacks` and other functions, produce GAP analysis in order to completely support OpenAPI 3.1.0 spec. |
| **Multiple security providers**                  | 🚧     | Multiple providers are supported and auto-registered from OpenAPI schemes; per-route scheme enforcement is covered by tests. Full OpenAPI OR-of-AND combination semantics are tracked in PRD. |
| **Configurable stack size with instrumentation** | 🚧     | Stack size comes from `BRRTR_STACK_SIZE` environment variable and is logged in metrics; no runtime API or used-stack metrics.                                             |
| **Hot reload on spec change**                    | 🚧     | `hot_reload::watch_spec` rebuilds the `Router`, but the server doesn’t automatically update the dispatcher or routes.                                                     |
| **Code generation for typed handlers**           | 🚧     | Implemented via templates generating `TryFrom<HandlerRequest>` impls.                                                                                                     |
| **Dynamic route registration**                   | 🚧     | `Dispatcher::add_route` and `register_from_spec` allow runtime insertion; tests cover this.                                                                               |
| **Structured tracing / metrics / CORS**          | 🚧     | Tracing and metrics middleware exist (with OTEL test support); CORS middleware returns default headers but is not configurable.                                           |
| **WebSocket support**                            | 🚧     | Absent. Only SSE is available via `x-sse` flag.                                                                                                                           |
| **SPIFFE support**                               | 🚧     | SPIFFE fetching of X.509 and JWT SVIDs, bundles and supports watch/stream updates.                                                                                        |
| **Performance target**                           | 🚧     | Criterion benchmarks exist, but no explicit optimization work toward the 100k req/sec goal.                                                                                 |
| **Documentation & packaging**                    | 🚧     | README and roadmap exist; crate not yet prepared for crates.io publication.                                                                                               |

---

## 🧪 Try It

Run the coroutine server:

```bash
just start-petstore

curl -i -H "X-API-Key: test123" -H "Content-Type: application/json" -d '{"name":"Bella"}' "http://0.0.0.0:8080/pets"
HTTP/1.1 200 Ok
Server: M
Date: Sat, 27 Sep 2025 19:15:27 GMT
Content-Length: 31
Content-Type: application/json
Content-Type: application/json

> {"id":67890,"status":"success"}


curl http://localhost:8080/health
> { "status": "ok" }
```



Visit `http://localhost:8080/docs` to open the bundled Swagger UI powered by the
`/openapi.yaml` specification.

Troubleshooting `/docs`:
- If you launch from a different working directory, pass an explicit docs path: `--doc-dir examples/pet_store/doc`.
- The `just start-petstore` task already sets the correct `--doc-dir`.

### Environment Variables

BRRTRouter reads `BRRTR_STACK_SIZE` to determine the stack size for
coroutines. The value can be a decimal number or a hex string like `0x8000`.
If unset, the default stack size is `0x4000` bytes.

## 🏗 Building the Pet Store Example
Run:

```bash
just build-pet-store
```

Builds the Pet Store example; you can pass cargo flags after the task.

## 🧪 Running Tests

```bash
# Standard cargo test
just test

# Fast parallel execution with nextest (recommended)
just nt

# All 219 tests pass reliably with parallel execution ✅
```

### 📈 Measuring Coverage

Install [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov):

```bash
cargo install cargo-llvm-cov
just coverage # runs `cargo llvm-cov --fail-under 80`
```

The command fails if total coverage drops below 80%.

### 🦆 Load Testing with Goose

BRRTRouter includes comprehensive load testing using [Goose](https://book.goose.rs/), which tests **ALL OpenAPI endpoints** (unlike wrk):

```bash
# Quick 30-second load test
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  -u10 -r2 -t30s \
  --header "X-API-Key: test123"

# Full load test with HTML report
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  -u20 -r5 -t2m \
  --no-reset-metrics \
  --header "X-API-Key: test123" \
  --report-file goose-report.html
```

**What Goose tests that wrk doesn't:**
- ✅ Authenticated endpoints (`GET /pets`, `/users` with API keys)
- ✅ All routes from OpenAPI spec (not just `/health`)
- ✅ Static files (`/openapi.yaml`, `/docs`, CSS, JS)
- ✅ Memory leak detection (sustained 2+ minute tests)
- ✅ Per-endpoint metrics with automatic failure detection

**CI Integration:**
Every PR runs a 2-minute Goose load test that:
- Tests 20 concurrent users across all endpoints
- Uploads ASCII metrics, HTML, and JSON reports
- Automatically fails if errors detected

See [`docs/GOOSE_LOAD_TESTING.md`](docs/GOOSE_LOAD_TESTING.md) for complete guide.

## 🐳 Docker & Container Deployment

### ⭐ Recommended: Tilt + kind (Local Development)

For **local development**, use the [Tilt + kind setup](#option-1-local-development-with-tilt--kind--recommended) which provides:
- Fast iteration with live updates
- Full observability stack
- Multi-service testing (PostgreSQL, Redis)
- Production-like Kubernetes environment

See [docs/LOCAL_DEVELOPMENT.md](docs/LOCAL_DEVELOPMENT.md) for complete setup.

### Legacy: Docker Compose (Deprecated)

⚠️ **The `docker-compose.yml` setup is deprecated** in favor of Tilt + kind for local development.

For simple single-container testing, you can still use:

```bash
docker compose up -d --build
```

However, this approach:
- ❌ Lacks observability (no Prometheus, Grafana, Jaeger)
- ❌ Slower iteration (full rebuild required)
- ❌ No multi-service support
- ❌ Less production-like

**For contributors**: Please use `tilt up` instead of `docker compose`.


Unit tests validate:

- All HTTP verbs: `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS`, `TRACE`
- Static and parameterized paths
- Deeply nested routes
- Handler resolution
- Fallbacks (404/500) for Unknown paths and fallback behavior

### 📊 Running Benchmarks

```bash
just bench
```

This executes `cargo bench` using Criterion to measure routing throughput.

Recent profiling with `flamegraph` highlighted regex capture and `HashMap`
allocations as hotspots. Preallocating buffers in `Router::route` and
`path_to_regex` trimmed roughly 5% off benchmark times on the expanded
throughput suite.

### 🔥 Generating Flamegraphs

Install the `cargo-flamegraph` subcommand by adding it as a development
dependency:

```toml
[dev-dependencies]
flamegraph = "0.6"
```

Run the profiler against the pet store example:

```bash
just flamegraph
```

The command produces `flamegraph.svg` in `target/flamegraphs/`. Open the file in
your browser to inspect hot code paths.
See [docs/flamegraph.md](docs/flamegraph.md) for tips on reading the output.




## 🔧 Handler Registration Example

```rust
use brrrouter::dispatcher::{Dispatcher, echo_handler};

let mut dispatcher = Dispatcher::new();

unsafe {
dispatcher.register_handler("list_pets", echo_handler);
dispatcher.register_handler("get_user", echo_handler);
dispatcher.register_handler("post_item", echo_handler);
}
```

Each handler runs in its own coroutine, receiving requests via a channel and sending back structured HandlerResponse.

### Using `#[handler]`

Controllers can derive the `Handler` trait automatically with the procedural macro:

```rust
use brrtrouter_macros::handler;
use brrtrouter::typed::TypedHandlerRequest;

#[handler(MyController)]
pub fn handle(req: TypedHandlerRequest<MyRequest>) -> MyResponse {
    // ...
}
```

---
## 🔌 Middleware

Middlewares run before and after each handler. Register them on the dispatcher:

```rust
use brrrouter::middleware::{
    AuthMiddleware, CorsMiddleware, MetricsMiddleware, TracingMiddleware,
};
use std::sync::Arc;

let mut dispatcher = Dispatcher::new();
dispatcher.add_middleware(Arc::new(MetricsMiddleware::new()));
dispatcher.add_middleware(Arc::new(TracingMiddleware));
dispatcher.add_middleware(Arc::new(AuthMiddleware::new("Bearer secret".into())));
dispatcher.add_middleware(Arc::new(CorsMiddleware));
```

`MetricsMiddleware` tracks request counts and average latency. `TracingMiddleware`
creates spans for each request, and `CorsMiddleware` adds CORS headers to responses.

Note: `AuthMiddleware` in examples is development-only. Prefer OpenAPI-driven `SecurityProvider`s (`BearerJwtProvider`, `OAuth2Provider`, `JwksBearerProvider`, `RemoteApiKeyProvider`) for production authentication.

---
## 🔐 Security & Authentication

BRRTRouter provides a pluggable `SecurityProvider` abstraction and auto-registers providers from your OpenAPI `components.securitySchemes`.

- **Bearer (mock/JWT)**: `BearerJwtProvider` validates simple dot-separated tokens and checks whitespace-separated `scope` claims; supports header and cookie extraction.
- **OAuth2 (mock)**: `OAuth2Provider` mirrors Bearer validation while matching OAuth2 schemes declared in OpenAPI.
- **JWKS (production)**: `JwksBearerProvider` fetches keys from a JWKS URL and validates JWTs using the token header algorithm. Supported: HS256/384/512 and RS256/384/512. Includes issuer/audience checking, exp with leeway, and TTL-based JWKS caching.
- **Remote API Keys**: `RemoteApiKeyProvider` verifies API keys via an HTTP endpoint, with configurable header name, timeout, and result caching. Also accepts `Authorization: Bearer <key>` fallback.

Config:

```yaml
security:
  # Global PropelAuth config (preferred)
  propelauth:
    auth_url: "https://auth.example.com"
    audience: "my-audience"
    # issuer, jwks_url, leeway_secs, cache_ttl_secs are optional; derived when omitted

  # Per-scheme JWKS (if not using PropelAuth)
  jwks:
    BearerAuth:
      jwks_url: "https://issuer.example/.well-known/jwks.json"
      iss: "https://issuer.example/"
      aud: "my-audience"
      leeway_secs: 30
      cache_ttl_secs: 300

  # Remote API key verification
  remote_api_keys:
    ApiKeyAuth:
      verify_url: "https://auth.example/verify"
      header_name: "X-API-Key"
      timeout_ms: 500
      cache_ttl_secs: 60
```

Manual provider wiring has been intentionally omitted; configure providers via YAML.

Authentication failures are tracked by `MetricsMiddleware` counters for observability.

Auto-registration from OpenAPI and config

- Providers are bound automatically from `components.securitySchemes` at startup.
- You can override or configure providers via `config/config.yaml` in generated apps. See `templates/config.yaml` for all available options (PropelAuth JWKS, per-scheme JWKS, static/remote API keys, cookie names, leeway/TTL).

TODO

- Add an optional CI workflow to validate against a real PropelAuth sandbox using repository secrets (auth_url, audience). Keep disabled by default to avoid external flakiness; primary tests remain hermetic with local JWKS/API-key mocks.

---

## 📚 Documentation

BRRTRouter has **100% comprehensive documentation** across all levels:

### For Contributors

- **🚀 Local Development**: [docs/LOCAL_DEVELOPMENT.md](docs/LOCAL_DEVELOPMENT.md) - **START HERE** for Tilt + kind setup
- **🏗️ Tilt Implementation**: [docs/TILT_IMPLEMENTATION.md](docs/TILT_IMPLEMENTATION.md) - Architecture of the dev environment
- **🤝 Contributing Guide**: [CONTRIBUTING.md](CONTRIBUTING.md) - Development workflow and standards
- **🧪 Test Documentation**: [docs/TEST_DOCUMENTATION.md](docs/TEST_DOCUMENTATION.md) - Complete test suite overview
- **🦆 Load Testing**: [docs/GOOSE_LOAD_TESTING.md](docs/GOOSE_LOAD_TESTING.md) - Goose load testing guide

### For API Users

- **📖 API Documentation**: `cargo doc --open` - All public APIs documented
- **🏗️ Architecture**: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) - System design with Mermaid diagrams
- **🚀 Publishing**: [docs/PUBLISHING.md](docs/PUBLISHING.md) - Release process for crates.io
- **📊 Roadmap**: [docs/ROADMAP.md](docs/ROADMAP.md) - Future plans and completed work

**Build and view docs locally:**

```bash
just docs
# or
cargo doc --open
```

---

## 📡 Server-Sent Events

BRRTRouter can serve [Server-Sent Events](https://html.spec.whatwg.org/multipage/server-sent-events.html).
Mark a `GET` operation in your OpenAPI spec with the custom `x-sse: true` extension and
return `text/event-stream` content. Handlers use `brrrouter::sse::channel()` to emit events.
See [`examples/openapi.yaml`](examples/openapi.yaml) for the sample `/events` endpoint.

---
## 🤝 Contributing

We welcome contributions from developers at all levels! 

### Getting Started as a Contributor

1. **🚀 Set up your development environment** (5 minutes):
   ```bash
   git clone https://github.com/microscaler/BRRTRouter.git
   cd BRRTRouter
   ./scripts/dev-setup.sh  # One-time: creates kind cluster
   tilt up                 # Start full dev stack
   ```

2. **✅ Verify everything works**:
   ```bash
   curl http://localhost:9090/health
   curl -H "X-API-Key: test123" http://localhost:9090/pets
   ```

3. **📖 Read the contribution guide**: [CONTRIBUTING.md](CONTRIBUTING.md)

4. **🔍 Pick an issue**: Look for [`good first issue`](https://github.com/microscaler/BRRTRouter/labels/good%20first%20issue) labels

5. **🧪 Run tests before committing**:
   ```bash
   just nt        # Fast parallel tests with nextest
   cargo fmt      # Format code
   ```

### Development Workflow

- **Edit code** in `src/` or `examples/pet_store/src/`
- **Tilt auto-rebuilds** and syncs (~1-2s)
- **Test immediately** with `curl` or Swagger UI
- **View logs**: `kubectl logs -f -n brrtrouter-dev deployment/petstore`
- **Check metrics**: http://localhost:3000 (Grafana)
- **Trace requests**: http://localhost:16686 (Jaeger)

See [docs/LOCAL_DEVELOPMENT.md](docs/LOCAL_DEVELOPMENT.md) for complete workflow.

### Areas for Contribution

We welcome contributions that improve:
- 🧵 Typed handler deserialization
- ✨ Auto-generation of impl `From<HandlerRequest>` for `TypedHandlerRequest<T>` based on schema
- 🚧 Dynamic dispatcher route registration
- 🚧 Hot reload
- 🚧 Header parsing and extraction
- 🚧 Cookie parsing and extraction
- 🚧 WebSocket support
- 🚧 Server-side events
- 🧪 Test coverage and spec validation
- 🧠 Coroutine handler ergonomics
- 📊 Benchmarks for match throughput (goal: 100k matches/sec)
- 🔐 Middleware hooks 
  - Metrics
  - Tracing
  - Auth (JWT, OAuth, etc.) - routed to Sesame-IDAM or similar
  - CORS
- 💥 Reusable SDK packaging and publising to crates.io

Benchmark goal:
- Raspberry Pi 5
- 100k route matches/sec
- ≤8ms latency (excluding handler execution)

---

## 📋 Quick Reference for Contributors

| Task | Command | Notes |
|------|---------|-------|
| **Setup dev environment** | `./scripts/dev-setup.sh && tilt up` | One-time cluster creation, then start Tilt |
| **Start development** | `tilt up` | All services with hot reload |
| **Stop development** | `Ctrl-C` or `tilt down` | Clean shutdown |
| **View Tilt UI** | Press `space` in terminal | Interactive dashboard |
| **Run tests** | `just nt` | Fast parallel execution (recommended) |
| **Run tests (standard)** | `just test` | Standard cargo test |
| **Format code** | `cargo fmt` | Required before commit |
| **Check coverage** | `just coverage` | Must be ≥80% |
| **Build docs** | `just docs` | Opens in browser |
| **Load test** | `cargo run --release --example api_load_test -- --host http://localhost:9090 -u10 -r2 -t30s` | Tests all endpoints |
| **View service logs** | `kubectl logs -f -n brrtrouter-dev deployment/petstore` | Real-time logs |
| **Restart service** | `kubectl rollout restart deployment/petstore -n brrtrouter-dev` | Force restart |
| **Check cluster status** | `just dev-status` | View all pods/services |
| **Teardown cluster** | `just dev-down` | Remove everything |

### Service URLs (when Tilt is running)

| Service | URL | Purpose |
|---------|-----|---------|
| **Pet Store API** | http://localhost:8080 | Main API (standard HTTP port) |
| **Swagger UI** | http://localhost:8080/docs | Interactive API docs |
| **Health Check** | http://localhost:8080/health | Readiness probe |
| **Metrics** | http://localhost:8080/metrics | Prometheus metrics |
| **Grafana** | http://localhost:3000 | Dashboards (admin/admin) |
| **Prometheus** | http://localhost:9090 | Metrics database (standard Prometheus port) |
| **Jaeger** | http://localhost:16686 | Distributed tracing |
| **PostgreSQL** | localhost:5432 | Database (user: brrtrouter, db: brrtrouter, pass: dev_password) |
| **Redis** | localhost:6379 | Cache/session store |
| **Tilt Web UI** | http://localhost:10351 | Dev dashboard (press 'space' in terminal) |

---

## 📞 Community & Support

- **Issues**: [GitHub Issues](https://github.com/microscaler/BRRTRouter/issues)
- **Discussions**: [GitHub Discussions](https://github.com/microscaler/BRRTRouter/discussions)
- **Roadmap**: [docs/ROADMAP.md](docs/ROADMAP.md)

**Found a bug?** Open an issue with:
- Steps to reproduce
- Expected vs actual behavior
- Output of `just dev-status` and relevant logs

**Have an idea?** Start a discussion or open a feature request!
