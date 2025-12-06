# JSF Hot Path Allocation Audit

**Date**: December 2025  
**Branch**: `JFS-implementation-three`  
**Status**: In Progress  

This document catalogs all heap allocations in the BRRTRouter request hot path, prioritized for optimization in future JSF iterations.

---

## Executive Summary

| Module | Hot Path Allocations | Priority | Estimated Effort |
|--------|---------------------|----------|------------------|
| `router/radix.rs` | 2 per param | **P0** | Medium |
| `dispatcher/core.rs` | 3 per request | **P0** | High |
| `server/request.rs` | 8+ per request | **P1** | Medium |
| `server/service.rs` | 5+ per request | **P1** | Medium |
| `security.rs` | 1-2 per request | **P2** | Low |

**Current Benchmark**: ~1.64µs per route match (radix tree only)

---

## Module-by-Module Analysis

### 1. Router Module (`src/router/radix.rs`)

#### Current State (Phase 3 Complete - P0-1 DONE ✅)
- ✅ Uses `SmallVec<[&str; 16]>` for path segments (avoids Vec allocation)
- ✅ Uses `SmallVec<[(Arc<str>, String); 8]>` for params (ParamVec) - **P0-1 COMPLETE**
- ✅ `#[inline]` hints on hot methods
- ✅ `param_name` stored as `Arc<str>` in `RadixNode` - **P0-1 COMPLETE**

#### Remaining Allocations

| Line | Code | Allocation Type | Per-Request? | Status |
|------|------|-----------------|--------------|--------|
| 208 | `Arc::clone(param_name)` | Atomic increment | Yes (per param) | ✅ **OPTIMIZED** - O(1) |
| 208 | `segment.to_string()` | String clone | Yes (per param) | Required (request data) |

#### Completed Fix (jsf3-9) - **MERGED**
```rust
// Before:
pub type ParamVec = SmallVec<[(String, String); 8]>;

// After (P0-1):
pub type ParamVec = SmallVec<[(Arc<str>, String); 8]>;
```

**Impact**: Param name cloning now O(1) atomic increment instead of O(n) string copy. All 198+ tests passing.

#### Performance Validation (2000 concurrent users, 60s)

| Metric | Before P0-1 | After P0-1 | Improvement |
|--------|-------------|------------|-------------|
| **Throughput** | 67k req/s | **72.8k req/s** | **+8.7%** |
| **p50 latency** | 22ms | **20ms** | **-9%** |
| **p75 latency** | 34ms | **31ms** | **-9%** |
| **p98 latency** | 63ms | **55ms** | **-13%** |
| **p99 latency** | 63ms | **61ms** | **-3%** |
| p99.9 latency | - | 150ms | - |
| Max latency | 400ms | 456ms | - |
| Total requests | 3.15M | **7.07M** | +124% |
| Failures | 0% | 0% | ✅ |

**Key findings**:
- ~9% throughput improvement from O(1) param name cloning
- Consistent latency improvements across all percentiles
- Zero failures maintained under load

---

### 2. Router Core (`src/router/core.rs`)

#### Current State
- ✅ Uses `ParamVec` (SmallVec) for path/query params
- ✅ Uses `Arc<RouteMeta>` for route metadata (shared)

#### Remaining Allocations

| Line | Code | Allocation Type | Per-Request? | Fix |
|------|------|-----------------|--------------|-----|
| 283 | `handler_name.clone()` | String clone | Yes | Use `Arc<str>` in RouteMeta |

**Recommendation**: Store `handler_name` as `Arc<str>` in `RouteMeta`.

---

### 3. Dispatcher (`src/dispatcher/core.rs`)

#### Hot Path: `dispatch_with_request_id()`

| Line | Code | Allocation Type | Per-Request? | Fix |
|------|------|-----------------|--------------|-----|
| 555 | `Vec<&String>` | Vec allocation | Only on error | OK (error path) |
| 566 | `request_id.parse()` | Parse allocation | Yes | Pre-validate format |
| 567 | `method.clone()` | Enum copy | Yes | Cheap (enum) |
| 568 | `path_pattern.clone()` | String clone | Yes | Use `Arc<str>` |
| 620/637 | `request.clone()` | Full struct clone | Yes | **Required** (channel ownership) |

