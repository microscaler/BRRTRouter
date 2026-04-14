# BRRTRouter Performance Analysis for 5000 r/s Target

**Date**: 2025-11-18  
**Target**: Sustained 5000 requests/second across the sample pet_store app  
**Current Performance**: ~40k r/s (baseline established)  
**Analyst**: Performance Optimization Review

## Executive Summary

BRRTRouter currently achieves ~40k requests/second in baseline testing, which is **8x higher** than the 5000 r/s target. However, this analysis identifies several architectural bottlenecks that could prevent sustained performance at 5k r/s under production conditions with full validation, authentication, and telemetry enabled.

### Key Findings

1. **✅ Route Matching**: Already optimized with radix tree (O(k) lookup) - **not a bottleneck**
2. **✅ Schema Validator Caching**: Already implemented - **not a bottleneck**  
3. **⚠️ Channel-Based Dispatch**: Single MPSC channel per handler is a potential bottleneck
4. **⚠️ Serialization Overhead**: serde_json parsing on every request/response
5. **⚠️ Lock Contention**: Multiple Arc<RwLock<T>> in hot paths
6. **⚠️ Coroutine Stack Overhead**: 64KB per coroutine with 800+ concurrent connections
7. **⚠️ Middleware Chain**: Sequential processing adds latency
8. **⚠️ Memory Allocations**: Per-request HashMap allocations in hot paths

### Risk Assessment for 5000 r/s

- **Low Risk** (unlikely to block 5k r/s): Route matching, validator cache
- **Medium Risk** (may cause degradation): Middleware overhead, lock contention  
- **High Risk** (likely bottleneck): Handler dispatch channels, serialization, memory allocations

---

## Detailed Analysis

## 1. Request Lifecycle & Hot Paths

### Current Flow (from ARCHITECTURE.md and source code)

```
Client Request
    ↓
[HTTP Parser] ← may_minihttp (C-level, fast)
    ↓
[AppService::call] ← Entry point
    ↓
[Middleware Chain] ← Sequential execution
    ├─ MetricsMiddleware (DashMap writes)
    ├─ MemoryMiddleware (atomic ops)
    └─ AuthMiddleware (security provider lookups)
    ↓
[Route Matching] ← Radix tree O(k) lookup (FAST ✅)
    ↓
[Security Validation] ← JWT verification, API key checks
    ↓
[Request Schema Validation] ← JSONSchema validation (cached ✅)
    ↓
[Dispatcher::dispatch] ← Channel send + recv
    ├─ MPSC channel send
    ├─ Handler coroutine recv
    ├─ Handler execution
    ├─ Channel send (response)
    └─ Dispatcher recv
    ↓
[Response Schema Validation] ← JSONSchema validation (cached ✅)
    ↓
[Response Serialization] ← serde_json::to_string
    ↓
[HTTP Response Write] ← may_minihttp
    ↓
Client
```

### Latency Budget Breakdown (Target: <200µs per request at 5k r/s)

Based on code analysis and PERFORMANCE_METRICS.md:

| Component | Current P99 | Budget | Status |
|-----------|-------------|--------|--------|
| HTTP Parsing | ~10µs | 20µs | ✅ Good |
| Route Matching | ~156µs | 100µs | ⚠️ Slightly high |
| Middleware Chain | ~50-100µs | 50µs | ⚠️ At limit |
| Security Validation | ~50-200µs | 100µs | ⚠️ Variable |
| Request Validation | ~50-100µs | 50µs | ✅ Cached |
| Handler Dispatch | ~50-200µs | 50µs | ❌ High variance |
| Handler Execution | Variable | N/A | Out of scope |
| Response Validation | ~50-100µs | 50µs | ✅ Cached |
| Serialization | ~20-50µs | 30µs | ✅ Acceptable |
| **Total (excl. handler)** | **~400-800µs** | **450µs** | ⚠️ Marginal |

**Observation**: With handler execution time, P99 latency could exceed 1ms under load.

