# BRRTRouter Roadmap

This document captures the high-level roadmap for the project. It consolidates the current feature set and outlines upcoming work.

## ✅ Completed

The following capabilities are already implemented in the main branch:

- OpenAPI 3.1 specification parser
- Routing table construction with regex-based path matching
- Coroutine-based HTTP server using `may_minihttp` and `may`
- Dynamic handler dispatch through a coroutine dispatcher
- Request context extraction (path, query, and JSON body)
- Example echo handler for testing
- Query parameter parsing
- JSON request body decoding
- 404/500 fallback responses
- Verbose CLI mode
- Modular design across `spec`, `router`, `dispatcher`, and `server`
- Coroutine-safe handler registry
- Zero I/O testing utilities
- Basic unit test coverage
- Server-side events
- Middleware hooks for metrics, authentication, 

## 🚧 Planned

Planned or in-progress tasks include:

- Investigate & implement config context
- Registered handlers output on startup 
- Extend fake otel collector across all tests
- handler coroutinge stack size from config context
- Docker compose for development (Otel-collector, Prometheus, Grafana, Loki)
- Dashboards for Otel-collector, Prometheus, Grafana, Loki targeting BRRTRouter
- implement tracing across entire codebsase
- Start rust doc book
- Start inline documentation of every public item
- Typed handler deserialization
- Auto-generation of `From<HandlerRequest>` for typed requests
- Dynamic dispatcher route registration
- Hot reload of specifications
- Header and cookie extraction
- WebSocket support
- Expanded test coverage and spec validation
- Improved coroutine handler ergonomics
- Benchmark suite targeting 1M+ matches/sec/core
- Middleware hooks for tracing, and CORS, CRSF
- Packaging reusable SDKs on crates.io

## 🎯 Benchmark Goal

- Raspberry Pi 5, single core
- 1M route matches/sec
- ≤1ms latency (excluding handler execution)



## Roadmap Update - May 2025


**Notes** - The repository’s tests still rely on a fixed sleep after starting the server.
Example from `tests/server_tests.rs` shows a `std::thread::sleep` after `HttpServer::start` to wait for readiness
path=tests/server_tests.rs git_url="https://github.com/microscaler/BRRTRouter/blob/main/tests/server_tests.rs#L33-L44"}.

A deterministic handshake and graceful shutdown are missing.

- `Dispatcher::register_handler` catches panics, but typed handlers spawned via `spawn_typed` do not.
- path=src/typed/typed.rs git_url="https://github.com/microscaler/BRRTRouter/blob/main/src/typed/typed.rs#L26-L75"}.
- Panic recovery testing is currently ignored (`#[ignore]`) in `test_panic_recovery` path=tests/server_tests.rs git_url="https://github.com/microscaler/BRRTRouter/blob/main/tests/server_tests.rs#L124-L154"}.
- `MetricsMiddleware` records the configured stack size, but actual used stack is not captured  path=src/middleware/metrics.rs git_url="https://github.com/microscaler/BRRTRouter/blob/main/src/middleware/metrics.rs#L50-L63"}


### Updated Roadmap 1.

**Stabilize and Harden Tests**

- Deterministic server start/stop
- Re-enable panic recovery test 

**Improve Coroutine Safety**

- Panic recovery for typed handlers
- Explicit AND/OR logic for multiple security providers
- Configurable stack size via API and stack-usage metrics 

**Enhance Developer Experience**

- Integrate `watch_spec` with dispatcher for live route updates
- Provide handler ergonomic helpers or macros
- Configurable CORS middleware 

**Extend Protocol Support**

- WebSocket handlers via `x-websocket` extension
- Finish SSE streaming implementation 

**Observability**
- Prometheus-compatible metrics endpoint
- OpenTelemetry tracing ready by default 

**OpenAPI Compliance**

- Request/response schema validation
- JWT/OAuth2 integration aligned with security requirements 

**Performance & Release**

- Benchmark-driven optimizations towards 1M req/sec
- Documentation overhaul and crate publishing

## Task List

### "Introduce server start handshake and graceful shutdown"}
1. Modify `may_minihttp::HttpServer` wrapper so `start()` returns a handle exposing `wait_ready()` and `stop()` methods.
2. Update all integration tests (e.g., `tests/server_tests.rs` and `tests/sse_tests.rs`) to call `wait_ready()` before sending requests and `stop()` instead of `coroutine().cancel()`.
3. Remove fixed `std::thread::sleep` calls and re-enable any ignored tests relying on them.

