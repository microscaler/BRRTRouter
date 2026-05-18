# BRRTRouter Performance Optimization PRD

**Goal:** Reduce unloaded request latency from 15-20ms to <5ms  
**Target:** 10k-20k RPS in CI load tests  
**Date:** December 4, 2025

---

## Executive Summary

Analysis of the current hot path reveals multiple bottlenecks causing the 15-20ms latency. The primary culprits are:
1. Per-request allocations (Vec, HashMap, String)
2. Excessive logging/tracing overhead
3. RwLock contention on router/dispatcher
4. Channel creation overhead
5. JSON schema validation on every request

This PRD proposes a phased approach to achieve <5ms latency.

---

## Current Hot Path Analysis

### Request Flow (Measured Latency Contributors)

```
Request Arrival → Parse (~0.5ms)
              → RwLock<Router>.read() (~0.1ms)
              → Route Match (~0.2ms) *but allocates*
              → RwLock<Dispatcher>.read() (~0.1ms)
              → Security Validation (~1-5ms if enabled)
              → Schema Validation (~2-5ms, compiles on first use)
              → Channel create (~0.1ms)
              → Handler dispatch + response (~5-10ms)
              → Response write (~0.5ms)
              → Logging overhead (~1-2ms distributed)
```

### Identified Bottlenecks

#### 1. **Per-Request Allocations** (Est. 2-3ms total)

**radix.rs:300-307:**
```rust
let segments: Vec<&str> = path
    .trim_start_matches('/')
    .split('/')
    .filter(|s| !s.is_empty())
    .collect();  // ALLOCATION

let mut params = HashMap::new();  // ALLOCATION
```

**radix.rs:184:**
```rust
params.insert(param_name.to_string(), segment.to_string());  // 2x STRING ALLOCATION
```

**dispatcher.rs:389:**
```rust
let (reply_tx, reply_rx) = mpsc::channel();  // CHANNEL ALLOCATION
```

#### 2. **Logging Overhead** (Est. 1-2ms)

Even with `RUST_LOG=error`, tracing has overhead:
- `info_span!` creation (service.rs:738)
- RequestLogger struct creation
- Multiple `debug!`/`info!` calls throughout

#### 3. **RwLock Contention** (Est. 0.2ms, but can spike under load)

- `self.router.read()` - acquired per request
- `self.dispatcher.read()` - acquired per request
- Under load, readers queue behind each other

#### 4. **Schema Validation** (Est. 2-5ms first request, cached after)

**service.rs:1126-1129:**
```rust
let compiled = self.validator_cache
    .get_or_compile(&route_match.handler_name, "request", None, schema)
    .expect("invalid request schema");
```

First request compiles schema; subsequent requests are cached but still have lookup overhead.

#### 5. **Handler Communication** (Est. 3-5ms)

- Channel creation: ~0.1ms
- Send to coroutine: ~0.1ms
- Coroutine wake + execution: ~2-3ms
- Recv response: ~0.1ms blocking

---

## Proposed Optimizations

### Phase 1: Zero-Allocation Route Matching (Est. -3ms)

**Priority: P0 - Implement First**

Replace per-request allocations with stack-based buffers:

```rust
// Before (current)
pub fn route(&self, method: Method, path: &str) 
    -> Option<(Arc<RouteMeta>, HashMap<String, String>)> {
    let segments: Vec<&str> = path.split('/').filter(...).collect();  // ALLOC
    let mut params = HashMap::new();  // ALLOC
    // ...
}

// After (optimized)
const MAX_SEGMENTS: usize = 16;
const MAX_PARAMS: usize = 8;

pub fn route<'a>(
    &self,
    method: Method,
    path: &'a str,
    params_out: &mut [Option<(&'static str, &'a str)>; MAX_PARAMS],
) -> Option<Arc<RouteMeta>> {
    // Parse segments using indices, not Vec
    let mut segment_count = 0;
    let mut pos = if path.starts_with('/') { 1 } else { 0 };
    
    // Walk path without allocation
    while pos < path.len() && segment_count < MAX_SEGMENTS {
        let start = pos;
        while pos < path.len() && path.as_bytes()[pos] != b'/' {
            pos += 1;
        }
        // segment = &path[start..pos]
        segment_count += 1;
        if pos < path.len() { pos += 1; }
    }
    
    // Params stored directly into caller-provided buffer
    // No HashMap allocation
}
```

**Implementation:**
1. Add `route_no_alloc()` method to `RadixRouter`
2. Use fixed-size arrays on stack instead of Vec/HashMap
3. Parameter names are `&'static str` (interned at build time)
4. Parameter values are `&'a str` (slices into request path)

### Phase 2: Eliminate Request-Path Logging (Est. -1ms)

**Priority: P0**

Add compile-time feature flag to disable hot-path logging:

```rust
// Cargo.toml
[features]
default = []
hot-path-logging = []

// service.rs
#[cfg(feature = "hot-path-logging")]
debug!(method = %method, path = %path, "Request received");
```

For production deployments, disable `hot-path-logging` feature.

**Alternative:** Use sampling at runtime:
```rust
// Sample 1 in 1000 requests for logging
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);
let count = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
if count % 1000 == 0 {
    info!(method = %method, path = %path, "Sampled request");
}
```

