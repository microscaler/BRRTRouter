# High-Performance Coroutine-Native Microservices in Rust
## Technical Whitepaper: The BRRTRouter & Lifeguard Unified Stack
**Date:** June 25, 2026  
**Author:** Charles Sibbald, Microscaler  

---

## Abstract

In cloud-native web environments, microservices are typically constrained by low-resource footprints (e.g., Kubernetes pods running on 1–2 CPU cores). Traditional asynchronous Rust frameworks built on the Tokio runtime provide safety and concurrency but introduce context-switching, heap-allocation, and pipeline-synchronization overheads under high concurrent request volume. Moreover, when integrating blocking operations or attempting to map asynchronous execution to stackful coroutines, these runtimes face a fundamental mismatch.

This whitepaper presents the **BRRTRouter & Lifeguard** unified ecosystem: a coroutine-native, OpenAPI-first web stack built on the stackful coroutine runtime `may`. By compiling OpenAPI specifications directly into zero-allocation routing paths and managing persistent storage with a coroutine-native PostgreSQL ORM (`Lifeguard`) and an out-of-band cache-coherence engine (`LifeReflector`), this stack sustains **1,500+ requests per second with sub-10ms median latency on 2 CPU cores**, scaling up to **67,000+ requests per second under peak load**. We detail the design principles, performance optimizations, and coding disciplines (inspired by the Joint Strike Fighter AV C++ coding standards) that enable this lightweight concurrency model.

---

## 1. Introduction: The Async-Tokio Mismatch

Most Rust web applications rely on asynchronous futures driven by the Tokio thread-pool executor. While Tokio is highly optimized, it is based on **stackless coroutines** (state machines compiled from `async/await` syntax). Under the hood, this requires future polling, dynamic task allocation, and complex work-stealing mechanics.

### 1.1 The Coroutine Stack and Mismatch
When a system requires stackful coroutines—where execution contexts have independent stacks and can yield from deep nested calls without cascading state-machine code—integrating standard async frameworks becomes problematic. Stackful coroutines (such as those provided by the `may` runtime) compile to direct assembly context switches, bypassing Tokio's polling loops. Forcing an async executor on top of a coroutine stack:
1. Double-wraps concurrency states, causing CPU cache thrashing.
2. Leads to compilation deadlocks or runtime hangs when async tasks block thread pools over NFS or network drives.
3. Introduces runtime overhead due to task wrapping and heap allocation on the hot path.

### 1.2 The Solution
The Microscaler stack addresses this by eliminating Tokio. The entire path—from the HTTP server (`may_minihttp`), to the route dispatcher, to the database client (`may_postgres`), and connection pool—runs on `may` coroutines. The result is a unified stack with:
* Lightweight stack allocations (configurable via `BRRTR_STACK_SIZE`, defaulting to `0x4000` bytes).
* Native thread-local I/O connection pooling.
* Assembly-level context switching (<20ns switch time).

### 1.3 Threading and Concurrency Benefits of Coroutines
Traditional multi-threaded and async environments force developers to constantly manage synchronization primitives (such as mutexes, read-write locks, and atomic operations) to prevent data races and guarantee thread safety. This introduces CPU cache line bouncing and synchronization overhead. 

Stackful coroutines resolve this complexity:
* **Cooperative Scheduling**: Coroutines run sequentially on a fixed pool of thread-local workers. A coroutine executes business logic unimpeded until it performs I/O or yields explicitly.
* **Elimination of Lock Contention**: Because application handlers run without preemptive thread interruption, thread-safety concerns and resource locking are minimized. Developers get the simplicity of writing standard, synchronous-looking blocking code while attaining the blistering-fast execution speed of non-blocking I/O.
* **Low Context-Switch Cost**: Context switches are performed via direct register swaps in assembly (~20ns overhead) instead of kernel-space thread scheduling, yielding microservices that handle high concurrent request volumes without memory churn or lock thrashing.

---

## 2. BRRTRouter Architecture & OpenAPI Compilation

**BRRTRouter** compiles an OpenAPI 3.1.0 specification into a type-safe, validated HTTP router and handler registry.

