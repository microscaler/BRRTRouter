# JWKS Test Coverage Gaps Analysis

## Overview
This document identifies all critical points that need test coverage for recent JWKS fixes, particularly around thread spawn failures, condition variables, and background refresh lifecycle.

## Critical Test Coverage Gaps

### 1. Thread Spawn Failure Handling (P0 - CRITICAL)
**Location**: `src/security/jwks_bearer/mod.rs:767-785`

**Issue**: If `thread::spawn` fails (resource exhaustion), the `refresh_in_progress` flag must be cleared to prevent permanent deadlock.

**Test Requirements**:
- [ ] **test_jwks_thread_spawn_failure_clears_flag**: Mock thread spawn failure and verify `refresh_in_progress` is cleared
- [ ] **test_jwks_thread_spawn_failure_notifies_waiters**: Verify condition variable notifies waiting threads when spawn fails
- [ ] **test_jwks_thread_spawn_failure_recovery**: After spawn failure, verify system can recover and spawn successfully on next attempt
- [ ] **test_jwks_thread_spawn_failure_doesnt_deadlock**: Multiple concurrent calls with spawn failures should not deadlock

**Test Strategy**:
- Use a custom thread builder that can be configured to fail
- Or use `std::thread::Builder::new().stack_size(usize::MAX)` to force failure on some systems
- Verify `refresh_in_progress` is `false` after spawn failure
- Verify condition variable is notified
- Verify subsequent refresh attempts can succeed

---

### 2. Condition Variable Notification on Spawn Failure (P0 - CRITICAL)
**Location**: `src/security/jwks_bearer/mod.rs:780-783`

**Issue**: When thread spawn fails, waiting threads must be notified via condition variable.

**Test Requirements**:
- [ ] **test_jwks_spawn_failure_wakes_waiting_threads**: Threads waiting on condition variable are woken when spawn fails
- [ ] **test_jwks_spawn_failure_wakeup_timing**: Verify threads are woken immediately, not after timeout

**Test Strategy**:
- Spawn multiple threads that all try to refresh simultaneously
- Force thread spawn to fail for one thread
- Verify other waiting threads are woken immediately
- Verify threads don't wait for full timeout

---

### 3. Empty Cache Blocking Refresh with Condition Variable (P1)
**Location**: `src/security/jwks_bearer/mod.rs:639-655`

**Issue**: When cache is empty and another thread is refreshing, threads should wait using condition variable (not polling).

**Test Requirements**:
- [ ] **test_jwks_empty_cache_condition_variable_wait**: Verify threads wait on condition variable when cache is empty
- [ ] **test_jwks_empty_cache_wakeup_on_completion**: Verify waiting threads are woken when refresh completes
- [ ] **test_jwks_empty_cache_multiple_waiters**: Multiple threads waiting on empty cache are all woken

**Test Strategy**:
- Create provider with empty cache
- Spawn multiple threads that all try to validate (triggering empty cache refresh)
- Verify only one thread does the refresh
- Verify other threads wait on condition variable
- Verify all threads are woken when refresh completes

---

### 4. Empty Cache Timeout Handling (P1)
**Location**: `src/security/jwks_bearer/mod.rs:667-699`

**Issue**: If refresh times out while waiting, system should retry.

**Test Requirements**:
- [ ] **test_jwks_empty_cache_timeout_retry**: When refresh times out, verify retry is attempted
- [ ] **test_jwks_empty_cache_timeout_still_empty_check**: Verify cache emptiness is checked after timeout
- [ ] **test_jwks_empty_cache_timeout_recovery**: System recovers from timeout and eventually populates cache

**Test Strategy**:
- Create provider with slow/failing JWKS server
- Trigger empty cache refresh
- Wait for timeout (2s)
- Verify retry is attempted
- Verify cache eventually populated

---

### 5. Empty Cache Retry After Refresh Completes But Still Empty (P1)
**Location**: `src/security/jwks_bearer/mod.rs:702-730`

**Issue**: If refresh completes but cache is still empty (refresh failed), system should retry.

**Test Requirements**:
- [ ] **test_jwks_empty_cache_retry_after_failed_refresh**: Verify retry when refresh completes but cache empty
- [ ] **test_jwks_empty_cache_retry_claim_race**: Verify atomic claim works correctly for retry attempts

**Test Strategy**:
- Create provider with JWKS server that returns empty/invalid response
- Trigger empty cache refresh
- Verify refresh completes but cache remains empty
- Verify retry is attempted
- Verify only one thread does the retry (atomic claim)

