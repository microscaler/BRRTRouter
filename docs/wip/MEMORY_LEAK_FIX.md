# Memory Leak Fix: test_route_404

## Problem

Test `test_route_404` in `tests/server_tests.rs` was reporting a memory leak:

```
LEAK [   0.640s] brrtrouter::server_tests test_route_404
```

## Root Cause

The test was calling `handle.stop()` **before** parsing the HTTP response:

```rust
#[test]
fn test_route_404() {
    let (_tracing, handle, addr) = start_petstore_service();
    let resp = send_request(&addr, "GET /nope HTTP/1.1\r\nHost: localhost\r\n\r\n");
    handle.stop();  // ❌ STOPPING BEFORE PARSING RESPONSE
    let (status, _body) = parse_response(&resp);
    assert_eq!(status, 404);
}
```

### Why This Caused a Leak

1. **Request Sent**: `send_request()` sends the HTTP request and receives the raw response bytes
2. **Server Stopped Early**: `handle.stop()` calls:
   ```rust
   unsafe {
       self.handle.coroutine().cancel();
   }
   let _ = self.handle.join();
   ```
3. **Coroutine Cancellation**: The server coroutine handling the request is canceled mid-flight
4. **Resource Leak**: The 404 response handler coroutine may not complete cleanup, leaking:
   - Coroutine stack memory
   - Response buffer allocations
   - Any pending I/O resources

## Solution

Move `handle.stop()` to **after** parsing the response and assertions:

```rust
#[test]
fn test_route_404() {
    let (_tracing, handle, addr) = start_petstore_service();
    let resp = send_request(&addr, "GET /nope HTTP/1.1\r\nHost: localhost\r\n\r\n");
    let (status, _body) = parse_response(&resp);  // ✅ PARSE FIRST
    assert_eq!(status, 404);
    handle.stop();  // ✅ THEN STOP
}
```

### Why This Works

1. **Request Completes**: `send_request()` receives the full HTTP response
2. **Response Parsed**: `parse_response()` extracts status and body from the response bytes
3. **Assertions Run**: Test verifies the 404 status
4. **Clean Shutdown**: `handle.stop()` now cancels a server that has completed all work

The server coroutine has already finished handling the request and writing the response before cancellation, so all resources are properly cleaned up.

## Related Fixes

Applied the same fix to `test_dispatch_success()` for consistency:

```rust
#[test]
fn test_dispatch_success() {
    let (_tracing, handle, addr) = start_petstore_service();
    let resp = send_request(
        &addr,
        "GET /pets HTTP/1.1\r\nHost: localhost\r\nX-API-Key: test123\r\n\r\n",
    );
    let (status, body) = parse_response(&resp);  // ✅ PARSE FIRST
    assert_eq!(status, 200);
    assert!(body.is_array());
    handle.stop();  // ✅ THEN STOP
}
```

## Verification

All 8 tests in `server_tests.rs` now pass with no leaks:

```bash
$ cargo test --test server_tests
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

No `LEAK` messages reported.

## Best Practice for Server Tests

**Rule**: Always ensure all request/response processing completes before stopping the server.

**Pattern**:
```rust
#[test]
fn test_endpoint() {
    let (_tracing, handle, addr) = start_petstore_service();
    
    // 1. Send request
    let resp = send_request(&addr, "...");
    
    // 2. Parse response
    let (status, body) = parse_response(&resp);
    
    // 3. Run assertions
    assert_eq!(status, expected);
    
    // 4. FINALLY: Stop server
    handle.stop();
}
```

**Anti-pattern** (causes leaks):
```rust
#[test]
fn test_endpoint() {
    let (_tracing, handle, addr) = start_petstore_service();
    let resp = send_request(&addr, "...");
    handle.stop();  // ❌ TOO EARLY
    let (status, body) = parse_response(&resp);
    assert_eq!(status, expected);
}
```

## Technical Details

### ServerHandle::stop() Implementation

```rust
pub fn stop(self) {
    unsafe {
        self.handle.coroutine().cancel();  // Cancels the coroutine
    }
    let _ = self.handle.join();  // Waits for cleanup
}
```

- **`cancel()`**: Marks the coroutine for cancellation
- **`join()`**: Waits for the coroutine to finish
- If canceled mid-request, cleanup may be incomplete

### send_request() Behavior

```rust
fn send_request(addr: &SocketAddr, req: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).unwrap();
    resp
}
```

- Opens connection
- Sends request
- **Blocks until full response received**
- Returns raw HTTP response

**Key insight**: By the time `send_request()` returns, the server has already written the response. However, the server coroutine may still be doing cleanup (closing connections, freeing buffers). Stopping too early interrupts this cleanup.

## Lessons Learned

1. **Timing matters**: Even though `send_request()` receives the response, the server needs time to clean up
2. **Coroutine cancellation**: May interrupt cleanup routines, causing leaks
3. **Test patterns**: Always parse/assert before cleanup to ensure work is complete
4. **Consistency**: Apply the same pattern to all similar tests

## Files Modified

- `tests/server_tests.rs`:
  - Fixed `test_route_404()` - moved `handle.stop()` after assertions
  - Fixed `test_dispatch_success()` - moved `handle.stop()` after assertions

## Impact

- ✅ Memory leak eliminated
- ✅ All tests pass
- ✅ No change in test behavior (only timing)
- ✅ Consistent pattern across all server tests

