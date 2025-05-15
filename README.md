![BRRTRouter](docs/images/BRRTRouter.png)
# BRRTRouter

**BRRTRouter** is a high-performance, coroutine-powered request router for Rust, driven entirely by an [OpenAPI 3.1.0](https://spec.openapis.org/oas/v3.1.0) specification.

Inspired by the *GAU-8/A Avenger* on the A-10 Warthog, this router is designed to deliver precision request dispatch with massive throughput and low overhead.

### 🔭 Vision
Build the fastest, most predictable OpenAPI-native router in Rust — capable of millions of requests/sec, entirely spec-driven, and friendly to coroutine runtimes.

The expectation is that an api server implemented in Rust with BRRTRouter will be able to handle 1 million route match requests per second on a single core Raspberry Pi 5, with under 1ms latency.

This performance target precludes the cost of route handler dispatching, and assumes that the route handlers are implemented in a coroutine-friendly manner.

### 👁️ Logo & Theme
The logo features a stylized A-10 Warthog nose cannon, symbolizing BRRTRouter’s precision and firepower. This reflects our goal: maximum routing performance with zero stray shots.

---

## ✅ Current Foundation Status

### 🚧 Implemented Features

| Feature                        | Status | Description |
|-------------------------------|--------|-------------|
| **OpenAPI 3.1 parser**        | ✅     | Parses `paths`, `methods`, `parameters`, `schemas`, and `x-handler-*` extensions |
| **Routing table generation**  | ✅     | Compiles OpenAPI paths into regex matchers |
| **Path parameter extraction** | ✅     | Captures `{param}` values from URL paths |
| **Handler mapping**           | ✅     | Resolves handlers via OpenAPI `x-handler-*` |
| **CLI with `--verbose`**      | ✅     | Prints detailed routing logs on demand |
| **Full router test suite**    | ✅     | Covers all HTTP verbs, nested paths, edge cases |
| **Zero I/O testable spec**    | ✅     | `load_spec_from_spec()` allows testing without filesystem access |
| **Path + method dispatch**    | ✅     | Matches `(Method, Path)` to handler name and metadata |
| **Non-blocking design**       | ✅     | Prepared for coroutine-based request handling via `may` and `may_minihttp` |

---

## 🧪 Running Tests

Unit tests validate:

- All HTTP verbs: `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS`, `TRACE`
- Static and parameterized paths
- Deeply nested routes
- Unknown paths and fallback behavior

```bash
cargo test -- --nocapture