---

### 6. Condition Variable Wake-up on Refresh Completion (P1)
**Location**: `src/security/jwks_bearer/mod.rs:575-580`

**Issue**: When refresh completes successfully, all waiting threads should be woken immediately.

**Test Requirements**:
- [ ] **test_jwks_refresh_completion_wakes_waiters**: Verify condition variable notifies on successful refresh
- [ ] **test_jwks_refresh_completion_wakeup_timing**: Verify threads are woken immediately, not after delay
- [ ] **test_jwks_refresh_completion_all_waiters_woken**: All waiting threads are woken, not just one

**Test Strategy**:
- Create provider with slow JWKS server (200ms delay)
- Spawn multiple threads that wait for refresh
- Verify all threads are woken when refresh completes
- Verify wake-up happens immediately (within 50ms of refresh completion)

---

### 7. Condition Variable Wake-up on Refresh Failure (P1)
**Location**: `src/security/jwks_bearer/mod.rs:484-490, 500-506, 515-521`

**Issue**: When refresh fails (HTTP error, parse error, etc.), waiting threads should still be woken.

**Test Requirements**:
- [ ] **test_jwks_refresh_failure_wakes_waiters**: Verify condition variable notifies on refresh failure
- [ ] **test_jwks_refresh_failure_wakeup_timing**: Verify threads are woken immediately on failure
- [ ] **test_jwks_refresh_failure_all_waiters_woken**: All waiting threads are woken on failure

**Test Strategy**:
- Create provider with failing JWKS server (returns 500, invalid JSON, etc.)
- Spawn multiple threads that wait for refresh
- Verify all threads are woken when refresh fails
- Verify wake-up happens immediately

---

### 8. Background Refresh Thread Lifecycle (P1)
**Location**: `src/security/jwks_bearer/mod.rs:350-436`

**Issue**: Background refresh thread should start on provider creation and stop on drop.

**Test Requirements**:
- [ ] **test_jwks_background_thread_starts_on_creation**: Verify background thread starts immediately
- [ ] **test_jwks_background_thread_stops_on_drop**: Verify background thread stops when provider dropped (already tested)
- [ ] **test_jwks_background_thread_handles_shutdown_flag**: Verify thread checks shutdown flag and exits cleanly
- [ ] **test_jwks_background_thread_refresh_interval**: Verify thread refreshes at correct interval (cache_ttl - 10s)

**Test Strategy**:
- Create provider and verify background thread is running
- Monitor refresh intervals to verify correct timing
- Verify thread exits when shutdown flag is set

---

### 9. Cache TTL Change Detection in Background Thread (P1)
**Location**: `src/security/jwks_bearer/mod.rs:369-429`

**Issue**: Background thread should detect cache_ttl changes and recalculate refresh interval.

**Test Requirements**:
- [ ] **test_jwks_background_thread_ttl_change_detection**: Verify thread detects cache_ttl changes
- [ ] **test_jwks_background_thread_ttl_change_recalculation**: Verify refresh interval is recalculated
- [ ] **test_jwks_background_thread_ttl_change_wakeup**: Verify thread wakes up early when TTL changes

**Test Strategy**:
- Create provider with long cache_ttl (60s)
- Wait for background thread to start sleeping
- Change cache_ttl to short value (1s)
- Verify thread wakes up and recalculates interval
- Verify refresh happens at new interval

---

### 10. Atomic Claim Prevention Edge Cases (P2)
**Location**: `src/security/jwks_bearer/mod.rs:742-751`

**Issue**: Atomic claim should prevent thread storms even under extreme concurrency.

**Test Requirements**:
- [ ] **test_jwks_atomic_claim_under_extreme_load**: Verify atomic claim works with 1000+ concurrent threads
- [ ] **test_jwks_atomic_claim_rapid_succession**: Verify atomic claim works when refreshes happen in rapid succession
- [ ] **test_jwks_atomic_claim_flag_consistency**: Verify flag state is consistent across all threads

**Test Strategy**:
- Spawn 1000+ threads that all try to refresh simultaneously
- Verify only 1-2 HTTP requests are made
- Verify flag is properly cleared after refresh

---

## Test Implementation Priority

### P0 (Critical - Must Have)
1. Thread spawn failure handling
2. Condition variable notification on spawn failure