```
┌───────────────────────────────────────────────┐
│              OpenAPI 3.1.0 Spec               │
└───────────────────────┬───────────────────────┘
                        │
                        ▼  (brrtrouter-gen templates)
┌───────────────────────────────────────────────┐
│        Type-Safe Request/Response DTOs        │
│          and Handler Trait Scaffolding        │
└───────────────────────┬───────────────────────┘
                        │
                        ▼  (Runtime HTTP Ingress)
┌───────────────────────────────────────────────┐
│        may_minihttp Server (Ingress)          │
└───────────────────────┬───────────────────────┘
                        │
                        ▼  (CORS & Tracing)
┌───────────────────────────────────────────────┐
│           RFC-Compliant Middleware            │
└───────────────────────┬───────────────────────┘
                        │
                        ▼  (Radix Tree Routing)
┌───────────────────────────────────────────────┐
│        O(k) Radix Tree Matcher (Zero Heap)    │
└───────────────────────┬───────────────────────┘
                        │
                        ▼  (MPSC Dispatcher)
┌───────────────────────────────────────────────┐
│       Coroutine Handler Execution (may)       │
└───────────────────────────────────────────────┘
```

### 2.1 Code Generation Pipeline (`brrtrouter-gen`)
At compile time, `brrtrouter-gen` reads the API spec, parses JSON Schema schemas, and utilizes the Askama templating engine to generate:
1. **Request/Response Structs**: Automatically annotated with `serde` for fast JSON serialization.
2. **Handler Traits**: Scaffolded blocks where developers write business logic.
3. **Registry and Main**: Setup loops that hook the routes into the coroutine server.

### 2.2 Radix Tree Routing and Zero-Heap Hot Path
Path parameters (e.g., `/pets/{id}`) are matched using an $O(k)$ Radix Tree, where $k$ is the URI length. 
To achieve maximum throughput:
* The router uses stack-allocated `SmallVec` structures to capture matched parameters and headers, avoiding heap allocations on the hot path.
* Routing tables are compiled once at boot time, supporting "last write wins" registration.
* A live spec file watcher allows **Hot Reload**—re-compiling the Radix Tree and swiping it in the active dispatcher thread-locally without losing active connections.

### 2.3 Dispatching and Channel Isolation
When a request is matched, it is converted into a `HandlerRequest` and passed via a lock-free Multi-Producer Single-Consumer (MPSC) coroutine channel to its dedicated handler coroutine. This keeps individual routes isolated; a panic in a handler is caught via `catch_unwind` at the dispatcher boundary, returning a `500 Internal Server Error` without crashing the global HTTP worker pool.

### 2.4 OpenAPI-First Development and Parallel UI Prototyping
In traditional architectures, API documentation is often an afterthought generated from code comments or annotations. This leads to drift between the implementation and the documentation, causing integration failures. 

BRRTRouter establishes the OpenAPI specification as the **single source of truth** for the microservice:
* **Strict Schema Contracts**: The schema validation layer compiles JSON Schema assertions from the spec directly into the routing path. Non-compliant client inputs are rejected before reaching business logic handlers.
* **Instantaneous Mock Prototyping**: Because the routing framework compiles handlers automatically, developers can immediately run mock routes (such as the default `echo_handler` or schema-driven auto-mock payloads).
* **Parallel UI Development**: Frontend designers and UI engineers do not have to wait for the backend database migrations, queries, and business logic to be complete. They can spin up the BRRTRouter mock service within seconds of defining the spec and start building/testing their interfaces against live, validating endpoints that simulate production responses.

---

## 3. The Lifeguard ORM & Persistent Data Access

Database persistence is handled by **Lifeguard**, a coroutine-native Postgres ORM. 

### 3.1 Coroutine-Native Driver Integration
Lifeguard replaces async ORMs (like SeaORM or SQLx) by targeting the `may_postgres` driver. It wraps standard database client connections inside coroutine execution loops, allowing developers to write linear, blocking-style SQL queries that yield the coroutine automatically during network I/O, yielding the underlying CPU thread to other active coroutines.

### 3.2 Lifeguard ORM Design
* **LifeModel and LifeRecord**: Developers define their schemas using `#[derive(LifeModel, LifeRecord)]` attributes. The macros generate SQL schemas, field mappings, and validation checks.
* **Type Safety for PostgreSQL Scalars**: Strict mapping of database columns (such as mapping Postgres `UUID` columns directly to `uuid::Uuid` instead of allocation-heavy `String` representation).
* **SelectQuery**: An ergonomic query builder built on `SeaQuery` that builds safe SQL statements compile-time without raw string building.

### 3.3 Connection Pooling (`LifeguardPool`)
`LifeguardPool` manages database connections using separate primary and replica worker pools:
* **Primary Connections**: Serve all writes (INSERT/UPDATE/DELETE) and strong-consistency reads (Read-Your-Writes).
* **Replica Connections**: Handle scaled reads when configured. The pool checks WAL replication lag to automatically route reads to healthy replicas.
* **Semaphore Constraints**: Bounded by a token semaphore to limit active database connections (ranging from 100 to 500 connections), preventing connection exhaustion under heavy load.