### "Catch panics in typed handlers"
1. In `src/typed/typed.rs`, wrap the handler invocation within `spawn_typed` in `std::panic::catch_unwind`.
2. On panic, send a `500` `HandlerResponse` like `register_handler` does and log the panic. 3. Un-ignore `test_panic_recovery` in `tests/server_tests.rs`.

### "Sequential security provider evaluation"
1. Refactor the loop in `AppService::call` around lines 154‑179 :codex-file-citation[codex-file-citation]{line_range_start=154 line_range_end=179 path=src/server/service.rs git_url="https://github.com/microscaler/BRRTRouter/blob/main/src/server/service.rs#L154-L179"}  to support `AND` and `OR` semantics for multiple schemes.
2. Provide a composite middleware or policy struct to coordinate multiple `SecurityProvider`s.
3. Add tests demonstrating both combined (AND) and alternative (OR) security requirements.

### "Expose programmatic stack configuration and metrics"
1. Add a `RuntimeConfig` struct (e.g., in `src/bin/config.rs`) that sets coroutine stack size via `may::config().set_stack_size`.
2. Capture used stack bytes when an odd stack size is configured and store them in `MetricsMiddleware`’s `used_stack` field.
3. Document the new configuration API and update examples/tests to use it.

### "Apply hot-reload updates to dispatcher"
1. Extend `hot_reload::watch_spec` so the reload callback also registers new routes with `Dispatcher::add_route`.
2. Store the returned watcher handle in `AppService` when `--watch` is enabled.
3. Add an integration test that modifies `openapi.yaml` during execution and verifies the new route responds without a restart.

### "Provide #[handler] macro for ergonomic handlers"
1. Create a `brrtrouter_macros` crate implementing a `#[handler]` attribute.
2. The macro should expand an async or sync function into a coroutine-compatible handler, auto-generating the necessary request conversion code.
3. Convert one example handler in `examples/pet_store` to use the macro and update docs.

### "Extend CORS middleware with configuration"
1. Enhance `src/middleware/cors.rs` to accept allowed origins, headers, and methods.
2. Detect and short-circuit OPTIONS preflight requests.
3. Update tests to cover custom origin and preflight behaviour.

### "Validate requests against OpenAPI schemas"
1. Integrate the `jsonschema` crate within `server::request` to validate parsed JSON bodies and parameters using schemas from `RouteMeta`.
2. On validation failure, return `400 Bad Request` with error details.
3. Add tests sending invalid payloads to demonstrate rejection.

### "Implement WebSocket support via x-websocket"
1. Add `src/websocket.rs` using `tungstenite` or `may_minihttp`’s upgrade API to manage WebSocket connections.
2. Recognize `x-websocket: true` in `build_routes` and dispatch to a `WebSocketHandler`.
3. Provide an example chat endpoint and corresponding tests.

### "Prometheus metrics integration"
1. Replace the manual text generation in `metrics_endpoint` with counters and histograms from the `prometheus` crate.
2. Expose `/metrics` in `AppService` using `prometheus::TextEncoder`.
3. Add tests verifying metric values after multiple requests.

### "Complete documentation and prepare crate"
1. Expand `docs/` with a user guide covering setup, middleware, SSE, and hot reload.
2. Ensure every public item has rustdoc comments.
3. Run `cargo publish --dry-run` and fix any warnings.


These tasks address the outstanding issues from the May 2025 roadmap while breaking large features—like hot reload and
coroutine ergonomics—into manageable steps. Implementing them will stabilize tests, enhance safety and observability,
and move BRRTRouter toward a polished 1.0 release.


## links

- [Describing API Security](https://learn.openapis.org/specification/security.html)
- [API Server & Base Path](https://swagger.io/docs/specification/v3_0/api-host-and-base-path/)
- [Data Models](https://swagger.io/docs/specification/v3_0/data-models/data-models/)
- [Callbacks](https://swagger.io/docs/specification/v3_0/callbacks/)
- [GraphQL](https://swagger.io/docs/specification/v3_0/graphql/)
- [Rate limiting](https://swagger.io/docs/specification/v3_0/rate-limiting/)
- 