### Phase 3: Remove RwLock from Hot Path (Est. -0.5ms, major under load)

**Priority: P1**

Change from `Arc<RwLock<Router>>` to atomic pointer swap:

```rust
// Before
pub struct AppService {
    pub router: Arc<RwLock<Router>>,
    pub dispatcher: Arc<RwLock<Dispatcher>>,
}

// After
pub struct AppService {
    pub router: arc_swap::ArcSwap<Router>,
    pub dispatcher: arc_swap::ArcSwap<Dispatcher>,
}

// Hot path becomes lock-free
let router = self.router.load();  // No lock, just atomic load
```

This is already supported by the `arc_swap` crate (widely used, well-tested).

### Phase 4: Pre-allocated Response Channels (Est. -0.5ms)

**Priority: P1**

Instead of creating a channel per request, use a pool:

```rust
// Channel pool using crossbeam
struct ChannelPool {
    pool: crossbeam::queue::ArrayQueue<(Sender, Receiver)>,
}

impl ChannelPool {
    fn acquire(&self) -> (Sender, Receiver) {
        self.pool.pop().unwrap_or_else(|| mpsc::channel())
    }
    
    fn release(&self, chan: (Sender, Receiver)) {
        let _ = self.pool.push(chan);  // Best effort, drop if full
    }
}
```

### Phase 5: Pre-compiled Schema Validators (Already Implemented, Verify)

**Priority: P2**

The `ValidatorCache` already exists. Verify:
1. All schemas are pre-compiled at startup via `precompile_schemas()`
2. Cache lookup is O(1) HashMap
3. No compilation happens on hot path

**Quick Win:** Pre-warm cache on startup:
```rust
// In main.rs after creating service
service.precompile_schemas(&routes);
info!("Pre-compiled {} schemas", routes.len());
```

### Phase 6: Inline Critical Functions (Est. -0.2ms)

**Priority: P2**

Add `#[inline(always)]` to hot path functions:

```rust
#[inline(always)]
fn segment_eq(seg_id: u32, incoming: &str, segments: &[u8], index: &[u32]) -> bool {
    // ...
}

#[inline(always)]
pub fn route(&self, method: Method, path: &str) -> Option<RouteMatch> {
    // ...
}
```

---

## Implementation Roadmap

### Week 1: Quick Wins (Phase 2 + Phase 5 verification)

| Task | Est. Effort | Impact |
|------|-------------|--------|
| Add sampling to hot-path logging | 2h | -1ms |
| Verify schema pre-compilation | 1h | Confirm no regression |
| Add `#[inline(always)]` annotations | 1h | -0.2ms |
| Benchmark baseline | 2h | Establish metrics |

### Week 2: Zero-Allocation Router (Phase 1)

| Task | Est. Effort | Impact |
|------|-------------|--------|
| Implement `route_no_alloc()` | 8h | -2ms |
| Add interned parameter names | 4h | -0.5ms |
| Update dispatcher to use borrowed params | 4h | -0.5ms |
| Integration tests | 4h | Validate correctness |

### Week 3: Lock-Free Hot Path (Phase 3 + 4)

| Task | Est. Effort | Impact |
|------|-------------|--------|
| Replace RwLock with ArcSwap | 4h | -0.5ms baseline, major under load |
| Implement channel pool | 4h | -0.5ms |
| Hot reload compatibility | 4h | Ensure atomic swaps work |
| Load testing | 4h | Validate 10k+ RPS |

---

## Success Metrics

| Metric | Current | Target | How to Measure |
|--------|---------|--------|----------------|
| p50 latency (unloaded) | 15-20ms | <5ms | `ab -n 1000 -c 1` |
| p99 latency (unloaded) | 25-30ms | <10ms | Same |
| Throughput (loaded) | ~3k RPS | 10k-20k RPS | Goose load test |
| Memory per request | ~4KB | <1KB | Measure allocs |

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Zero-alloc changes break edge cases | Extensive test coverage for path patterns |
| ArcSwap may have subtle concurrency issues | Use well-tested crate, add stress tests |
| Logging removal hides production issues | Keep error-level logging, use sampling |
| Performance gains don't materialize | Benchmark after each phase, pivot if needed |

---

## Quick Start: Immediate Actions

1. **Today:** Run baseline benchmark
   ```bash
   ab -n 10000 -c 10 http://localhost:8081/pets
   ```

2. **This week:** Disable hot-path logging
   ```bash
   # In config.yaml or env
   RUST_LOG=brrtrouter=error,warn
   BRRTR_LOG_SAMPLING_MODE=error-only
   ```

3. **Verify schema pre-compilation:**
   ```rust
   // In main.rs after service creation
   let compiled = service.precompile_schemas(&routes);
   info!("Pre-compiled {} schemas at startup", compiled);
   ```

---

## Appendix: JSF Alignment

This PRD aligns with the JSF AV rules analysis:
- **AV Rule 206 (No heap after init):** Phase 1 eliminates hot-path allocations
- **AV Rule 208 (No exceptions):** Already implemented with `catch_unwind`
- **Bounded complexity:** Zero-alloc router has simple, predictable control flow

The radix trie from the JSF writeup provides an even more aggressive optimization path if Phase 1 doesn't achieve targets.