#### Why `request.clone()` is Required

```
dispatch() -> send(request) -> handler coroutine
          └─> use request for:
              - Logging (handler_name, method, path)
              - Middleware after()
              - Error messages
```

The channel takes ownership, but we need the request afterward for middleware and logging.

#### Recommended Fixes

1. **Store `path_pattern` as `Arc<str>`** (jsf3-10)
   - In `RouteMeta`, change `path_pattern: String` to `path_pattern: Arc<str>`
   - Makes clone O(1)

2. **Consider extracting needed fields before clone**
   - Extract `handler_name`, `method` before send
   - Pass extracted fields to middleware instead of full request
   - **High effort**: Requires middleware interface change

---

### 4. Server Request Parsing (`src/server/request.rs`)

#### Hot Path: `parse_request()`

| Line | Code | Allocation Type | Per-Request? | Fix |
|------|------|-----------------|--------------|-----|
| 87-88 | Cookie name/value `.to_string()` | 2× String | Yes (per cookie) | Consider `Cow` |
| 115 | Query param `(k.to_string(), v.to_string())` | 2× String | Yes (per param) | Consider `Cow` |
| 210 | `method.to_string()` | String | Yes | Use `Method` enum |
| 211 | `raw_path.to_string()` | String | Yes | Required |
| 212 | `path.to_string()` | String | Yes | Required |
| 213 | `format!("{:?}", version)` | String | Yes | Use static str |
| 222 | Header value `to_string()` | String | Yes (per header) | Consider `Cow` |

#### Recommended Fixes

1. **Use `http::Method` enum instead of String**
   - Already available, avoids string allocation
   - `ParsedRequest.method` should be `Method` not `String`

2. **HTTP version as static str**
   ```rust
   // Current:
   let http_version = format!("{:?}", req.version());
   
   // Better:
   let http_version: &'static str = match req.version() {
       Version::HTTP_10 => "HTTP/1.0",
       Version::HTTP_11 => "HTTP/1.1",
       _ => "HTTP/1.x",
   };
   ```

3. **Consider `Cow<'a, str>` for headers/cookies**
   - Only allocate when modification needed
   - Borrows from request buffer otherwise

---

### 5. Server Service (`src/server/service.rs`)

#### Hot Path: `HttpService::call()`

| Line | Code | Allocation Type | Per-Request? | Fix |
|------|------|-----------------|--------------|-----|
| 782 | `method.clone()` | String clone | Yes | Use `Method` enum |
| 783 | `path.clone()` | String clone | Yes | Consider `Cow` |
| 852 | `format!("Content-Type: {ct}")` | String + Box | Yes (conditional) | Pre-intern |
| 883 | `format!("Invalid HTTP method...")` | String | Error only | OK |
| 892 | `query_params.clone()` | ParamVec clone | Yes | Required (stored in RouteMatch) |
| 908 | `keys().cloned()` | Vec<String> | Yes (per security) | Consider SmallVec |

#### Metrics Endpoint (Not Hot Path)
Lines 381-561 contain many `format!()` calls for Prometheus output. These are acceptable as the metrics endpoint is not request-critical.

#### Recommended Fixes

1. **Pre-intern Content-Type headers**
   ```rust
   static CT_JSON: &str = "Content-Type: application/json";
   static CT_HTML: &str = "Content-Type: text/html";
   // Use static references instead of format!
   ```

2. **Use SmallVec for security scheme collection**
   ```rust
   let schemes_required: SmallVec<[&String; 4]> = ...
   ```

---

### 6. Security (`src/security.rs`)

#### Hot Path: `validate()` methods

| Line | Code | Allocation Type | Per-Request? | Fix |
|------|------|-----------------|--------------|-----|
| 564 | JWKS key `.cloned()` | DecodingKey clone | Per JWT validation | Cache decoded keys |
| 881 | `key.to_string()` | String | Per API key lookup | Use Arc<str> |

#### Recommended Fixes

1. **Cache decoded JWT claims**
   - If same token seen within TTL, skip decode

