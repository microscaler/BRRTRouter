# Dogfood Branch Performance Regression Analysis

**Date:** December 4, 2025  
**Impact:** 42-44x performance degradation  
**Priority:** P0 - Critical - Must fix before JSF optimizations

---

## Executive Summary

The dogfood branch has catastrophic performance degradation compared to main:

| Metric | Main Branch | Dogfood Branch | Degradation |
|--------|-------------|----------------|-------------|
| **Scenarios/s** | 1,656 | 39.32 | **42x slower** |
| **Avg latency** | 11.36ms | 498.73ms | **44x slower** |
| **p50 latency** | 2ms | 130ms | **65x slower** |
| **p99 latency** | 49ms | 700ms | **14x slower** |
| **Total requests (2min)** | 621,690 | 14,193 | **44x fewer** |

---

## Root Cause Analysis

### 🔴 CAUSE #1: Debug Logging Enabled (Est. 300-400ms overhead)

**Location:** `k8s/app/deployment.yaml`

**Dogfood (SLOW):**
```yaml
- name: RUST_LOG
  value: "debug"
- name: BRRTR_LOG_SAMPLING_MODE
  value: "all"  # Logs EVERY request
```

**Main (FAST):**
```yaml
- name: RUST_LOG
  value: "error"
- name: BRRTR_LOG_SAMPLING_MODE
  value: "error-only"  # Only logs errors
- name: BRRTR_LOG_LEVEL
  value: "error"
```

**Impact:** 
- `debug` logging with `"all"` sampling mode logs EVERY request to disk/Loki
- JSON serialization overhead per log line
- Disk I/O blocking on log writes
- Network I/O if Loki/OTLP is enabled

**Fix Priority:** P0 - Immediate

---

### 🔴 CAUSE #2: Worker Pool Configuration Missing (Est. 100-200ms overhead)

**Location:** `k8s/app/deployment.yaml`

**Dogfood (SLOW):**
```yaml
# No BRRTR_HANDLER_WORKERS setting - defaults to 4 workers
```

**Main (FAST):**
```yaml
- name: BRRTR_HANDLER_WORKERS
  value: "1"  # Optimized for single-handler performance testing
```

**Impact:**
- 4 workers per handler × 22 handlers = 88 worker coroutines
- Main uses 1 worker per handler = 22 worker coroutines
- 4x more context switching overhead
- 4x more memory pressure
- Contention on channel send/receive

**Fix Priority:** P0 - Immediate

---

### 🟡 CAUSE #3: Request Cloning in Handler Loop

**Location:** `src/typed/core.rs` line ~170 (dogfood) vs main

**Dogfood (SLOW):**
```rust
let data = match H::Request::try_from(req.clone()) {  // CLONES ENTIRE REQUEST
    Ok(v) => v,
    ...
};

let typed_req = TypedHandlerRequest {
    method: req.method,       // Uses req AGAIN after clone
    path: req.path,
    handler_name: req.handler_name,
    path_params: req.path_params,
    query_params: req.query_params,
    data,
};
```

**Main (FAST):**
```rust
// Extract metadata fields BEFORE consuming req
let method = req.method.clone();
let path = req.path.clone();
let handler_name = req.handler_name.clone();
let path_params = req.path_params.clone();
let query_params = req.query_params.clone();

// CONSUME req - no clone needed
let data = match H::Request::try_from(req) {
    Ok(v) => v,
    ...
};
```

**Impact:**
- `req.clone()` copies the entire request including:
  - HTTP method, path, headers (~500 bytes)
  - `body: Option<serde_json::Value>` (can be KB to MB for POST requests)
  - `path_params: HashMap<String, String>` (heap allocations)
  - `query_params: HashMap<String, String>` (heap allocations)
- Main branch consumes `req` directly after extracting metadata
- Per-request allocation overhead adds up at high RPS

**Fix Priority:** P1 - This Week

---

### 🟡 CAUSE #4: Closure Structure Inefficiency

**Location:** `src/typed/core.rs`

**Dogfood (SLOW):**
```rust
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let reply_tx_inner = reply_tx.clone();
    // ... uses req.clone() inside
}));
```