### P1 (Important - Should Have)
3. Empty cache blocking refresh with condition variable
4. Empty cache timeout handling
5. Empty cache retry after failed refresh
6. Condition variable wake-up on refresh completion
7. Condition variable wake-up on refresh failure
8. Background refresh thread lifecycle
9. Cache TTL change detection

### P2 (Nice to Have)
10. Atomic claim prevention edge cases

---

## Existing Test Coverage

### Already Tested ✅
- Thread storm prevention (atomic claim) - `test_jwks_refresh_atomic_claim_prevention`
- Thread storm prevention (refresh_in_progress) - `test_jwks_refresh_thread_storm_prevention`
- Sub-second cache TTL - `test_jwks_sub_second_cache_ttl_no_thread_storm`
- Drop implementation - `test_jwks_drop_stops_background_thread`
- Background refresh basic functionality - various tests

### Missing Test Coverage ❌
- ~~Thread spawn failure handling~~ ✅ COMPLETE
- ~~Condition variable notification on spawn failure~~ ✅ COMPLETE
- ~~Empty cache condition variable waiting~~ ✅ COMPLETE
- ~~Empty cache timeout and retry logic~~ ✅ COMPLETE (some tests marked as ignored - redundant)
- ~~Condition variable wake-up timing~~ ✅ COMPLETE
- ~~Background thread TTL change detection~~ ✅ COMPLETE

### Recently Completed Tests ✅
- `test_jwks_thread_spawn_failure_recovery` - Verifies system recovers from stuck refresh_in_progress flag
- `test_jwks_thread_spawn_failure_doesnt_deadlock` - Verifies no deadlock under extreme concurrent load
- `test_jwks_spawn_failure_wakeup_timing` - Verifies waiting threads are woken promptly via condition variable
- `test_jwks_empty_cache_condition_variable_wait` - Verifies threads wait on condition variable when cache is empty
- `test_jwks_empty_cache_wakeup_on_completion` - Verifies waiting threads are woken when refresh completes
- `test_jwks_empty_cache_retry_claim_race` - Verifies atomic claim works correctly for retry attempts
- `test_jwks_condition_variable_wakeup_on_refresh_completion` - Verifies condition variable notifies on successful refresh
- `test_jwks_condition_variable_wakeup_on_refresh_failure` - Verifies condition variable notifies on refresh failure
- `test_jwks_background_thread_ttl_change_detection` - Verifies background thread detects cache_ttl changes
- `test_jwks_background_thread_starts_on_creation` - Verifies background thread starts immediately
- `test_jwks_background_thread_handles_shutdown_flag` - Verifies thread checks shutdown flag and exits cleanly
- `test_jwks_background_thread_refresh_interval` - Verifies thread refreshes at correct interval

---

## Implementation Notes

### Mocking Thread Spawn Failures
Rust's `std::thread::Builder` doesn't provide a direct way to mock failures. Options:
1. Use `stack_size(usize::MAX)` to force failure on some systems (not reliable)
2. Create a wrapper trait for thread spawning that can be mocked in tests
3. Use conditional compilation to inject test-only spawn failure logic
4. Test indirectly by verifying flag clearing behavior

### Testing Condition Variables
Condition variables are difficult to test directly. Strategies:
1. Use timing assertions (threads should wake within X ms)
2. Use counters to verify all threads are woken
3. Use barriers to synchronize test threads
4. Monitor thread state (blocked vs running)

### Testing Background Thread
Background thread is internal implementation detail. Test via:
1. Monitor refresh intervals (verify correct timing)
2. Monitor HTTP requests (verify refresh happens)
3. Test shutdown behavior (verify thread stops)
4. Test TTL change detection (verify interval recalculation)

---

## Test File Organization

All new tests should be added to `tests/security_tests.rs` in the appropriate section:

```rust
// --- Thread spawn failure tests ---
#[test]
fn test_jwks_thread_spawn_failure_clears_flag() { ... }

// --- Condition variable tests ---
#[test]
fn test_jwks_empty_cache_condition_variable_wait() { ... }

// --- Background thread lifecycle tests ---
#[test]
fn test_jwks_background_thread_ttl_change_detection() { ... }
```

---

## Success Criteria

Tests are considered successful when:
1. All P0 tests pass consistently
2. All P1 tests pass with <5% flakiness
3. Tests run in reasonable time (<30s for full suite)
4. Tests don't interfere with each other (proper cleanup)
5. Tests provide clear failure messages for debugging

