# BrokenPipe Error Analysis - may_minihttp

## Issue Description

The error message:
```
{"timestamp":"2025-10-20T06:22:41.884117Z","level":"ERROR","fields":{"message":"service err = Custom { kind: BrokenPipe, error: \"read closed\" }","log.target":"may_minihttp::http_server","log.module_path":"may_minihttp::http_server","log.file":"/Users/casibbald/.cargo/git/checkouts/may_minihttp-59f6bf6d3835a79f/7140a4a/src/http_server.rs","log.line":275},"target":"may_minihttp::http_server","threadId":"ThreadId(6)"}
```

## Root Cause

The error occurs in `may_minihttp/src/http_server.rs` when:

1. **Client closes connection**: When a client closes the TCP connection (e.g., browser cancels request, client timeout, network interruption)
2. **Server attempts to read**: The server's `nonblock_read` function (line 92) receives 0 bytes from `stream.read()`
3. **Error is created**: This triggers creation of a `BrokenPipe` error with message "read closed"
4. **Error is logged**: The error propagates up to line 282 where it's logged as ERROR level

## Code Flow

### 1. Error Creation (line 92)
```rust
fn nonblock_read(stream: &mut impl Read, req_buf: &mut BytesMut) -> io::Result<bool> {
    // ...
    match stream.read(unsafe { read_buf.get_unchecked_mut(read_cnt..) }) {
        Ok(0) => return err(io::Error::new(io::ErrorKind::BrokenPipe, "read closed")),
        // ...
    }
}
```

### 2. Error Propagation (line 162)
```rust
fn each_connection_loop_with_headers<T: HttpService, const N: usize>(
    stream: &mut TcpStream,
    mut service: T,
) -> io::Result<()> {
    loop {
        let read_blocked = nonblock_read(stream.inner_mut(), &mut req_buf)?;
        // Error propagates here
    }
}
```

### 3. Error Logging (line 282)
```rust
go!(move || if let Err(e) = 
    each_connection_loop_with_headers::<T, N>(&mut stream, service)
{
    error!("service err = {e:?}");  // Logged as ERROR
    stream.shutdown(std::net::Shutdown::Both).ok();
});
```

## Why This Is Not Critical

1. **Normal HTTP behavior**: Clients closing connections is normal in HTTP:
   - Browser navigation away from page
   - Request timeouts
   - Network interruptions
   - Keep-alive timeout expiry
   - Client-side cancellations

2. **Graceful handling**: The code properly:
   - Catches the error
   - Shuts down the stream cleanly
   - Continues serving other connections

3. **No resource leak**: Each connection runs in its own coroutine which properly terminates

## The Real Problem

The issue is **inappropriate error severity**. This should be logged at INFO or DEBUG level, not ERROR, because:
- It's expected behavior
- It doesn't indicate a server problem
- It creates noise in production logs
- It may trigger false alerts in monitoring systems

## Solutions

### Option 1: Fix in may_minihttp (Recommended)
Modify `may_minihttp/src/http_server.rs` line 282:

```rust
go!(move || if let Err(e) = 
    each_connection_loop_with_headers::<T, N>(&mut stream, service)
{
    match e.kind() {
        io::ErrorKind::BrokenPipe | 
        io::ErrorKind::ConnectionAborted | 
        io::ErrorKind::ConnectionReset => {
            debug!("connection closed: {e:?}");  // Log as DEBUG
        }
        _ => {
            error!("service err = {e:?}");  // Real errors stay as ERROR
        }
    }
    stream.shutdown(std::net::Shutdown::Both).ok();
});
```

### Option 2: Fork and Patch
Since may_minihttp is a Git dependency, we can:
1. Fork the repository
2. Apply the fix
3. Update Cargo.toml to point to our fork

### Option 3: Log Filtering (Temporary)
Configure log filtering to suppress these specific errors:
```rust
// In BRRTRouter initialization
tracing_subscriber::fmt()
    .with_env_filter(
        EnvFilter::from_default_env()
            .add_directive("may_minihttp::http_server=warn".parse().unwrap())
    )
    .init();
```

### Option 4: Wrapper Service
Create a wrapper around AppService that filters these errors before they reach may_minihttp.

## Impact on BRRTRouter

### Current Impact
- **Performance**: None - errors are handled gracefully
- **Functionality**: None - service continues operating normally
- **Monitoring**: High - excessive ERROR logs may trigger alerts
- **Debugging**: Medium - real errors may be lost in noise

### After Fix
- Clean logs showing only real errors
- Better monitoring accuracy
- Easier debugging
- Professional production deployment

## Recommendation

1. **Immediate**: Apply Option 3 (log filtering) to reduce noise
2. **Short-term**: Fork may_minihttp and apply Option 1
3. **Long-term**: Submit PR to upstream may_minihttp with the fix

## Testing the Fix

To verify the fix works:
1. Start pet_store server
2. Use curl with timeout: `curl --max-time 1 http://localhost:8080/pets`
3. Use browser and navigate away mid-request
4. Check logs - should see DEBUG messages, not ERROR

## Related Issues

This is similar to common issues in other HTTP servers:
- nginx: "client closed connection while waiting for request"
- Apache: "client gone away"
- Node.js: ECONNRESET errors

All mature HTTP servers handle these at DEBUG/INFO level, not ERROR.
