# Slide-by-Slide Pitch Deck: BRRTRouter & Lifeguard Ecosystem
## High-Performance Coroutine-Native Web Stack
**Author:** Charles Sibbald, Microscaler  
**Date:** June 25, 2026  

---

### Slide 1: Title Slide
* **Title**: BRRTRouter & Lifeguard
* **Subtitle**: The OpenAPI-First Coroutine Stack for High-Performance Microservices
* **Speaker**: Charles Sibbald, Microscaler
* **Visual**: Clean, modern dark theme layout featuring a stylized A-10 Avenger cannon logo, symbolizing speed, precision, and zero wasted cycles.

---

### Slide 2: The Problem: The Legacy Async Overhead
* **The Core Issue**: Traditional async Rust runtimes (e.g., Tokio) are built on stackless coroutines, requiring polling loops, dynamic task allocation, and heavy context switches under high load.
* **The Mismatch**: 
  * Integrating blocking database queries or stackful coroutines causes thread contention and latency spikes.
  * Development slows down due to manual boilerplate code mapping, route registration, and validation logic.
  * Shared infrastructure limits throughput to ~300 requests per core.
* **The Impact**: Higher cloud costs, unstable p99 latencies, and delayed release cycles.

---

### Slide 3: The Solution: Microscaler Coroutine Stack
* **A Single Unified Pipeline**:
  * **BRRTRouter**: An OpenAPI-first coroutine-native HTTP router.
  * **Lifeguard**: A coroutine-native Postgres ORM built for the `may` stackful coroutine runtime.
* **Core Advantages**:
  * Eliminates Tokio in favor of assembly-level context switching (<20ns switch overhead).
  * Direct execution of blocking-style operations without thread blocking.
  * Configurable lightweight stack size (`BRRTR_STACK_SIZE`), minimizing memory overhead.

---

### Slide 4: BRRTRouter: OpenAPI-First Generation
* **Design Once, Deploy Instantly**:
  * Code generator (`brrtrouter-gen`) converts OpenAPI 3.1.0 specifications into type-safe DTOs, JSON Schema validators, and handler stubs.
  * Integrates an $O(k)$ Radix Tree matcher for zero-heap routing lookup.
  * FS watcher watches spec changes to enable **Hot Reload** without restarting the server or dropping connections.
* **Result**: Zero configuration drift, complete API security compliance, and an instantaneous feedback loop.

---

### Slide 5: Lifeguard: Coroutine-Native Persistent Storage
* **Ergonomic Data Access**:
  * SeaORM-like ease-of-use with the performance of `may_postgres`.
  * Strict compile-time type-safety (e.g., mapping database UUID columns to `uuid::Uuid` instead of expensive strings).
  * Ergonomic, non-allocating query builder (`SelectQuery`).
* **Connection Routing**:
  * `LifeguardPool` segregates traffic into primary and replica connection pools.
  * Writes route to the primary node; scaled reads route to replicas after auditing replication lag.

---

### Slide 6: Asynchronous Cache Coherence (LifeReflector)
* **The Performance Cache-Aside Pattern**:
  * Writes commit directly to the primary database, keeping write response latencies low.
  * Database changes trigger lightweight asynchronous notifications via Postgres `LISTEN/NOTIFY`.
  * **LifeReflector** leader-elected daemon listens to triggers, updates Redis *only* if the key is already in the active cache (active-set coherence check).
* **Benefit**: Zero hand-written invalidation logic in the API handlers, avoiding cold cache bloat.

---

### Slide 7: The Developer Cockpit: SolidJS Dashboard
* **Everything Needed in One View**:
  * Bundled SolidJS + Vite developer console served automatically in local dev mode.
  * **Live Metrics Dashboard**: Real-time charts detailing requests, latency histograms, database pool utilization, and CPU usage.
  * **API Explorer**: An interactive client explorer with parameter forms and authentication support (OAuth2, JWT, API Key) for testing routes.
  * **SSE Channel Viewer**: Visual indicators and connection logs for Server-Sent Events streams.

---

### Slide 8: Verified Benchmarks & Horizontal Scale-Out
* **Real-World Metrics (2 CPU Cores / 512MB RAM)**:
  * Sustained Throughput: **1,536 requests per second** (average).
  * Median Latency: **8 ms** (sub-10ms target).
  * Failure Rate: **0.00%** (0 errors across 190k+ requests).
* **JSF AV Zero-Allocation Performance**:
  * Thread-local worker pools push peak performance to **67,000+ requests per second**.
  * Built-in queue-shedding protection returns `503 Service Unavailable` on overload, triggering Kubernetes Horizontal Pod Autoscalers (HPA) without memory leaks.

---

### Slide 9: Project Roadmap & Milestones
* **Phase 1: Foundations (Completed)**:
  * Core parser, Radix Tree routing, coroutine dispatch integration.
  * Code generator templates (Askama) and SolidJS developer dashboard UI.
* **Phase 2: Database & Caching (Completed)**:
  * Lifeguard ORM, primary/replica connection pooling, and LifeReflector Redis integration.
* **Phase 3: Production Validation (Active)**:
  * Production deployment validations (e.g., petstore and PriceWhisperer integrations).
  * Full OpenAPI 3.1.0 compliance audit (refer to compliance gap analysis).
* **Phase 4: Crates.io & Public Release (Upcoming)**:
  * Packaging and publishing library crates to crates.io.

---

### Slide 10: Conclusion: The Autonomous Software Stack
* **The Big Picture**: 
  * The BRRTRouter + Lifeguard stack is the default ingress and persistent runtime for Microscaler microservices.
  * High-density, low-latency execution ensures maximum performance on minimal infrastructure budgets.
* **Contact & Source**:
  * **Author**: Charles Sibbald, Microscaler
  * **Repository**: microscaler/BRRTRouter & microscaler/lifeguard