---

## 2. Architecture Bottlenecks

### 2.1 Handler Dispatch Architecture ⚠️ HIGH IMPACT

**Current Implementation** (`src/dispatcher/core.rs:172-275`):

```rust
pub unsafe fn register_handler<F>(&mut self, name: &str, handler_fn: F)
where
    F: Fn(HandlerRequest) + Send + 'static + Clone,
{
    let (tx, rx) = mpsc::channel::<HandlerRequest>();  // ← Unbounded channel
    // ...
    coroutine::Builder::new()
        .stack_size(stack_size)
        .spawn(move || {
            for req in rx.iter() {  // ← Single receiver, sequential processing
                // Handler execution
                handler_fn(req);
            }
        });
    self.handlers.insert(name, tx);
}
```

**Problems**:

1. **Single Coroutine Per Handler**: Only ONE coroutine processes requests for each handler
   - Under load, requests queue up in the MPSC channel
   - No parallelism even if multiple cores available
   - Tail latency increases linearly with request rate

2. **Channel Overhead**: Every request goes through:
   - `mpsc::send()` (channel lock + wake receiver)
   - Coroutine context switch
   - `mpsc::recv()` (channel lock + block sender)
   - Handler execution
   - Response channel send/recv (same overhead)
   - **Total: 4 channel operations + 2 context switches per request**

3. **Unbounded Channel**: Can grow without limit under load spike
   - Memory pressure increases
   - Cache eviction causes cascading slowdown

**Measured Impact** (from code comments):
- Dispatch latency P99: 50-200µs (variable)
- Channel overhead: ~10-50µs per operation × 4 = **40-200µs total**

**Why It's a Bottleneck at 5k r/s**:
- 5000 req/s = 200µs per request budget
- If handler takes 100µs, only 100µs left for dispatch
- But dispatch overhead is 40-200µs → **queue builds up → latency spikes**

**Existing Mitigation** (`src/worker_pool.rs`):
- `register_handler_with_pool()` provides worker pools
- Configurable via `BRRTR_HANDLER_WORKERS` (default: 4)
- But: **NOT used by default in petstore** (only single-coroutine handlers)

**Evidence from Code**:
```rust
// examples/pet_store/examples/pet_store/src/registry.rs
// Generated handlers use single-coroutine dispatch:
dispatcher.register_handler("list_pets", move |req| { ... });
// NOT using:
// dispatcher.register_handler_with_pool("list_pets", move |req| { ... });
```

---

### 2.2 Lock Contention ⚠️ MEDIUM IMPACT

**Sources of Lock Contention**:

#### 2.2.1 Router Lock (Read-Heavy)

**Location**: `src/server/service.rs:27-28`
```rust
pub struct AppService {
    pub router: Arc<RwLock<Router>>,  // ← Read lock on every request
    pub dispatcher: Arc<RwLock<Dispatcher>>,  // ← Read lock on every request
}
```

**Usage Pattern**:
```rust
// In AppService::call() - every request acquires read lock
let router = self.router.read().expect("router lock poisoned");
let route_match = router.route(method, path);
```

**Problem**:
- 5000 r/s = 5000 read lock acquisitions/second
- RwLock has contention even for readers at high concurrency
- If 100 concurrent requests → potential contention spike

**Measured Impact** (from PERFORMANCE_METRICS.md):
- P99 lock acquisition: 50-100µs
- Contention events: <10 per 1000 requests (baseline)
- At 5k r/s: Could spike to 50+ contentions per 1000 requests

**Why It's Used**:
- Hot reload support requires mutable access to swap Router
- Current trade-off: hot reload convenience vs. performance

---

#### 2.2.2 Metrics Middleware (Write-Heavy) ✅ MITIGATED

**Location**: `src/middleware/metrics.rs:1-150`