**Main (FAST):**
```rust
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe({
    let reply_tx_outer = reply_tx_outer.clone();
    let handler = &handler;  // BORROW, don't clone
    move || {
        // ... consumes req directly
    }
}));
```

**Impact:**
- Main branch uses a move closure with borrows for handler
- Main branch pre-extracts metadata before the closure
- Dogfood creates closure and clones inside, causing extra allocations

**Fix Priority:** P1 - This Week

---

## Immediate Fixes Required

### Fix #1: Restore Performance-Optimized Logging Config

```yaml
# k8s/app/deployment.yaml
- name: RUST_LOG
  value: "error"
- name: BRRTR_LOG_SAMPLING_MODE
  value: "error-only"
- name: BRRTR_LOG_LEVEL
  value: "error"
```

### Fix #2: Restore Worker Pool Tuning

```yaml
# k8s/app/deployment.yaml
- name: BRRTR_HANDLER_WORKERS
  value: "1"  # For performance testing, increase for production
```

### Fix #3: Cherry-pick Request Optimization from Main

```bash
# The optimization is in main's typed/core.rs
# Need to update dogfood branch to consume req directly instead of cloning
```

---

## Estimated Impact After Fixes

| Fix | Est. Improvement |
|-----|------------------|
| Logging → error-only | -300ms (from 500ms to 200ms) |
| Worker pool → 1 | -50ms (from 200ms to 150ms) |
| Remove req.clone() | -30ms (from 150ms to 120ms) |
| **Total** | **~120ms target (vs 500ms current)** |

**Expected Result:** p50 latency should drop from 130ms to ~20-30ms, putting us back in range of main branch performance.

---

## Action Items

| # | Task | Owner | ETA | Status |
|---|------|-------|-----|--------|
| 1 | Update deployment.yaml logging config | - | Today | ✅ DONE |
| 2 | Add BRRTR_HANDLER_WORKERS=1 | - | Today | ✅ DONE |
| 3 | Sync typed/core.rs request consumption optimization | - | Today | ✅ DONE |
| 4 | Re-run Goose benchmark | - | Today | ⬜ |
| 5 | Validate p50 < 30ms | - | Today | ⬜ |

## Changes Made (December 4, 2025)

### 1. k8s/app/deployment.yaml
- Changed `RUST_LOG` from `debug` to `error`
- Changed `BRRTR_LOG_SAMPLING_MODE` from `all` to `error-only`
- Added `BRRTR_LOG_LEVEL=error`
- Added `BRRTR_HANDLER_WORKERS=1`

### 2. src/typed/core.rs
- Fixed `spawn_typed` function to avoid `req.clone()` in the hot path
- Now extracts metadata fields (method, path, handler_name, path_params, query_params) BEFORE consuming `req`
- Uses optimized move closure pattern that consumes `req` directly in `H::Request::try_from(req)`
- Eliminates per-request heap allocations for the entire HandlerRequest

---

## Notes

- The dogfood branch was configured for **observability/debugging** (full logging, multiple workers)
- Main branch was configured for **performance testing** (minimal logging, single worker)
- These are **configuration differences**, not fundamental architectural issues
- After fixing these, we can proceed with JSF optimizations for the sub-5ms target

---

## JSF Safety Compliance - Lint Configuration Added

Added `[lints]` section to `Cargo.toml` to enforce JSF AV safety rules:

```toml
[lints.clippy]
unwrap_used = "warn"     # 172 instances to fix
expect_used = "warn"     # prefer ? operator
panic = "warn"           # explicit panic!() calls
unreachable = "warn"     # unreachable!() panics at runtime
```

**Current Status:** `warn` (to avoid breaking CI)  
**Target Status:** `deny` (after cleanup)

**Cleanup Scope:**
| File | Count |
|------|-------|
| src/router/radix.rs | 38 |
| src/middleware/metrics.rs | 16 |
| src/generator/stack_size.rs | 14 |
| src/router/tests.rs | 13 |
| src/server/response.rs | 10 |
| Other files | 81 |
| **Total** | **172** |

