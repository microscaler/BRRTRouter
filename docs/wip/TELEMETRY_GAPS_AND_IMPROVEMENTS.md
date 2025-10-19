# BRRTRouter Telemetry Gaps and Improvements

## Date: October 19, 2025

## Executive Summary

Critical telemetry gaps have been identified in BRRTRouter, particularly around coroutine stack usage measurement and missing logging at key execution points. This document outlines the gaps and provides implementation recommendations.

## Critical Gap: Stack Usage Measurement

### Current State
- **Problem**: We report stack SIZE but not actual stack USAGE
- **Impact**: Cannot detect when handlers are close to stack overflow
- **Current Code**: Always reports `used = 0` because May doesn't expose usage

### Discovery
The test `test_metrics_stack_usage` uses an odd stack size (`0x801`) with a comment "set an odd stack size so may prints usage information". This suggests May has hidden functionality for stack measurement when using odd sizes.

### Solution: Enable Stack Usage Measurement

#### Option 1: Use Odd Stack Sizes in Production
```rust
// In runtime_config.rs
pub fn from_env() -> Self {
    let mut stack_size = // ... existing parsing logic
    
    // Make stack size odd to enable May's stack usage tracking
    if stack_size % 2 == 0 {
        stack_size += 1;
    }
    
    RuntimeConfig { stack_size }
}
```

#### Option 2: Stack Canary Pattern
Implement our own stack usage measurement using a canary pattern:
```rust
// In dispatcher when spawning coroutines
const STACK_CANARY: u64 = 0xDEADBEEFCAFEBABE;

// Fill stack with canary pattern at spawn time
// Later, scan from bottom to find first non-canary value
```

## Missing Telemetry Points

### 1. Coroutine Lifecycle
**Gap**: No telemetry when coroutines are created/destroyed
**Impact**: Cannot track coroutine leaks or spawning failures

**Add in `src/dispatcher/core.rs`:**
```rust
// After successful spawn
info!(
    handler_name = %name,
    stack_size = stack_size,
    total_handlers = self.handlers.len(),
    "Coroutine spawned successfully"
);

// When dropping old handler
warn!(
    handler_name = %route.handler_name,
    "Destroying old coroutine - handler replaced"
);
```

### 2. Router Performance
**Gap**: No metrics on route matching performance
**Impact**: Cannot identify slow regex patterns

**Add in `src/router/core.rs`:**
```rust
// Track route matching time
let match_start = Instant::now();
let result = self.route(method, path);
let match_duration = match_start.elapsed();

if match_duration > Duration::from_millis(1) {
    warn!(
        method = %method,
        path = %path,
        duration_us = match_duration.as_micros(),
        "Slow route matching detected"
    );
}
```

### 3. Security Provider Performance
**Gap**: No telemetry on authentication/authorization time
**Impact**: Cannot identify slow auth providers

**Add in `src/server/service.rs`:**
```rust
let auth_start = Instant::now();
let auth_result = provider.validate(&security_req);
let auth_duration = auth_start.elapsed();

info!(
    provider_name = %name,
    duration_ms = auth_duration.as_millis(),
    success = auth_result.is_ok(),
    "Security provider validation complete"
);
```

### 4. Channel Health
**Gap**: No metrics on channel queue depth
**Impact**: Cannot detect backpressure or handler overload

**Add metrics for:**
- Channel send failures
- Queue depth at send time
- Time waiting for response

### 5. Memory Allocation
**Gap**: No tracking of memory allocation per request
**Impact**: Cannot identify memory-intensive handlers

**Add tracking for:**
- Request body size
- Response body size
- Allocations during handler execution

## Template Telemetry Gaps

### 1. Startup Telemetry
**File**: `templates/main.rs.txt`

**Current**: Limited startup logging
**Needed**: Comprehensive startup telemetry

```rust
info!(
    stack_size = config.stack_size,
    routes_count = routes.len(),
    handlers_count = dispatcher.handlers.len(),
    "Service starting with configuration"
);
```

### 2. Configuration Validation
**File**: `templates/main.rs.txt`

**Gap**: No telemetry when config is loaded
**Add**:
```rust
info!(
    config_path = %args.config.display(),
    has_static_dir = args.static_dir.is_some(),
    has_doc_dir = args.doc_dir.is_some(),
    keep_alive = app_config.http.keep_alive.unwrap_or(true),
    "Configuration loaded successfully"
);
```

### 3. Handler Registration
**File**: `templates/registry.rs.txt`

**Gap**: No telemetry during handler registration
**Add**:
```rust
debug!(
    handler_name = "{{ entry.name }}",
    controller = "{{ entry.controller_struct }}",
    "Registering typed handler"
);
```

## Implementation Priority

### Phase 1: Critical (Immediate)
1. **Stack Usage Measurement** - Implement odd stack size trick
2. **Coroutine Lifecycle Logging** - Add spawn/destroy telemetry
3. **Memory Leak Detection** - Add handler count metrics

### Phase 2: Important (This Week)
1. **Security Provider Metrics** - Add auth timing
2. **Route Matching Performance** - Add slow route warnings
3. **Channel Health Metrics** - Add queue depth tracking

### Phase 3: Nice to Have (Next Sprint)
1. **Memory Allocation Tracking** - Per-request memory metrics
2. **Template Telemetry** - Enhanced startup logging
3. **Request Tracing** - Full request lifecycle tracing

## Metrics to Add

### Prometheus Metrics
```prometheus
# Coroutine metrics
brrtrouter_coroutines_active{handler="name"} gauge
brrtrouter_coroutines_spawned_total counter
brrtrouter_coroutines_destroyed_total counter
brrtrouter_coroutine_stack_used_percent gauge

# Channel metrics
brrtrouter_channel_queue_depth{handler="name"} gauge
brrtrouter_channel_send_failures_total{handler="name"} counter

# Security metrics
brrtrouter_auth_duration_seconds{provider="name"} histogram
brrtrouter_auth_failures_total{provider="name", reason="reason"} counter

# Route matching metrics
brrtrouter_route_match_duration_seconds histogram
brrtrouter_route_match_failures_total counter
```

## Testing Recommendations

1. **Load Test with Telemetry**: Run extended load tests monitoring all new metrics
2. **Stack Usage Validation**: Verify odd stack sizes enable usage tracking
3. **Memory Profiling**: Use valgrind/heaptrack to validate memory metrics
4. **Telemetry Overhead**: Measure performance impact of new telemetry

## Conclusion

BRRTRouter has solid basic telemetry but lacks critical visibility into:
- Actual stack usage (not just allocation)
- Coroutine lifecycle events
- Channel health and backpressure
- Security provider performance
- Memory allocation patterns

Implementing these improvements will provide the observability needed to operate BRRTRouter in production with confidence.
