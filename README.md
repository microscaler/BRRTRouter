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

| Feature                          | Status | Description                                                                                |
|----------------------------------|--------|--------------------------------------------------------------------------------------------|
| **OpenAPI 3.1 Spec Parser**      | ✅     | Parses `paths`, `methods`, parameters, and `x-handler-*` extensions                        |
| **Routing Table Construction**   | ✅     | Compiles OpenAPI paths into regex matchers with param tracking                             |
| **Coroutine-Based Server**       | ✅     | Fully integrated with `may_minihttp` and `may` coroutine runtime                           |
| **Dynamic Handler Dispatch**     | ✅     | Request is dispatched to named handlers via coroutine channels                             |
| **Full Request Context Support** | ✅     | Request path, method, path params, query params, and JSON body all passed into the handler |
| **`echo_handler` Coroutine**     | ✅     | Mock handler that serializes and returns all request input data                            |
| **Query Parameter Parsing**      | ✅     | Fully extracted from the request URI and passed to handler                                 |
| **Request Body Decoding (JSON)** | ✅     | JSON body is read and deserialized for POST/PUT/PATCH handlers                             |
| **404 and 500 Handling**         | ✅     | Fallback responses for unknown routes or missing handlers                                  |
| **Verbose Mode for CLI**         | ✅     | `--verbose` flag enables OpenAPI parsing debug output                                      |
| **Modular Design**               | ✅     | Clean separation of `spec`, `router`, `dispatcher`, and `server` logic                     |
| **Composable Handlers**          | ✅     | Coroutine-safe handler registry for runtime dispatch                                       |
| **Regex-Based Path Matching**    | ✅     | Path parameters are extracted using fast regex matchers                                    |
| **Zero I/O Testing Support**     | ✅     | `load_spec_from_spec()` allows programmatic spec testing                                   |
| **Test Coverage**                | ✅     | Minimal Unit test suite covering all HTTP verbs, paths, and fallback routing               |

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
```


## 🧪 Running Tests

```bash
cargo test -- --nocapture

```

Unit tests validate:

- All HTTP verbs: `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS`, `TRACE`
- Static and parameterized paths
- Deeply nested routes
- Handler resolution
- Fallbacks (404/500) for Unknown paths and fallback behavior



```bash
cargo test -- --nocapture
```
---

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

---
## 📈 Contributing & Benchmarks
We welcome contributions that improve:
- 🧪 Test coverage and spec validation
- 🧠 Coroutine handler ergonomics
- 📊 Benchmarks for match throughput (goal: 1M+ matches/sec/core)
- 🔐 Middleware hooks (auth, CORS, metrics, tracing)
- 🧵 Typed handler deserialization

Benchmark goal:
- Raspberry Pi 5, single core
- 1M route matches/sec
- ≤1ms latency (excluding handler execution)