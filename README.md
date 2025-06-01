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

## 🔭 Vision

Build the fastest, most predictable OpenAPI-native router in Rust — capable of **millions of requests per second**, entirely spec-driven, and friendly to coroutine runtimes.

> We aim for **1 million route match requests/sec on a single-core Raspberry Pi 5**, with sub-millisecond latency.  
> This excludes handler execution cost and assumes coroutine-friendly request handling.

---

## 👁️ Logo & Theme

The logo features a stylized **A-10 Warthog nose cannon**, symbolizing BRRTRouter’s precision and firepower. This reflects our goal: maximum routing performance with zero stray shots.

---

## ✅ Current Foundation Status

### 🚧 Implemented Features (May 2025)
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
| **Fix flaky tests / deterministic startup**      | 🚧     | Tests use a fixed sleep to wait for server readiness and cancel the coroutine abruptly.                                                                                   |
| **Investigate config context**                   | 🚧     | A pragmatic way to pass Configuration across the entire code base, possibly with an immutable global config that is loaded at start time                                  |
| **Extend fake otel collector across all tests**  | 🚧     | Fake OpenTelemetry collector is used in just tests, but not all tests utilize it.                                                                                         |
| **handler coroutinge stack size**                | 🚧     | Coroutine stack size is set via `BRRTR_STACK_SIZE` env var, but not dynamically adjustable or measured.                                                                   |
| **implement tracing across entire codebsase**    | 🚧     | Tracing is implemented in some places, but not consistently across the entire codebase.                                                                                   |
| **Deep dive into OpenAPI spec**                  | 🚧     | OpenAPI spec parsing is basic; does not handle all features like `callbacks` and other functions, produce GAP analysis in order to completely support OpenAPI 3.1.0 spec. |
| **Panic recovery for handlers**                  | 🚧     | Un-typed handlers recover from panics using `catch_unwind`; typed handlers do not.                                                                                        |
| **Multiple security provider race**              | 🚧     | Security checks run sequentially in `AppService::call` but lack explicit combination logic.                                                                               |
| **Configurable stack size with instrumentation** | 🚧     | Stack size comes from `BRRTR_STACK_SIZE` environment variable and is logged in metrics; no runtime API or used-stack metrics.                                             |
| **Hot reload on spec change**                    | 🚧     | `hot_reload::watch_spec` rebuilds the `Router`, but the server doesn’t automatically update the dispatcher or routes.                                                     |
| **Code generation for typed handlers**           | 🚧     | Implemented via templates generating `TryFrom<HandlerRequest>` impls.                                                                                                     |
| **Dynamic route registration**                   | 🚧     | `Dispatcher::add_route` and `register_from_spec` allow runtime insertion; tests cover this.                                                                               |
| **Improved handler ergonomics**                  | ✅     | Use `#[handler]` to implement the `Handler` trait automatically. |
| **Structured tracing / metrics / CORS**          | 🚧     | Tracing and metrics middleware exist (with OTEL test support); CORS middleware returns default headers but is not configurable.                                           |
| **Schema validation**                            | 🚧     | Request/response validation against OpenAPI schema is not implemented.                                                                                                    |
| **WebSocket support**                            | 🚧     | Absent. Only SSE is available via `x-sse` flag.                                                                                                                           |
| **JWT/OAuth2 auth**                              | 🚧     | `BearerJwtProvider` and `OAuth2Provider` exist but examples don’t demonstrate combined schemes. Implement JWT mocking in tests                                            |
| **SPIFFE support**                               | 🚧     | SPIFFE fetching of X.509 and JWT SVIDs, bundles and supports watch/stream updates.                                                                                        |
| **Performance target**                           | 🚧     | Criterion benchmarks exist, but no explicit optimization work toward the 1M req/sec goal.                                                                                 |
| **Documentation & packaging**                    | 🚧     | README and roadmap exist; crate not yet prepared for crates.io publication.                                                                                               |

---

## 🧪 Try It

Run the coroutine server:

```bash
cargo run

curl "http://localhost:8080/items/123?debug=true" \
  -X POST \
  -H "Content-Type: application/json" \
  -d '{"name": "Ball"}'

> {
  "handler": "post_item",
  "method": "POST",
  "path": "/items/{id}",
  "params": { "id": "123" },
  "query": { "debug": "true" },
  "body": { "name": "Ball" }
}

curl http://localhost:8080/health
> { "status": "ok" }
```

Visit `http://localhost:8080/docs` to open the bundled Swagger UI powered by the
`/openapi.yaml` specification.

### Environment Variables

BRRTRouter reads `BRRTR_STACK_SIZE` to determine the stack size for
coroutines. The value can be a decimal number or a hex string like `0x8000`.
If unset, the default stack size is `0x4000` bytes.

## 🏗 Building the Pet Store Example
Run:

```bash
just build-pet-store
```

This wraps `./scripts/build_pet_store.sh` so you can pass cargo flags after the task.

## 🧪 Running Tests

```bash
just test
```

### 📈 Measuring Coverage

Install [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov):

```bash
cargo install cargo-llvm-cov
just coverage # runs `cargo llvm-cov --fail-under 80`
```

The command fails if total coverage drops below 80%.

## 🐳 Pet Store Docker Image

The `examples/pet_store` application can be packaged as a Docker image for
integration testing or deployment. A `Dockerfile` and `docker-compose.yml` are
included. Build and run the container with:

```bash
docker compose up -d --build
```

The Dockerfile automatically runs the `brrtrouter-gen` generator so the example
code is always up to date. The generated `doc` and `static_site` directories are
copied into the final image. The service listens on port `8080` and exposes the
`/health` endpoint for readiness checks.


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
creates spans for each request, `AuthMiddleware` performs a simple header token
check, and `CorsMiddleware` adds CORS headers to responses.

---
## 📡 Server-Sent Events

BRRTRouter can serve [Server-Sent Events](https://html.spec.whatwg.org/multipage/server-sent-events.html).
Mark a `GET` operation in your OpenAPI spec with the custom `x-sse: true` extension and
return `text/event-stream` content. Handlers use `brrrouter::sse::channel()` to emit events.
See [`examples/openapi.yaml`](examples/openapi.yaml) for the sample `/events` endpoint.

---
## 📈 Contributing & Benchmarks
For a detailed view of completed and upcoming work, see [docs/ROADMAP.md](docs/ROADMAP.md).
Please read [CONTRIBUTING.md](CONTRIBUTING.md) for instructions on generating the example code.
See [CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow and repository layout.
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
- 📊 Benchmarks for match throughput (goal: 1M+ matches/sec/core)
- 🔐 Middleware hooks 
  - Metrics
  - Tracing
  - Auth (JWT, OAuth, etc.) - routed to Sesame-IDAM or similar
  - CORS
- 💥 Reusable SDK packaging and publising to crates.io

Benchmark goal:
- Raspberry Pi 5, single core
- 1M route matches/sec
- ≤1ms latency (excluding handler execution)
