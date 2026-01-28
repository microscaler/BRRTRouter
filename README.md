![BRRTRouter](images/BRRTRouter.png)

# 🚀 BRRTRouter

> **MVP-ready HTTP router for Rust, powered by OpenAPI 3.1.0**

[![CI](https://github.com/microscaler/BRRTRouter/actions/workflows/ci.yml/badge.svg)](https://github.com/microscaler/BRRTRouter/actions)
[![Crate](https://img.shields.io/crates/v/brrrouter.svg)](https://crates.io/crates/brrrouter)
[![Docs](https://docs.rs/brrrouter/badge.svg)](https://docs.rs/brrrouter)

---

## What is BRRTRouter?

**BRRTRouter** generates a complete, type-safe HTTP server from your OpenAPI specification. Write your API definition once, get routing, validation, middleware, observability, and handler scaffolding automatically. Ship production-ready APIs faster.

Inspired by the *GAU-8/A Avenger* on the A-10 Warthog, this router delivers precision request dispatch with massive throughput. Built on `may` coroutines for lightweight concurrency (800+ concurrent connections), it's designed for developers who want OpenAPI-first development without sacrificing performance.

---

## 🎯 Why BRRTRouter?

| Traditional Approach | BRRTRouter |
|---------------------|------------|
| Write routes manually for each endpoint | Generated from OpenAPI spec |
| Add validation per endpoint | Automatic from JSON Schema |
| Configure observability stack | Built-in, zero config (Prometheus, Jaeger, Loki) |
| Build admin/testing UI | Included (Sample SolidJS dashboard) |
| Setup local infrastructure | One command: `just dev-up` (Tilt + kind) |
| Test with curl scripts | Interactive dashboard with API testing |
| Memory leak hunting | Goose load tests (2+ minute sustained tests) |

✅ **Design Once, Deploy Everywhere**  
OpenAPI spec generates server, client SDKs, and docs

✅ **Production-Ready Day One**  
Observability, security, and error handling included

✅ **Developer Experience First**  
Hot reload, live metrics, comprehensive testing, 1-2s iteration cycle

---

## 🏗️ Architecture at a Glance

Detailed information on the systems architecture can be found in [Architecture Docs](./docs/ARCHITECTURE.md)

---
## 🎯 Early Stage MVP Notice

**BRRTRouter has reached Early Stage MVP status!**

This marks a **monumental milestone** - BRRTRouter has successfully transitioned from conceptual stage to early stage MVP. The tool now supports running both the **petstore** example crate and **PriceWhisperer** production crates, demonstrating real-world viability across different use cases.

**Status:**
- ✅ Core functionality working
- ✅ Multi-crate support (petstore + PriceWhisperer)
- ✅ Real-world production crate validation
- 🔧 API may change (breaking changes expected)
- 🔧 Performance optimization ongoing
- 🧪 Seeking early feedback and testing

**We welcome:**
- 📝 Documentation feedback
- 🐛 Bug reports
- 💡 API suggestions
- 🧪 Testing and experimentation

**One step closer to beta!** We're actively working toward v0.1.0 stable release.

---

## ✨ Key Features

- **📜 OpenAPI-First**: Your API spec is the single source of truth - routing, validation, and handlers generated automatically
- **🎨 Interactive Dashboard**: Production-ready SolidJS UI with live data, SSE streaming, and comprehensive API testing
- **⚡ Coroutine-Powered**: Built on `may` coroutines for lightweight concurrency (800+ concurrent connections on 1MB stack)
- **🔐 Security Built-In**: JWT/JWKS, OAuth2, API Keys with auto-registration from OpenAPI `securitySchemes`
- **🌐 RFC-Compliant CORS**: Full CORS support with route-specific configuration, credentials, and environment-specific origins
- **📊 Zero-Config Observability**: Prometheus metrics, OpenTelemetry tracing, health checks out of the box
- **🔥 Hot Reload**: Live spec reloading without server restart
- **🧪 Well-Tested**: 219 tests, 80%+ coverage, parallel execution support

---

## 🏃 Quick Start

See [CONTRIBUTING.md](CONTRIBUTING.md#-quick-start) for complete setup instructions, including:
- Tilt + kind setup (recommended)
- Simple cargo run option
- Prerequisites and installation
- Observability stack overview

**Goal: Running in <5 minutes**

---

## 📸 See It In Action

See [CONTRIBUTING.md](CONTRIBUTING.md#-see-it-in-action) for the interactive dashboard demo and observability stack overview.

---

## ✅ Feature Status

### 🎯 Production-Ready (October 2025)
| Feature                                          | Status | Description                                                                                                                                                                                                                                     |
|--------------------------------------------------|--------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **Performance target (100k req/sec)**            | ✅      | Extensive work towards the the 100k req/sec goal has been undertaken on our JSF initiative.                                                                                                                                                     |
| **OpenAPI 3.1 Spec Parser**                      | ✅      | Parses `paths`, `methods`, parameters, and `x-handler-*` extensions                                                                                                                                                                             |
| **Routing Table Construction**                   | ✅      | Compiles OpenAPI paths into regex matchers with param tracking                                                                                                                                                                                  |
| **Coroutine-Based Server**                       | ✅      | Fully integrated with `may_minihttp` and `may` coroutine runtime                                                                                                                                                                                |
| **Dynamic Handler Dispatch**                     | ✅      | Request is dispatched to named handlers via coroutine channels                                                                                                                                                                                  |
| **Full Request Context Support**                 | ✅      | Request path, method, path params, query params, and JSON body all passed into the handler                                                                                                                                                      |
| **`echo_handler` Coroutine**                     | ✅      | Mock handler that serializes and returns all request input data                                                                                                                                                                                 |
| **Query Parameter Parsing**                      | ✅      | Fully extracted from the request URI and passed to handler                                                                                                                                                                                      |
| **Request Body Decoding (JSON)**                 | ✅      | JSON body is read and deserialized for POST/PUT/PATCH handlers                                                                                                                                                                                  |
| **404 and 500 Handling**                         | ✅      | Fallback responses for unknown routes or missing handlers                                                                                                                                                                                       |
| **Verbose Mode for CLI**                         | ✅      | `--verbose` flag enables OpenAPI parsing debug output                                                                                                                                                                                           |
| **Modular Design**                               | ✅      | Clean separation of `spec`, `router`, `dispatcher`, and `server` logic                                                                                                                                                                          |
| **Composable Handlers**                          | ✅      | Coroutine-safe handler registry for runtime dispatch                                                                                                                                                                                            |
| **Regex-Based Path Matching**                    | ✅      | Path parameters are extracted using fast regex matchers                                                                                                                                                                                         |
| **Zero I/O Testing Support**                     | ✅      | `load_spec_from_spec()` allows programmatic spec testing                                                                                                                                                                                        |
| **Test Coverage**                                | ✅      | 219 tests covering all HTTP verbs, paths, and fallback routing                                                                                                                                                                                  |
| **Swagger UI & Spec Endpoints**                  | ✅      | Bundled Swagger UI at `/docs` and spec served from `/openapi.yaml`                                                                                                                                                                              |
| **Prometheus metrics middleware**                | ✅      | Complete metrics collection for requests, responses, latency, auth failures; `/metrics` endpoint for Prometheus scraping                                                                                                                        |
| **Interactive Dashboard (SolidJS UI)**           | ✅      | Production-ready UI with live data, SSE streaming, API explorer/testing, authentication UI                                                                                                                                                      |
| **Pluggable Security Providers**                 | ✅      | `SecurityProvider` trait enables custom authentication schemes                                                                                                                                                                                  |
| **Server-Sent Events**                           | ✅      | `x-sse` extension with `sse::channel` helper; streaming fixes pending                                                                                                                                                                           |
| **JWT/OAuth2 & API Key Auth**                    | ✅      | `BearerJwtProvider`, `OAuth2Provider`, `JwksBearerProvider` (JWKS HS/RS algs), and `RemoteApiKeyProvider`; scope checks, cookie support, metrics, and OpenAPI-driven registration                                                               |
| **Schema validation**                            | ✅      | Request and response validation against OpenAPI JSON Schema with clear 400 errors; exercised in tests.                                                                                                                                          |
| **Improved handler ergonomics**                  | ✅      | Use `#[handler]` to implement the `Handler` trait automatically.                                                                                                                                                                                |
| **Fix flaky tests / deterministic startup**      | ✅      | Tests use a fixed sleep to wait for server readiness and cancel the coroutine abruptly.                                                                                                                                                         |
| **Investigate config context**                   | ✅      | A pragmatic way to pass Configuration across the entire code base, possibly with an immutable global config that is loaded at start time                                                                                                        |
| **Panic recovery for handlers**                  | ✅      | Un-typed handlers recover from panics using `catch_unwind`; typed handlers do not.                                                                                                                                                              |
| **Comprehensive logging/tracing**                | ✅      | Structured tracing with 49 runtime touchpoints across request lifecycle, routing, security, validation, dispatcher, handlers, and hot reload; JSON format with redaction, sampling, async buffering; dual output (stdout + Loki) for hot reload |
| **Multiple security providers**                  | ✅      | Multiple providers supported and auto-registered from OpenAPI schemes; per-route scheme enforcement tested; supports ApiKey, Bearer, OAuth2, JWKS, RemoteApiKey                                                                                 |
| **Code generation for typed handlers**           | ✅      | Complete template system generates `TryFrom<HandlerRequest>` impls, Request/Response structs with serde annotations; production-ready                                                                                                           |
| **Dynamic route registration**                   | ✅      | `Dispatcher::add_route` and `register_from_spec` working; used in production; tests cover this functionality                                                                                                                                    |
| **Structured tracing (OTEL)**                    | ✅      | OpenTelemetry tracing implemented with test support; integrated with Jaeger in Tilt environment                                                                                                                                                 |
| **Configurable stack size with instrumentation** | ✅      | Stack size comes from `BRRTR_STACK_SIZE` environment variable and is logged in metrics; no runtime API or used-stack metrics.                                                                                                                   |
| **Hot reload on spec change**                    | ✅      | `hot_reload::watch_spec` rebuilds the `Router`, the server automatically updates the dispatcher and registers new routes.                                                                                                                       |
| **RFC-compliant CORS middleware**                | ✅      | Full CORS implementation with origin validation, preflight handling, credentials support, exposed headers, preflight caching; route-specific config via OpenAPI `x-cors`; origins from `config.yaml`; regex patterns and custom validators; JSF-compliant startup processing |
| **Extend fake otel collector across all tests**  | 🚧     | Fake OpenTelemetry collector is used in just tests, but not all tests utilize it.                                                                                                                                                               |
| **handler coroutine stack size**                 | 🚧     | Coroutine stack size is set via `BRRTR_STACK_SIZE` env var, but not dynamically adjustable or measured.                                                                                                                                         |
| **Deep dive into OpenAPI spec**                  | 🚧     | OpenAPI spec parsing is basic; does not handle all features like `callbacks` and other functions. See [OPENAPI_3.1.0_COMPLIANCE_GAP.md](OPENAPI_3.1.0_COMPLIANCE_GAP.md) for the gap analysis and path to full OpenAPI 3.1.0 support.                                                                       |
| **WebSocket support**                            | 🚧     | Not implemented. Only SSE is available via `x-sse` flag.                                                                                                                                                                                        |
| **Documentation & packaging**                    | 🚧     | README and roadmap exist; crate not yet prepared for crates.io publication.                                                                                                                                                                     |

---

## 📊 Performance Benchmarks

**Current: ~81k req/s** with full OpenAPI validation, authentication, and JSON handling — competitive with Go's net/http.

**Key highlights:**
- **10,000 concurrent users** handled with 0% failures
- **JSF AV Rules** implementation doubled throughput from ~40k to ~81k req/s
- **16KB stack size** optimal (4x safety margin, 480 MB saved vs 64KB)
- **Zero allocations** in hot path (SmallVec for params/headers)

See [docs/PERFORMANCE.md](docs/PERFORMANCE.md) for complete benchmarks, load test results, and optimization details.


---

## 📈 Recent Progress (December 2025)

- **🛡️ JSF AV Rules Implementation**: Applied [Joint Strike Fighter coding standards](https://www.stroustrup.com/JSF-AV-rules.pdf) to hot path
  - Stack-allocated `SmallVec` for parameters and headers (zero heap in dispatch)
  - O(k) radix tree routing with "last write wins" semantics
  - Comprehensive Clippy configuration with JSF-inspired thresholds
  - Fixed critical MPSC→MPMC worker pool bug (was causing double-free crashes)
  - **Result: 67k req/s with 0% failures** at 4,500+ concurrent users (no breaking point found!)

- **🚀 Early Stage MVP Achievement**: BRRTRouter successfully supports both **petstore** example crate and **PriceWhisperer** production crates
  - Validated real-world production use cases beyond examples
  - Multi-crate support demonstrates tool maturity and flexibility
  - One step closer to beta release
  - Many thanks to the **PriceWhisperer.ai** startup team for trusting BRRTRouter with their mission-critical systems. Their testing and recommendations to adopt JSF have been transformational!

- **🎨 Sample SolidJS Dashboard**: Complete interactive UI showcasing all BRRTRouter capabilities
  - Live data display with auto-refresh and modal views
  - Real-time SSE streaming with visual connection indicator
  - API Explorer with 25+ endpoints and color-coded HTTP methods
  - Comprehensive API testing suite with parameter forms and body editors
  - Authentication UI with API Key + Bearer Token configuration
  - Professional design with SolidJS + Vite + Tailwind CSS

- **🎉 Tilt + kind Local Development**: Fast iteration (~1-2s) with full observability stack
  - Cross-compilation support for Apple Silicon → x86_64 Linux
  - Live binary syncing without container rebuilds
  - PostgreSQL and Redis included for multi-service testing
  - Docker Hub proxy cache (70% faster startup, saves ~4GB bandwidth/day)

- **🎉 100% Documentation Coverage**: All public APIs, impl blocks, complex functions, and test modules comprehensively documented

- **🌐 RFC-Compliant CORS Implementation**: Complete CORS middleware rewrite achieving full RFC 6454 compliance
  - Origin validation, preflight handling, credentials support, exposed headers, preflight caching
  - Route-specific CORS configuration via OpenAPI `x-cors` extension
    - `x-cors: false` - Disables CORS for route (no CORS headers, prevents cross-origin access)
    - `x-cors: "inherit"` - Uses global CORS config from `config.yaml`
    - `x-cors: { ... }` - Route-specific CORS configuration (merged with global origins)
  - Environment-specific origins from `config.yaml` (not in OpenAPI spec)
  - Advanced features: regex pattern matching, custom validation functions
  - JSF-compliant: all configuration processed at startup, zero runtime parsing
  - **26+ CORS-specific tests** (all passing), feature parity with Rocket-RS
  - **Production-ready** with comprehensive security posture

- **✅ Parallel Test Execution**: Fixed Docker container conflicts for nextest parallel execution (219 tests pass)

- **🦆 Goose Load Testing**: Comprehensive CI load tests covering ALL OpenAPI endpoints (unlike wrk)
  - Tests authenticated endpoints with API keys
  - Detects memory leaks via sustained 2-minute tests
  - Per-endpoint metrics with ASCII output for CI/CD
  - HTML reports with interactive visualizations

- **🔐 Security implementation - WIP**:
  - `JwksBearerProvider` with full JWKS support (HS256/384/512, RS256/384/512)
  - `RemoteApiKeyProvider` with caching and configurable headers
  - OpenAPI-driven auto-registration of security providers
  - Further testing with security backends required

- **📊 Enhanced Metrics**: Request counts, latency tracking, auth failure counters, stack usage monitoring

- **🔥 Hot Reload**: Live spec reloading with filesystem watching

- **📝 Code Generation**: Complete typed handler generation from OpenAPI schemas

---

## 🛡️ JSF AV Rules Compliance

BRRTRouter implements coding standards inspired by the [**Joint Strike Fighter Air Vehicle C++ Coding Standards**](https://www.stroustrup.com/JSF-AV-rules.pdf) (JSF AV Rules) — the same rigorous standards used in the F-35 fighter jet's flight-critical software.

**Key principles:**
- **Zero allocations** in hot path (SmallVec for params/headers)
- **O(k) radix tree** routing for predictable latency
- **Result-based** error handling (no panics in dispatch)
- **Stack-allocated** collections (JSF Rule 206)

**Results:** 81,407 req/s with 0% failures, 1ms p50/p99 latency.

See [docs/JSF_COMPLIANCE.md](docs/JSF_COMPLIANCE.md) for complete implementation details and validation results.

---

## 🛠️ Development

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for complete development guide, including:
- Prerequisites and setup
- Common tasks and workflows
- Service URLs and environment variables
- Working with generated code

---

## 📋 Quick Reference

### Service URLs (when Tilt is running)

| Service | URL | Purpose |
|---------|-----|---------|
| **🎨 Interactive Dashboard** | http://localhost:8080/ | **START HERE** - SolidJS UI with live data, SSE, API testing |
| **Pet Store API** | http://localhost:8080 | Main API (standard HTTP port) |
| **Swagger UI** | http://localhost:8080/docs | OpenAPI documentation |
| **Health Check** | http://localhost:8080/health | Readiness probe |
| **Metrics** | http://localhost:8080/metrics | Prometheus metrics |
| **Grafana** | http://localhost:3000 | Dashboards (admin/admin) |
| **Prometheus** | http://localhost:9090 | Metrics database |
| **Jaeger** | http://localhost:16686 | Distributed tracing |
| **PostgreSQL** | localhost:5432 | Database (user: brrtrouter, db: brrtrouter, pass: dev_password) |
| **Redis** | localhost:6379 | Cache/session store |
| **Tilt Web UI** | http://localhost:10351 | Dev dashboard (press 'space' in terminal) |

### Environment Variables

BRRTRouter reads `BRRTR_STACK_SIZE` to determine the stack size for coroutines. The value can be a decimal number or a hex string like `0x8000`. If unset, the default stack size is `0x4000` bytes.

---

## 📚 Documentation

**Organized by user journey, not by component**

### Getting Started
- [🚀 Local Development](docs/LOCAL_DEVELOPMENT.md) - **START HERE** for Tilt + kind setup
- [🛠️ Development Guide](docs/DEVELOPMENT.md) - Development workflow and common tasks
- [🧪 Testing](docs/TEST_DOCUMENTATION.md) - Complete test suite overview
- [🦆 Load Testing](docs/GOOSE_LOAD_TESTING.md) - Goose load testing guide

### Core Concepts  
- [📖 BRRTRouter Overview](docs/BRRTRouter_OVERVIEW.md) - What it is, concepts, core components, and how it works (with diagrams)
- [🏗️ Architecture](docs/ARCHITECTURE.md) - System design with Mermaid diagrams
- [🔄 Request Lifecycle & Code Generation](docs/RequestLifecycle.md) - End-to-end request flow from OpenAPI to response
- [🔐 Security & Authentication](docs/SecurityAuthentication.md) - OpenAPI-driven security with multiple auth providers
- [📋 OpenAPI 3.1.0 Compliance Gap](OPENAPI_3.1.0_COMPLIANCE_GAP.md) - Outstanding work for full OpenAPI 3.1.0 support
- [📡 Server-Sent Events](#-server-sent-events) - SSE implementation guide

### Performance
- [📊 Performance Benchmarks](docs/PERFORMANCE.md) - Performance results, benchmarks, and optimization
- [🛡️ JSF Compliance](docs/JSF_COMPLIANCE.md) - JSF AV Rules implementation and validation

### Operations
- [🏗️ Tilt Implementation](docs/TILT_IMPLEMENTATION.md) - Architecture of the dev environment
- [📁 K8s Directory Structure](docs/K8S_DIRECTORY_STRUCTURE.md) - Organized Kubernetes manifests
- [💾 Backup & Recovery](docs/VELERO_BACKUPS.md) - Velero backup system

### Contributing
- [🤝 Contributing Guide](CONTRIBUTING.md) - How to contribute to BRRTRouter

### Advanced
- [🔥 Flamegraphs](docs/flamegraph.md) - Performance profiling guide
- [🚀 Publishing](docs/PUBLISHING.md) - Release process for crates.io
- [📊 Roadmap](docs/ROADMAP.md) - Future plans and completed work

**Build and view docs locally:**

```bash
just docs
# or
cargo doc --open
```

---

## 🧪 Testing

**219 tests** covering all HTTP verbs, paths, routing, validation, security, and middleware.

See [docs/TEST_DOCUMENTATION.md](docs/TEST_DOCUMENTATION.md) for complete testing guide, including:
- Running tests (standard and parallel with nextest)
- Code coverage (≥80% required)
- Load testing with Goose
- Benchmarks and flamegraphs

---

## 🤝 Contributing

We welcome contributions from developers at all levels!

See [CONTRIBUTING.md](CONTRIBUTING.md) for:
- Getting started as a contributor
- Areas for contribution
- Code standards and documentation requirements
- Development workflow

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

---

## 📄 License

See [LICENSE](LICENSE) for details.

---

## 👁️ Logo & Theme

The logo features a stylized **A-10 Warthog nose cannon**, symbolizing BRRTRouter's precision and firepower. This reflects our goal: maximum routing performance with zero stray shots.