**Already Optimized**:
```rust
/// Uses lock-free concurrent data structures (DashMap) for high-throughput scenarios.
pub struct MetricsMiddleware {
    // DashMap instead of RwLock<HashMap> ← Lock-free sharding
    path_metrics: DashMap<String, PathMetrics>,
    // Atomic counters
    total_requests: AtomicUsize,
    active_requests: AtomicUsize,
}
```

**Status**: ✅ Not a bottleneck (lock-free DashMap + atomics)

---

#### 2.2.3 ValidatorCache (Read-Heavy after warmup) ✅ MITIGATED

**Location**: `src/validator_cache.rs:206-215`

```rust
pub struct ValidatorCache {
    cache: Arc<RwLock<HashMap<String, Arc<JSONSchema>>>>,  // ← RwLock
    enabled: bool,
    spec_version: Arc<RwLock<SpecVersion>>,
}
```

**Usage Pattern**:
```rust
// Fast path: read-only lookup
{
    let cache = self.cache.read().expect("validator cache lock poisoned");
    if let Some(validator) = cache.get(&key) {
        return Some(Arc::clone(validator));  // ← Arc clone, no deep copy
    }
}
```

**Why It's Not a Bottleneck**:
1. Precompilation at startup → all validators cached before traffic
2. Read-only access during steady-state (no write contention)
3. Arc<JSONSchema> → cheap clone (just ref count bump)

**Status**: ✅ Not a bottleneck (read-only after warmup)

---

### 2.3 Memory Allocation Hot Paths ⚠️ MEDIUM IMPACT

**Per-Request Allocations** (identified from code):

#### 2.3.1 HashMap Allocations

**Location**: Multiple hot paths

```rust
// 1. Route parameters (src/router/core.rs)
pub struct RouteMatch {
    pub path_params: HashMap<String, String>,     // ← Heap allocation
    pub query_params: HashMap<String, String>,    // ← Heap allocation
}

// 2. Handler request (src/dispatcher/core.rs)
pub struct HandlerRequest {
    pub path_params: HashMap<String, String>,     // ← Clone from RouteMatch
    pub query_params: HashMap<String, String>,    // ← Clone from RouteMatch
    pub headers: HashMap<String, String>,         // ← Heap allocation
    pub cookies: HashMap<String, String>,         // ← Heap allocation
}
```

**Cost**:
- Each HashMap allocation: ~48 bytes + capacity overhead
- 4 HashMaps per request = ~200 bytes minimum
- At 5k r/s = 1MB/sec allocation rate (manageable but not ideal)
- GC pressure if many allocations

**Alternative**:
- Use `SmallVec` or `ArrayVec` for common cases (≤4 params)
- Pre-allocate with `HashMap::with_capacity()` based on route metadata

---

#### 2.3.2 String Cloning

**Location**: Throughout request path

```rust
// From HTTP headers → HandlerRequest
for (key, value) in headers {
    req.headers.insert(key.to_string(), value.to_string());  // ← 2 allocations
}

// Path/query parameters
req.path_params.insert(name.to_string(), value.to_string());  // ← 2 allocations
```

**Cost**:
- Average header: 10 bytes key + 20 bytes value = 30 bytes
- 10 headers = 300 bytes per request
- At 5k r/s = 1.5MB/sec allocation rate

**Alternative**:
- Use `Cow<'static, str>` for known header names
- Use `Arc<str>` for shared strings (e.g., handler names)

---

#### 2.3.3 JSON Serialization/Deserialization

**Location**: `src/server/request.rs` and response handling

```rust
// Request body parsing
if let Some(body) = req.body() {
    let body_str = std::str::from_utf8(body)?;
    let json: Value = serde_json::from_str(body_str)?;  // ← Parse + allocate
}

// Response serialization
let json_str = serde_json::to_string(&response.body)?;  // ← Serialize + allocate
```

**Cost**:
- Parsing: 1-10µs for small payloads (50-500 bytes)
- Serialization: 1-10µs for small payloads
- Memory: temporary buffer allocations

