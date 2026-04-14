# BRRTRouter Crash and Connection Drop Fix Summary

## Date: October 19, 2025

## Issues Identified

### 1. **Coroutine Spawn Failures (CRITICAL)**
- **Location**: `src/dispatcher/core.rs:199` and `src/typed/core.rs:135`
- **Problem**: Both files used `.unwrap()` on coroutine spawn operations
- **Impact**: When the system ran out of resources (memory, threads) or stack allocation failed, the entire application would panic and crash
- **Root Cause**: No error handling for coroutine spawn failures

### 2. **Stack Overflow Risk**
- **Default Stack Size**: Only 16KB (0x4000 bytes)
- **Problem**: Complex handlers with deep call chains or large local variables would overflow this small stack
- **Impact**: Immediate crash without proper error handling
- **Symptoms**: Random crashes under load, especially with complex request processing

### 3. **Missing Response Handling**
- **Problem**: When handlers crashed or failed to send responses, the dispatcher would wait indefinitely
- **Impact**: Connection timeouts and dropped connections
- **User Experience**: Clients would see connection drops instead of proper error responses

## Fixes Applied

### 1. **Graceful Coroutine Spawn Failure Handling**
- Added error handling for coroutine spawn operations
- In `dispatcher/core.rs`: Log error and skip handler registration if spawn fails
- In `typed/core.rs`: Panic with informative message (since function can't return error)
- Prevents cascading failures when resources are exhausted

### 2. **Increased Default Stack Size**
- Changed default from 16KB (0x4000) to 64KB (0x10000)
- Updated in:
  - `src/runtime_config.rs`: Changed default in RuntimeConfig
  - `src/dispatcher/core.rs`: Direct stack size configuration
  - `src/typed/core.rs`: Direct stack size configuration
- Rationale: 64KB provides enough space for complex handlers while still being memory-efficient

### 3. **Improved Error Response Handling**
- Modified dispatcher to return proper HTTP error responses instead of `None`
- Channel closure now returns 503 Service Unavailable
- Provides clear error messages to clients
- Prevents connection drops

## Code Changes

### src/dispatcher/core.rs
```rust
// Before: 
coroutine::Builder::new()
    .stack_size(may::config().get_stack_size())
    .spawn(move || { ... })
    .unwrap();  // CRASH POINT

// After:
let stack_size = std::env::var("BRRTR_STACK_SIZE")
    .ok()
    .and_then(|s| /* parse logic */)
    .unwrap_or(0x10000); // 64KB default

let spawn_result = coroutine::Builder::new()
    .stack_size(stack_size)
    .spawn(move || { ... });

if let Err(e) = spawn_result {
    error!("Failed to spawn handler coroutine");
    return; // Graceful failure
}
```

### src/typed/core.rs
```rust
// Similar changes with panic for typed handlers
match spawn_result {
    Ok(_) => tx,
    Err(e) => {
        panic!("Failed to spawn typed handler coroutine: {e}. 
                Stack size: {stack_size} bytes. 
                Consider increasing BRRTR_STACK_SIZE environment variable.");
    }
}
```

### src/runtime_config.rs
```rust
// Changed default stack size
pub struct RuntimeConfig {
    /// Stack size for coroutines in bytes (default: 64 KB / 0x10000)
    /// Increased from 16KB to prevent stack overflows in complex handlers
    pub stack_size: usize,
}
```

## Testing Recommendations

1. **Load Testing**: Run with high concurrency to verify no crashes under load
2. **Resource Exhaustion**: Test behavior when system resources are limited
3. **Complex Handlers**: Test with handlers that have deep call stacks
4. **Error Scenarios**: Verify proper error responses instead of connection drops

## Environment Variables

- `BRRTR_STACK_SIZE`: Can be set to override default stack size
  - Example: `BRRTR_STACK_SIZE=0x20000` for 128KB
  - Example: `BRRTR_STACK_SIZE=131072` for 128KB in decimal

## Monitoring

After deployment, monitor for:
- Reduced crash frequency
- Proper 503/504 error responses instead of connection drops
- Memory usage (slightly higher due to larger stacks)
- Handler spawn failures in logs

## Future Improvements

1. **Dynamic Stack Sizing**: Analyze handler complexity to set per-handler stack sizes
2. **Resource Pool Management**: Implement coroutine pooling with backpressure
3. **Circuit Breaker**: Add circuit breaker pattern for failing handlers
4. **Metrics**: Add metrics for spawn failures and stack usage
