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

## 🚧 Planned

Planned or in-progress tasks include:

- Typed handler deserialization
- Auto-generation of `From<HandlerRequest>` for typed requests
- Dynamic dispatcher route registration
- Hot reload of specifications
- Header and cookie extraction
- WebSocket support
- Server-side events
- Expanded test coverage and spec validation
- Improved coroutine handler ergonomics
- Benchmark suite targeting 1M+ matches/sec/core
- Middleware hooks for metrics, tracing, authentication, and CORS
- Packaging reusable SDKs on crates.io

## 🎯 Benchmark Goal

- Raspberry Pi 5, single core
- 1M route matches/sec
- ≤1 ms latency (excluding handler execution)



## links

- [Describing API Security](https://learn.openapis.org/specification/security.html)
- [API Server & Base Path](https://swagger.io/docs/specification/v3_0/api-host-and-base-path/)
- [Data Models](https://swagger.io/docs/specification/v3_0/data-models/data-models/)
- [Callbacks](https://swagger.io/docs/specification/v3_0/callbacks/)
- [GraphQL](https://swagger.io/docs/specification/v3_0/graphql/)
- [Rate limiting](https://swagger.io/docs/specification/v3_0/rate-limiting/)
- 