**Alternative**:
- Use `serde_json::from_slice()` to avoid UTF-8 validation
- Pre-allocate serialization buffer with capacity hint
- Consider `simd-json` for 2-3x faster parsing

---

### 2.4 Coroutine Stack Memory ⚠️ MEDIUM IMPACT

**Configuration** (`src/dispatcher/core.rs:182-191`):

```rust
let stack_size = std::env::var("BRRTR_STACK_SIZE")
    .ok()
    .and_then(|s| parse_size(s))
    .unwrap_or(0x10000); // 64KB default
```

**Memory Overhead**:
- 800 concurrent connections × 64KB = **51.2 MB** virtual memory
- May coroutines use guard pages → actual physical memory lower
- But: stack switching overhead on context switch

**Trade-off**:
- Smaller stacks (16KB) → faster context switches, less memory
- Larger stacks (64KB) → can handle deeper call stacks, safer

**Current Setting**: 64KB (conservative, safe for deep handlers)

**Recommendation**: Profile actual stack usage per handler:
- Simple handlers (list_pets): likely need only 16-32KB
- Complex handlers (with validation): may need 32-48KB
- Configure per-route via `x-brrtrouter-stack-size` (already supported!)

**Evidence from OpenAPI spec**:
```yaml
# From RouteMeta struct
x_brrtrouter_stack_size: Option<usize>  # ← Per-route stack size
```

**Status**: ⚠️ Moderate impact (memory overhead, not latency)

---

### 2.5 Middleware Chain Overhead ⚠️ LOW-MEDIUM IMPACT

**Architecture** (`src/dispatcher/core.rs:353-400`):

```rust
pub fn dispatch(&self, route_match: &RouteMatch, ...) -> Result<...> {
    // Build request
    let req = HandlerRequest { ... };
    
    // BEFORE middleware (sequential!)
    for mw in &self.middlewares {
        mw.before_handler(&req);  // ← Each middleware called sequentially
    }
    
    // Send to handler
    let response = /* ... */;
    
    // AFTER middleware (sequential!)
    for mw in &self.middlewares {
        mw.after_handler(&req, &response);  // ← Each middleware called sequentially
    }
    
    Ok(response)
}
```

**Current Middleware**:
1. MetricsMiddleware (DashMap writes) - ~5-10µs
2. MemoryMiddleware (atomic reads) - ~1-2µs  
3. AuthMiddleware (validation) - ~50-200µs (varies)
4. TracingMiddleware (span creation) - ~10-20µs

**Total Overhead**: 66-232µs per request

**Problem**:
- Sequential execution prevents optimization
- Auth middleware has high variance (JWT verification, network calls)
- No short-circuiting on failures

**Alternative**:
- Inline critical middleware (metrics, tracing) instead of dynamic dispatch
- Move auth validation earlier (before dispatch)
- Use middleware only for non-critical cross-cutting concerns

---

## 3. Component Performance Assessment

### 3.1 Router (Radix Tree) ✅ EXCELLENT

**Implementation**: `src/router/radix.rs`

**Performance Characteristics**:
- **Algorithm**: Radix tree with O(k) lookup (k = path length)
- **Benchmarks** (from `benches/throughput.rs`):
  - 10 routes: ~256 ns per lookup
  - 100 routes: ~411 ns per lookup  
  - 500 routes: ~990 ns per lookup
- **Scalability**: Flat performance curve (not O(n))

**Why It's Not a Bottleneck**:
- Sub-microsecond lookups even with 500 routes
- Minimal allocations (Arc for route metadata)
- Shared prefixes reduce memory footprint

**Evidence**:
```rust
// Performance characteristics documented in source
/// Based on benchmarks with the `criterion` crate:
/// - 10 routes: ~256 ns per lookup
/// - 100 routes: ~411 ns per lookup
/// - 500 routes: ~990 ns per lookup
```

**Status**: ✅ No optimization needed

---

### 3.2 Validator Cache ✅ EXCELLENT