2. **Use `Arc<str>` for API key storage**
   - Keys are long-lived, `Arc::clone()` is O(1)

---

## Priority Implementation Plan

### Iteration 4: String Interning (P0) ✅ COMPLETE

**Goal**: Reduce per-request string cloning by 60%+

1. ✅ Change `ParamVec` to use `Arc<str>` for names (P0-1, commit 1788e44)
2. ✅ Store `path_pattern` as `Arc<str>` in `RouteMeta` (P0-2, commit da8d865)
3. ✅ Store `handler_name` as `Arc<str>` in `RouteMeta` (P0-2, commit da8d865)

**Actual Impact** (P0-1 @ 2000 users):
| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| Throughput | 67.7k/s | 72.8k/s | +7.5% |
| p50 Latency | 22ms | 20ms | -9.1% |
| p99 Latency | 63ms | 61ms | -3.2% |

**Estimated Combined Impact (P0-1 + P0-2)**: -200-400ns per request ✅

### Iteration 5: Request Parsing (P1)

**Goal**: Reduce allocations in request parsing

1. Use `Method` enum instead of String
2. Use static strings for HTTP version
3. Consider `Cow` for headers (requires lifetime analysis)

**Estimated Impact**: -100-200ns per request

### Iteration 6: Advanced Optimizations (P2)

**Goal**: Eliminate remaining allocations

1. Pre-intern Content-Type headers
2. Use SmallVec for security collections
3. Consider arena allocation for per-request data

**Estimated Impact**: -50-100ns per request

---

## Measurement Methodology

### Benchmarks to Run

```bash
# Radix router microbenchmark
cargo bench --bench throughput -- route_match

# Full request benchmark (with Goose)
cargo run --release --example api_load_test -- --host http://127.0.0.1:8080 --users 100 --run-time 30s

# Memory profiling
cargo instruments -t "Allocations" --bin pet_store
```

### Metrics to Track

| Metric | Baseline | After P0-1 | After P0-2 | Target |
|--------|----------|------------|------------|--------|
| route_match latency | 1.64µs | TBD | TBD | <1.2µs |
| Goose p50 latency (2k users) | 22ms | **20ms** ✅ | TBD | <20ms |
| Goose p99 latency (2k users) | 63ms | **61ms** ✅ | TBD | <60ms |
| Throughput (2k users) | 67k/s | **72.8k/s** ✅ | TBD | >75k/s |
| Allocations per request | ~15-20 | ~14-19 | ~12-17 (est) | <10 |

---

## Appendix: Allocation Inventory

### Per-Request Allocations (Current)

| Source | Count | Size (est.) |
|--------|-------|-------------|
| Path params (names) | 0-4 | ~40 bytes each |
| Path params (values) | 0-4 | ~20 bytes each |
| Query params | 0-8 | ~30 bytes each |
| Headers | 10-20 | ~50 bytes each |
| Cookies | 0-5 | ~30 bytes each |
| handler_name clone | 1 | ~20 bytes |
| path_pattern clone | 1 | ~30 bytes |
| HandlerRequest clone | 1 | ~500 bytes |

**Total estimated**: 1.5-3KB of allocations per request

### JSF Rule 206 Compliance

Current compliance: **High** (after P0-2)

- ✅ SmallVec used for params (stack-allocated for ≤8)
- ✅ SmallVec used for headers (stack-allocated for ≤16)
- ✅ Arc used for shared RouteMeta
- ✅ Arc<str> for param names (P0-1) - eliminates ~40 bytes/param
- ✅ Arc<str> for path_pattern (P0-2) - eliminates ~30 bytes/request
- ✅ Arc<str> for handler_name (P0-2) - eliminates ~20 bytes/request
- ⚠️ Full HandlerRequest clone for channel send (required for ownership)
- ⚠️ String allocations in request parsing (P1 target)

Target: **Full compliance** after Iteration 6

---

## References

- [JSF AV Rules](https://www.stroustrup.com/JSF-AV-rules.pdf) - Rule 206: No heap after init
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Criterion Benchmarking](https://bheisler.github.io/criterion.rs/book/)