---

## 4. Cache Coherence & Background Synchronization (LifeReflector)

To support high-read workloads, the stack implements an out-of-band cache synchronization architecture using Redis and a background daemon called **LifeReflector**.

```
    ┌──────────────────┐               ┌───────────┐
    │  LifeRecord (ORM)│               │ App/Model │
    └────────┬─────────┘               └─────┬─────┘
             │                               │
       (1) Write                             │ (5) GET
             ▼                               ▼
    ┌──────────────────┐               ┌───────────┐
    │PostgreSQL Primary│               │Redis Cache│
    └────────┬─────────┘               └─────┬─────┘
             │                               │ (6) Miss
      (2) NOTIFY                             ▼
             ▼                         ┌───────────┐
    ┌──────────────────┐  (3) EXISTS   │ PG Read   │
    │  LifeReflector   ├──────────────►│ (Primary) │
    └────────┬─────────┘               └─────┬─────┘
             │                               │ (7) SETEX
             ├───────────────────────────────┘
             ▼ (4) Refresh Row (from Primary)
```

### 4.1 Synchronous Write Path
When a user updates a record, the ORM writes the change directly to the **PostgreSQL Primary** database (Step 1). The application does not block waiting for cache population, keeping response latencies low.

### 4.2 Asynchronous Change Listeners
Upon a successful commit, a PostgreSQL trigger fires a `NOTIFY table_changes, '{"id": 42}'` event (Step 2). **LifeReflector** (running as a background leader-elected coordinator) intercepts the notification.

### 4.3 Active-Set Coherence Check
1. **EXISTS Query**: LifeReflector checks if the key `lifeguard:model:table:42` is present in Redis (Step 3).
2. **Conditional Refresh**:
   * If the key is present (warm cache), LifeReflector fetches the fresh row from the PostgreSQL Primary (Step 4) and writes it back to Redis using `SETEX` with a predefined TTL.
   * If the key is not present (cold/expired cache), LifeReflector ignores the notification, preventing Redis from being filled with cold, unread records.

---

## 5. Extreme Performance & JSF AV Compliance

To ensure mission-critical stability and predictability, BRRTRouter enforces coding standards inspired by the **Joint Strike Fighter Air Vehicle C++ Coding Standards** (JSF AV Rules), modified for the Rust compiler.

### 5.1 Zero-Allocation Hot Path
1. **Stack Allocation**: Every variable on the HTTP dispatch loop is stack-allocated where possible. Collection boundaries are enforced using `SmallVec`.
2. **Fixed-Size Buffers**: Inbound HTTP header and routing param parsing does not request memory from the system allocator.
3. **No Panics**: All operations use monadic `Result<T, E>` error propagation. Production code blocks are configured with `clippy::unwrap_used` deny checks to block unsafe code paths.

### 5.2 Performance Metrics
In CI/CD environments matching standard resource footprints (2 CPU cores, 512MB RAM):
* **Sustained Throughput**: **1,536 requests per second** with **0% failure rates**.
* **Latency Profiles**:
  * Median Latency: **8 ms**
  * Average Latency: 12 ms
  * Peak Latency (p99): 121 ms for complex path parameter routing.
* **Worker Pool Scaling**: Under JSF-compliant worker configurations (avoiding lock contention by replacing global MPSC queues with thread-local MPMC worker pools), the stack sustains **67,000+ requests per second** at peak load.

---

## 6. Developer Experience & Observability

A high-performance stack must remain accessible to developer teams.

### 6.1 SolidJS Developer Console
BRRTRouter bundles a SolidJS + Vite developer dashboard, served from `/` in development mode.
* **Live Metrics Stream**: Displays active requests, response latency histograms, database connection pool utilization, and coroutine count.
* **API Explorer**: An interactive testing suite allowing developers to execute authenticated requests (OAuth2, JWT, API Key) directly from their browser.
* **SSE Connection Log**: Monitors Server-Sent Events streams (`x-sse`) with real-time payload inspecting.

### 6.2 Zero-Config Observability
The stack integrates with standard OpenTelemetry pipelines via custom tracing layers:
* **Prometheus Metrics**: Ingress endpoints export metrics on `/metrics` for scraping.
* **OTel/Jaeger Tracing**: Propagates trace contexts across the routing layers down to the Lifeguard query execution scopes.
* **Loki Log Aggregation**: Injects structured JSON messages with redacted authentication tokens and PII for centralized debugging.