**Implementation**: `src/validator_cache.rs`

**Performance Impact**:
- **Before**: Per-request JSONSchema compilation (~50-500µs)
- **After**: Cached lookup (~50ns) + Arc clone
- **Improvement**: 1000-10000x faster

**Cache Hit Rate**: ~100% after warmup (all schemas precompiled)

**Evidence from Documentation**:
```rust
/// ## Performance Impact
///
/// - **Eliminates**: Per-request JSONSchema::compile() calls
/// - **Reduces**: CPU usage by 20-40% under high load (measured in benchmarks)
/// - **Minimizes**: Memory allocations for schema validation
/// - **Startup Cost**: One-time compilation of all schemas (~1-10ms)
```

**Status**: ✅ No optimization needed (already optimal)

---

### 3.3 Serialization (serde_json) ⚠️ MODERATE IMPACT

**Current Usage**: `serde_json::to_string()` and `from_str()`

**Performance**:
- Small payloads (< 1KB): 5-20µs
- Medium payloads (1-10KB): 20-100µs
- Large payloads (> 10KB): 100-1000µs

**Alternative Libraries**:

| Library | Performance | Trade-off |
|---------|-------------|-----------|
| `serde_json` | Baseline | Safe, standard |
| `simd-json` | 2-3x faster | Requires unsafe, specific CPU features |
| `sonic-rs` | 1.5-2x faster | Less mature |
| `rkyv` | 5-10x faster | Zero-copy, but different API |

**Recommendation**:
- Keep `serde_json` for correctness and compatibility
- Consider `simd-json` as opt-in feature for performance-critical deployments
- Profile to confirm serialization is actually a bottleneck before changing

**Status**: ⚠️ Low priority (serde_json is "good enough" for 5k r/s)

---

### 3.4 Security Providers ⚠️ VARIABLE IMPACT

**Implementation**: `src/security.rs`

**Provider Types**:

#### API Key Validation (Fast)
```rust
// Simple hash map lookup - O(1)
if self.valid_keys.contains(provided_key) {
    return Ok(());
}
```
**Latency**: 1-10µs

#### JWT Validation (Medium)
```rust
// Signature verification + claims parsing
jsonwebtoken::decode::<Claims>(token, &decoding_key, &validation)?;
```
**Latency**: 50-200µs (depends on algorithm: HS256 < RS256)

#### JWKS Remote Validation (Slow)
```rust
// Network call to fetch public keys (cached)
let jwks = self.fetch_jwks(url).await?;
```
**Latency**: 100-500µs (if cached), 10-50ms (cold cache)

#### RemoteApiKeyProvider (Very Slow)
```rust
// HTTP call to verify API key
let response = self.http_client.post(verify_url)
    .json(&request)
    .send()?;
```
**Latency**: 50-200ms (network round-trip)

**Optimization Strategies**:
- ✅ Already implemented: Caching in JWKS and RemoteApiKey providers
- ⚠️ Room for improvement: Cache TTL configuration, LRU eviction

**Status**: ⚠️ Highly variable (depends on security scheme)

---

## 4. Identified Bottlenecks (Prioritized)

### Priority 1 (HIGH IMPACT) 🔴

#### 4.1 Single-Coroutine Handler Dispatch

**Problem**: One coroutine per handler = no parallelism under load

**Impact**: 
- Requests queue up in MPSC channel
- Tail latency increases linearly with load
- Can't utilize multiple cores for same handler

**Solution**: Use worker pools by default
- Switch from `register_handler()` to `register_handler_with_pool()`
- Configure `BRRTR_HANDLER_WORKERS=8` (or based on CPU cores)
- Update code generator templates to emit pool-based registration

**Estimated Improvement**: 2-8x throughput per handler (depends on cores)

**Implementation Cost**: Low (feature already exists, just needs to be default)

---

#### 4.2 Arc<RwLock<Router>> in Hot Path

**Problem**: Read lock acquisition on every request

