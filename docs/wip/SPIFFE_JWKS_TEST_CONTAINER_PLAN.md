# SPIFFE JWKS Mock Server Test Container Plan

## Current State Analysis

### Existing Patterns in Codebase

1. **Docker Test Containers** (`tests/docker_integration_tests.rs`, `tests/curl_harness.rs`):
   - Uses `bollard` crate for Docker API
   - RAII pattern: `DockerTestContainer` and `ContainerHarness` with automatic cleanup
   - Process ID-based naming for parallel test execution
   - Used for E2E tests with full application containers

2. **In-Process Mock Servers** (`tests/security_tests.rs`):
   - Simple TCP listener in a spawned thread
   - Pattern: `start_mock_jwks_server()` and `start_mock_apikey_verify_server()`
   - **Current Issue**: Only handles one connection, timing/synchronization problems
   - Lightweight, no external dependencies

### Current Problem

The existing `start_mock_jwks_server()` in `tests/spiffe_tests.rs`:
- Only accepts one connection (loop exits after first request)
- No proper HTTP parsing
- Race conditions with server startup timing
- No connection reuse for multiple requests (cache refresh, retries)

## Proposed Solutions

### Option A: Improved In-Process Mock Server (RECOMMENDED)

**Pros:**
- ✅ Fastest (no Docker overhead)
- ✅ No external dependencies
- ✅ Matches existing pattern in `security_tests.rs`
- ✅ Works in CI/CD without Docker
- ✅ Easy to debug

**Cons:**
- ⚠️ Still in-process (potential timing issues)
- ⚠️ Requires proper HTTP parsing

**Implementation:**
1. Use a proper HTTP server library (`tiny-http` or similar lightweight option)
2. Handle multiple connections in a loop
3. Proper HTTP request parsing
4. RAII wrapper for automatic cleanup
5. Wait for server readiness before returning URL

**Dependencies:**
- Add `tiny-http = "0.8"` to `[dev-dependencies]` (lightweight, synchronous HTTP server)

### Option B: Lightweight HTTP Server Container

**Pros:**
- ✅ Isolated from test process
- ✅ More realistic (separate process)
- ✅ Can use standard HTTP server images (nginx, httpd)

**Cons:**
- ❌ Requires Docker (may not be available in all CI environments)
- ❌ Slower startup (container initialization)
- ❌ More complex setup

**Implementation:**
1. Use `bollard` (already in dependencies) to start a lightweight HTTP server container
2. Create a simple nginx/httpd container with JWKS JSON file
3. Use RAII pattern like `DockerTestContainer`
4. Mount JWKS JSON as volume or embed in image

**Dependencies:**
- Already has `bollard` in `[dev-dependencies]`
- Would need to build/pull a lightweight HTTP server image

### Option C: Testcontainers-rs Library

**Pros:**
- ✅ Standard library for test containers
- ✅ Well-maintained and documented
- ✅ Supports many container types

**Cons:**
- ❌ Additional dependency
- ❌ Requires Docker
- ❌ Overkill for simple JWKS mock server

**Implementation:**
1. Add `testcontainers = "0.15"` to `[dev-dependencies]`
2. Use `GenericImage` or `HttpWaitStrategy`
3. Create minimal HTTP server container

## Recommended Approach: Option A (Improved In-Process)

### Implementation Plan

#### Phase 1: Add HTTP Server Dependency

```toml
[dev-dependencies]
tiny-http = "0.8"  # Lightweight, synchronous HTTP server
```

#### Phase 2: Create RAII Mock Server Wrapper

```rust
/// RAII wrapper for JWKS mock server with automatic cleanup
struct JwksMockServer {
    server: Option<tiny_http::Server>,
    url: String,
    handle: Option<thread::JoinHandle<()>>,
}

impl JwksMockServer {
    /// Start a new JWKS mock server
    fn new(jwks_json: String) -> Self {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().port();
        let url = format!("http://127.0.0.1:{}/jwks.json", port);
        
        let jwks_json_clone = jwks_json.clone();
        let handle = thread::spawn(move || {
            loop {
                match server.recv() {
                    Ok(Some(request)) => {
                        let response = tiny_http::Response::new(
                            200.into(),
                            vec![
                                tiny_http::Header::from_bytes(
                                    &b"Content-Type"[..],
                                    b"application/json"
                                ).unwrap(),
                            ],
                            jwks_json_clone.as_bytes().to_vec().into(),
                            Some(jwks_json_clone.len()),
                            None,
                        );
                        let _ = request.respond(response);
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        });
        
        // Wait for server to be ready
        thread::sleep(Duration::from_millis(50));
        
        Self {
            server: Some(server),
            url,
            handle: Some(handle),
        }
    }
    
    fn url(&self) -> &str {
        &self.url
    }
}

impl Drop for JwksMockServer {
    fn drop(&mut self) {
        // Server will stop when handle is dropped
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
```

#### Phase 3: Update Tests

Replace `start_mock_jwks_server()` calls with:

```rust
let mock_server = JwksMockServer::new(jwks);
let provider = SpiffeProvider::new()
    .trust_domains(&["example.com"])
    .audiences(&["api.example.com"])
    .jwks_url(mock_server.url());
```

### Alternative: Use Existing Pattern (Simpler Fix)

If we want to avoid adding dependencies, we can improve the existing TCP listener pattern:

```rust
fn start_mock_jwks_server(jwks: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://127.0.0.1:{}/jwks.json", addr.port());
    let jwks_clone = jwks.clone();
    
    thread::spawn(move || {
        // Handle multiple connections
        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let mut buf = [0u8; 1024];
                if stream.read(&mut buf).is_ok() {
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                        jwks_clone.len(),
                        jwks_clone
                    );
                    let _ = stream.write_all(response.as_bytes());
                }
            }
        }
    });
    
    // Wait for server to be ready
    thread::sleep(Duration::from_millis(100));
    
    url
}
```

**Key Fix**: Change from `accept()` (single connection) to `incoming()` loop (multiple connections).

## Recommendation

**Use Option A with `tiny-http`** because:
1. Matches the lightweight testing philosophy
2. Proper HTTP handling eliminates timing issues
3. RAII pattern ensures cleanup
4. No Docker dependency (works everywhere)
5. Fast startup (no container overhead)

## Migration Steps

1. Add `tiny-http` to `[dev-dependencies]`
2. Create `JwksMockServer` struct in `tests/spiffe_tests.rs`
3. Replace all `start_mock_jwks_server()` calls
4. Update tests to use RAII pattern
5. Remove old `start_mock_jwks_server()` function

## Testing the Solution

After implementation, verify:
- ✅ Multiple concurrent requests work
- ✅ Cache refresh requests succeed
- ✅ Server cleanup on test completion
- ✅ No port conflicts in parallel tests
- ✅ Tests pass consistently (no timing issues)