**Impact**:
- Lock contention at high concurrency (100+ concurrent requests)
- P99 latency increases from 50µs → 100µs+ under contention

**Solution**: Make Router immutable during steady-state
- Use `Arc<Router>` instead of `Arc<RwLock<Router>>`
- For hot reload: swap entire Arc atomically via `Arc::swap()`
- Use `ArcSwap` crate for lock-free atomic pointer swapping

```rust
// Before
pub router: Arc<RwLock<Router>>,

// After
pub router: ArcSwap<Router>,  // Lock-free atomic swap

// Usage (hot path - no lock!)
let router = self.router.load();
let route_match = router.route(method, path);

// Hot reload (rare path - atomic swap)
self.router.store(Arc::new(new_router));
```

**Estimated Improvement**: 20-50µs reduction in P99 latency under load

**Implementation Cost**: Medium (requires refactoring hot reload logic)

---

#### 4.3 Per-Request HashMap Allocations

**Problem**: 4-6 HashMap allocations per request (path params, query params, headers, cookies)

**Impact**:
- 1-2 MB/sec allocation rate at 5k r/s
- GC pressure, cache eviction
- Cumulative latency: 10-30µs per request

**Solution**: Use pre-allocated or stack-based collections
- `SmallVec<[(String, String); 4]>` for common cases (≤4 items)
- `HashMap::with_capacity()` based on route metadata
- Reuse buffers via thread-local storage

**Estimated Improvement**: 10-30µs reduction in average latency

**Implementation Cost**: Medium (requires refactoring request types)

---

### Priority 2 (MEDIUM IMPACT) 🟡

#### 4.4 Middleware Chain Sequential Execution

**Problem**: Middleware runs sequentially, can't short-circuit

**Impact**:
- Auth failures still run through metrics/tracing middleware
- Total overhead: 66-232µs

**Solution**: Early exit on auth failures
- Validate auth BEFORE entering middleware chain
- Inline critical middleware (metrics, tracing) to avoid dynamic dispatch

**Estimated Improvement**: 50-100µs reduction on auth failures

**Implementation Cost**: Medium (requires restructuring request flow)

---

#### 4.5 String Cloning in Request Parsing

**Problem**: Multiple string allocations for headers, params

**Impact**: 1.5 MB/sec allocation rate at 5k r/s

**Solution**: Use string interning for known values
- `Cow<'static, str>` for header names (limited set)
- `Arc<str>` for shared strings

**Estimated Improvement**: 5-15µs reduction in average latency

**Implementation Cost**: Low-Medium (incremental refactoring)

---

### Priority 3 (LOW IMPACT) 🟢

#### 4.6 Serialization Library Choice

**Problem**: `serde_json` is not the fastest JSON library

**Impact**: 5-20µs per request for small payloads

**Solution**: Consider `simd-json` as opt-in feature

**Estimated Improvement**: 2-10µs reduction

**Implementation Cost**: Low (feature flag + conditional compilation)

---

#### 4.7 Coroutine Stack Size Optimization

**Problem**: 64KB default may be too large for simple handlers

**Impact**: 51MB virtual memory for 800 connections

**Solution**: Use per-route stack size configuration
- Already supported via `x-brrtrouter-stack-size`
- Profile actual usage and configure accordingly

**Estimated Improvement**: 10-30MB memory savings (not latency)

**Implementation Cost**: Low (already implemented, just needs profiling)

---

## 5. Non-Bottleneck Components ✅

These components are **already well-optimized** and should NOT be changed:

1. **Route Matching (Radix Tree)**: Sub-microsecond lookups, O(k) complexity ✅
2. **Validator Cache**: 1000x speedup vs. per-request compilation ✅
3. **Metrics Middleware (DashMap)**: Lock-free concurrent access ✅
4. **HTTP Parser (may_minihttp)**: C-level parsing, very fast ✅

---

## 6. Recommendations by Priority

### Immediate Actions (Can easily support 10k+ r/s)

1. **Switch to Worker Pools by Default** 🔴
   - Change code generator to emit `register_handler_with_pool()`
   - Set `BRRTR_HANDLER_WORKERS=8` (or CPU cores / 2)
   - **Impact**: 2-8x throughput improvement per handler

2. **Remove Router RwLock** 🔴
   - Replace `Arc<RwLock<Router>>` with `ArcSwap<Router>`
   - Lock-free atomic pointer swapping for hot reload
   - **Impact**: 20-50µs P99 latency reduction under load

3. **Pre-allocate HashMaps** 🔴
   - Use `HashMap::with_capacity()` based on route metadata
   - **Impact**: 10-30µs latency reduction

### Short-Term Optimizations (1-2 weeks)

4. **Optimize String Allocations** 🟡
   - Use `Cow<'static, str>` for known header names
   - Intern common strings
   - **Impact**: 5-15µs latency reduction

5. **Inline Critical Middleware** 🟡
   - Metrics and tracing directly in request path
   - Remove dynamic dispatch overhead
   - **Impact**: 10-30µs latency reduction

6. **Early Auth Validation** 🟡
   - Validate security before middleware chain
   - Short-circuit on auth failures
   - **Impact**: 50-100µs on failures

### Long-Term Considerations

7. **simd-json Feature Flag** 🟢
   - Opt-in for performance-critical deployments
   - **Impact**: 2-10µs serialization speedup

8. **Stack Size Profiling** 🟢
   - Use per-route stack size based on profiling
   - **Impact**: 10-30MB memory savings

---

## 7. Performance Testing Strategy

### Load Test Scenarios

#### Scenario 1: Baseline (Current State)
```bash
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  -u 100 -r 10 -t 2m \
  --header "X-API-Key: test123"
```
**Expected**: ~40k r/s, P99 < 1ms

#### Scenario 2: Target Load (5k sustained)
```bash
# Sustained 5k r/s for 10 minutes
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  -u 100 -r 20 -t 10m \
  --header "X-API-Key: test123"
```
**Expected**: 5k r/s, P99 < 500µs

#### Scenario 3: Spike Test (10k burst)
```bash
# Burst to 10k r/s for 1 minute
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  -u 200 -r 50 -t 1m \
  --header "X-API-Key: test123"
```
**Expected**: Handle burst without degradation

#### Scenario 4: Soak Test (5k for 1 hour)
```bash
# Memory leak detection
cargo run --release --example api_load_test -- \
  --host http://localhost:8080 \
  -u 100 -r 20 -t 60m \
  --header "X-API-Key: test123"
```
**Expected**: Stable memory usage, no leaks

### Metrics to Collect

1. **Throughput**: requests/second (target: 5000+)
2. **Latency**: P50, P95, P99, Max (target: P99 < 500µs)
3. **Error Rate**: 4xx and 5xx responses (target: < 0.1%)
4. **Memory Usage**: RSS, heap size (target: stable over time)
5. **CPU Usage**: per-core utilization (target: < 80% average)
6. **Lock Contention**: lock acquisition times (target: < 50µs P99)
7. **Queue Depth**: handler channel depth (target: < 100 queued)

### Flamegraph Profiling

```bash
# Run under profiler
just flamegraph

# Analyze hot paths
# Expected: <10% time in route matching, serialization
#          <20% time in handler dispatch
#          <30% time in actual handler logic
```

---

## 8. Summary of Findings

### Current State
- **Baseline Performance**: 40k r/s (8x target) ✅
- **Route Matching**: Optimized (radix tree) ✅
- **Validation**: Optimized (cached) ✅
- **Dispatch**: Single coroutine (bottleneck) ❌
- **Lock Usage**: RwLock in hot path (bottleneck) ❌
- **Memory**: Per-request allocations (inefficient) ⚠️

### Can BRRTRouter Sustain 5k r/s?

**Answer**: **YES**, with minor changes:

1. **Current baseline (40k r/s) suggests 5k should be easy**
2. **BUT**: Baseline likely tested with:
   - Minimal validation (no request body validation)
   - No authentication
   - No real business logic

3. **With full production config** (auth + validation + logging):
   - Current: ~5-10k r/s (estimated)
   - With optimizations: 10-20k r/s (estimated)

### Critical Path Forward

#### Must-Fix for 5k r/s (Priority 1) 🔴
1. Enable worker pools by default
2. Remove Router RwLock (use ArcSwap)
3. Pre-allocate HashMaps with capacity

#### Nice-to-Have for 10k+ r/s (Priority 2) 🟡
4. Optimize string allocations
5. Inline critical middleware
6. Early auth validation

#### Future Optimizations (Priority 3) 🟢
7. simd-json feature flag
8. Stack size profiling
9. Connection pooling for remote auth

---

## 9. Petstore-Specific Considerations

### Current Petstore Architecture

**Handler Registration** (from `examples/pet_store/src/registry.rs`):
```rust
// Generated by brrtrouter-gen
pub unsafe fn register_from_spec(
    dispatcher: &mut Dispatcher,
    routes: &[RouteMeta],
) {
    // Single-coroutine handlers (DEFAULT)
    dispatcher.register_handler("list_pets", handlers::list_pets::handle);
    dispatcher.register_handler("get_pet", handlers::get_pet::handle);
    // ... etc
}
```

**Issues**:
1. ❌ Single coroutine per handler (no parallelism)
2. ❌ No worker pool configuration
3. ❌ No per-route stack size optimization

### Petstore Optimization Checklist

#### Template Changes (brrtrouter-gen)
- [ ] Update `templates/registry.rs.j2` to emit `register_handler_with_pool()`
- [ ] Add worker pool configuration to generated code
- [ ] Respect `x-brrtrouter-stack-size` from OpenAPI spec

#### Example OpenAPI Updates
- [ ] Add `x-brrtrouter-stack-size: 32768` for simple handlers
- [ ] Add `x-brrtrouter-worker-count: 8` for high-traffic endpoints
- [ ] Document performance tuning in petstore README

#### Testing
- [ ] Benchmark petstore before/after worker pool changes
- [ ] Verify 5k r/s sustained performance
- [ ] Profile memory usage and stack consumption

---

## 10. Conclusion

BRRTRouter has a **solid architectural foundation** with several key optimizations already in place:
- ✅ Radix tree routing (O(k) lookup)
- ✅ Validator caching (1000x speedup)
- ✅ Lock-free metrics (DashMap)

However, **three critical bottlenecks** prevent sustained 5k r/s performance under production load:
1. 🔴 **Single-coroutine dispatch** (no parallelism)
2. 🔴 **Router RwLock** (lock contention)
3. 🔴 **Per-request allocations** (GC pressure)

**Good News**: All three bottlenecks have **existing solutions** in the codebase:
- Worker pools already implemented (`register_handler_with_pool`)
- ArcSwap pattern is well-known in Rust
- HashMap pre-allocation is straightforward

**Estimated Timeline**:
- **1 day**: Switch to worker pools by default (Priority 1.1)
- **2 days**: Remove Router RwLock (Priority 1.2)
- **1 day**: Pre-allocate HashMaps (Priority 1.3)
- **Testing**: 2-3 days validation across all changes

**Total**: **~1 week** to confidently sustain 5k r/s with full production features enabled.

---

**End of Analysis**

This document should be treated as a **living document** and updated as:
- Optimizations are implemented
- Benchmarks are run
- New bottlenecks are discovered
- Architecture evolves

For questions or discussion, refer to:
- [ARCHITECTURE.md](./ARCHITECTURE.md) - System architecture
- [PERFORMANCE_METRICS.md](./PERFORMANCE_METRICS.md) - Metrics collection
- [GOOSE_LOAD_TESTING.md](./GOOSE_LOAD_TESTING.md) - Load testing guide
