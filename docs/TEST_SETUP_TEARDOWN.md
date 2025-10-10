# Test Setup and Teardown in Rust

## Overview

Rust provides several patterns for test setup and teardown, similar to Python's `setUp()` and `tearDown()` methods. The most idiomatic approach uses **RAII (Resource Acquisition Is Initialization)** with the `Drop` trait.

**⚠️ Critical Addition**: For resources stored in static `OnceLock`, the `Drop` trait alone is insufficient when tests are interrupted with SIGINT (Ctrl+C). See [Signal Handling for Static Resources](#signal-handling-for-static-resources) below.

## Comparison with Python

| Python | Rust Equivalent | Description |
|--------|-----------------|-------------|
| `setUp()` | Constructor (`new()`) | Run before test |
| `tearDown()` | `Drop::drop()` | Run after test (automatically) |
| `setUpClass()` | `lazy_static!` + `Once` | Run once for all tests |
| `tearDownClass()` | N/A (use Drop on singleton) | Run once after all tests |

## Approach 1: RAII with Drop Trait (Recommended)

This is the most idiomatic Rust pattern.

### Implementation

```rust
/// Test fixture with automatic setup and teardown
struct PetStoreTestServer {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}

impl PetStoreTestServer {
    /// Setup: Create and start the test server
    fn new() -> Self {
        // Setup code here
        may::config().set_stack_size(0x8000);
        let tracing = TestTracing::init();
        let (routes, schemes, _slug) = brrtrouter::load_spec_full("examples/openapi.yaml").unwrap();
        // ... more setup ...
        let handle = HttpServer(service).start(addr).unwrap();
        handle.wait_ready().unwrap();
        
        Self {
            _tracing: tracing,
            handle: Some(handle),
            addr,
        }
    }
    
    fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for PetStoreTestServer {
    /// Teardown: Automatically stop server when test completes
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
        // All fields are automatically dropped here
    }
}
```

### Usage in Tests

```rust
#[test]
fn test_dispatch_success() {
    // Setup happens automatically in new()
    let server = PetStoreTestServer::new();
    
    // Test code
    let resp = send_request(&server.addr(), "GET /pets HTTP/1.1\r\n...");
    let (status, body) = parse_response(&resp);
    assert_eq!(status, 200);
    
    // Teardown happens automatically when 'server' goes out of scope
    // No manual cleanup needed!
}
```

### Benefits

✅ **Automatic cleanup** - Impossible to forget teardown
✅ **Panic-safe** - Cleanup runs even if test panics
✅ **No resource leaks** - Guaranteed cleanup
✅ **Idiomatic Rust** - Follows RAII principles
✅ **Composable** - Can nest fixtures

### When Cleanup Runs

```rust
#[test]
fn test_example() {
    let server = PetStoreTestServer::new();  // Setup runs here
    
    // Test assertions...
    assert_eq!(1 + 1, 2);
    
    // Drop runs here automatically when 'server' goes out of scope
}

#[test]
fn test_with_panic() {
    let server = PetStoreTestServer::new();  // Setup runs here
    
    panic!("Test panic!");
    
    // Drop STILL runs here, even after panic!
    // This prevents resource leaks
}
```

## Approach 2: Manual Setup/Teardown Functions

Less idiomatic but sometimes needed for legacy code.

```rust
fn setup() -> (TestTracing, ServerHandle, SocketAddr) {
    may::config().set_stack_size(0x8000);
    let tracing = TestTracing::init();
    // ... setup code ...
    (tracing, handle, addr)
}

fn teardown(tracing: TestTracing, handle: ServerHandle) {
    handle.stop();
    drop(tracing);
}

#[test]
fn test_manual() {
    let (tracing, handle, addr) = setup();
    
    // Test code...
    
    teardown(tracing, handle);  // Must remember to call!
}
```

### Problems

❌ **Easy to forget** - Teardown must be called manually
❌ **Not panic-safe** - Cleanup skipped if test panics
❌ **Resource leaks** - No guarantees
❌ **Not idiomatic** - Goes against Rust patterns

## Approach 3: Test Fixtures with `rstest`

For complex parameterized tests.

```toml
[dev-dependencies]
rstest = "0.18"
```

```rust
use rstest::*;

#[fixture]
fn server() -> PetStoreTestServer {
    PetStoreTestServer::new()
}

#[rstest]
fn test_with_fixture(server: PetStoreTestServer) {
    let resp = send_request(&server.addr(), "GET /pets HTTP/1.1\r\n...");
    assert_eq!(parse_response(&resp).0, 200);
}

#[rstest]
#[case(200, "/pets")]
#[case(404, "/nope")]
fn test_status_codes(server: PetStoreTestServer, #[case] expected: u16, #[case] path: &str) {
    let resp = send_request(&server.addr(), &format!("GET {} HTTP/1.1\r\n...", path));
    assert_eq!(parse_response(&resp).0, expected);
}
```

## Approach 4: One-Time Setup with `lazy_static!`

For expensive setup that should run only once for all tests.

```toml
[dev-dependencies]
lazy_static = "1.4"
```

```rust
use lazy_static::lazy_static;
use std::sync::Once;

static INIT: Once = Once::new();

lazy_static! {
    static ref SHARED_SERVER: Arc<Mutex<Option<ServerHandle>>> = {
        let handle = start_shared_server();
        Arc::new(Mutex::new(Some(handle)))
    };
}

fn setup_once() {
    INIT.call_once(|| {
        // Expensive one-time setup
        may::config().set_stack_size(0x8000);
        tracing_subscriber::fmt::init();
    });
}

#[test]
fn test_with_shared_setup() {
    setup_once();
    
    // Use shared resources
    let server = SHARED_SERVER.lock().unwrap();
    // ...
}
```

## Approach 5: Test Harness with `ctor`

For setup/teardown that runs before/after ALL tests.

```toml
[dev-dependencies]
ctor = "0.2"
```

```rust
use ctor::{ctor, dtor};

#[ctor]
fn before_all_tests() {
    println!("Setting up before all tests");
    std::env::set_var("RUST_LOG", "debug");
}

#[dtor]
fn after_all_tests() {
    println!("Cleaning up after all tests");
}

#[test]
fn test_1() {
    // before_all_tests() has already run
    assert_eq!(1 + 1, 2);
}

#[test]
fn test_2() {
    assert_eq!(2 + 2, 4);
}
// after_all_tests() runs when test binary exits
```

## Best Practices

### 1. **Use RAII for Per-Test Resources**

```rust
// ✅ Good: Automatic cleanup
#[test]
fn test_good() {
    let server = PetStoreTestServer::new();
    // Test code
    // Automatic cleanup
}

// ❌ Bad: Manual cleanup
#[test]
fn test_bad() {
    let (_, handle, _) = setup();
    // Test code
    handle.stop();  // Easy to forget!
}
```

### 2. **Make Fixtures Small and Focused**

```rust
// ✅ Good: Focused fixture
struct TestServer {
    addr: SocketAddr,
    handle: Option<ServerHandle>,
}

// ❌ Bad: God object fixture
struct TestEverything {
    server: ServerHandle,
    database: DbConnection,
    cache: RedisClient,
    queue: RabbitMQ,
    logger: Logger,
    // ...
}
```

### 3. **Use Composition for Complex Setups**

```rust
struct TestDatabase {
    conn: DbConnection,
}

impl TestDatabase {
    fn new() -> Self {
        // Setup database
        Self { conn }
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        // Cleanup database
    }
}

struct TestServer {
    _db: TestDatabase,  // Composed fixture
    handle: ServerHandle,
}

impl TestServer {
    fn new() -> Self {
        let db = TestDatabase::new();
        let handle = start_server_with_db(&db.conn);
        Self { _db: db, handle }
    }
}
```

### 4. **Document Fixture Behavior**

```rust
/// Test fixture for Pet Store API server
///
/// # Setup
/// - Configures 32KB coroutine stack size
/// - Initializes tracing subscriber
/// - Loads OpenAPI spec from `examples/openapi.yaml`
/// - Starts HTTP server on random port
/// - Waits for server to be ready
///
/// # Teardown
/// - Stops HTTP server gracefully
/// - Drops tracing subscriber
///
/// # Example
/// ```
/// #[test]
/// fn test_api() {
///     let server = PetStoreTestServer::new();
///     // Test code here
///     // Automatic cleanup when server goes out of scope
/// }
/// ```
struct PetStoreTestServer {
    // ...
}
```

## BRRTRouter Implementation

See `tests/server_tests.rs` for the complete implementation:

```rust
/// Test fixture with automatic setup and teardown using RAII
///
/// Implements Drop to ensure proper cleanup when test completes.
/// This is the Rust equivalent of Python's setup/teardown.
struct PetStoreTestServer {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}
```

### Usage Examples

```rust
#[test]
fn test_dispatch_success() {
    let server = PetStoreTestServer::new();
    let resp = send_request(&server.addr(), "GET /pets HTTP/1.1\r\n...");
    assert_eq!(parse_response(&resp).0, 200);
    // Automatic cleanup
}

#[test]
fn test_route_404() {
    let server = PetStoreTestServer::new();
    let resp = send_request(&server.addr(), "GET /nope HTTP/1.1\r\n...");
    assert_eq!(parse_response(&resp).0, 404);
    // Automatic cleanup
}
```

## Benefits Over Python's unittest

| Feature | Python unittest | Rust RAII |
|---------|-----------------|-----------|
| **Automatic cleanup** | Only if using `tearDown()` | Always (compiler-enforced) |
| **Panic safety** | Exceptions may skip cleanup | Always runs, even on panic |
| **Compile-time checks** | ❌ Runtime errors | ✅ Compile-time guarantees |
| **Resource leaks** | ⚠️  Possible | ✅ Prevented by type system |
| **Composability** | ⚠️  Limited | ✅ Natural composition |

## Common Patterns

### Pattern: Scoped Resources

```rust
#[test]
fn test_with_scoped_resources() {
    let outer = OuterResource::new();
    
    {
        let inner = InnerResource::new();
        // Use both resources
    } // inner.drop() called here
    
    // outer still valid
} // outer.drop() called here
```

### Pattern: Conditional Cleanup

```rust
impl Drop for TestServer {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("Test panicked, performing emergency cleanup");
        }
        // Normal cleanup
    }
}
```

### Pattern: Shared State with Arc

```rust
struct SharedTestState {
    data: Arc<Mutex<HashMap<String, String>>>,
}

impl SharedTestState {
    fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    fn clone_data(&self) -> Arc<Mutex<HashMap<String, String>>> {
        Arc::clone(&self.data)
    }
}

#[test]
fn test_shared_state() {
    let state = SharedTestState::new();
    let data1 = state.clone_data();
    let data2 = state.clone_data();
    // Both references share same data
}
```

## Migration Guide: Python to Rust

### Python

```python
class TestAPI(unittest.TestCase):
    def setUp(self):
        self.server = start_server()
        
    def tearDown(self):
        self.server.stop()
        
    def test_success(self):
        resp = requests.get(self.server.url + "/pets")
        self.assertEqual(resp.status_code, 200)
```

### Rust

```rust
struct TestAPI {
    server: ServerHandle,
    addr: SocketAddr,
}

impl TestAPI {
    fn new() -> Self {
        let (handle, addr) = start_server();
        Self { server: handle, addr }
    }
}

impl Drop for TestAPI {
    fn drop(&mut self) {
        self.server.stop();
    }
}

#[test]
fn test_success() {
    let api = TestAPI::new();
    let resp = send_request(&api.addr, "GET /pets HTTP/1.1\r\n...");
    assert_eq!(parse_response(&resp).0, 200);
}
```

## Resources

- [Rust Book: RAII and Drop](https://doc.rust-lang.org/book/ch15-03-drop.html)
- [rstest documentation](https://docs.rs/rstest/latest/rstest/)
- [lazy_static documentation](https://docs.rs/lazy_static/latest/lazy_static/)
- [ctor documentation](https://docs.rs/ctor/latest/ctor/)

## Assessment: Opportunities for Drop Trait Implementation

This section analyzes all test modules in BRRTRouter to identify opportunities for implementing the Drop trait pattern for automatic cleanup.

### Summary Statistics

| Category | Count | Needs Drop Trait |
|----------|-------|------------------|
| **Server Handle Cleanup** | 23 tests | ✅ High Priority |
| **Temporary File Cleanup** | 4 tests | ✅ Medium Priority |
| **Docker Container Cleanup** | 1 test | ✅ Already Has Drop |
| **File Watcher Cleanup** | 1 test | ⚠️  Manual drop OK |
| **No Cleanup Needed** | ~190 tests | ✅ Good |

### Detailed Analysis by Test Module

#### 1. server_tests.rs - ✅ **COMPLETE**

| Test | Manual Cleanup | Should Use Drop | Priority | Status |
|------|----------------|-----------------|----------|--------|
| `test_dispatch_success` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** (PetStoreTestServer) |
| `test_route_404` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** (PetStoreTestServer) |
| `test_panic_recovery` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** (CustomServerTestFixture) |
| `test_headers_and_cookies` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** (CustomServerTestFixture) |
| `test_status_201_json` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** (CustomServerTestFixture) |
| `test_text_plain_error` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** (CustomServerTestFixture) |
| `test_request_body_validation_failure` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** (CustomServerTestFixture with schemas) |
| `test_response_body_validation_failure` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** (CustomServerTestFixture with schemas) |

**Progress**: 8/8 tests completed ✅
**Test Results**: All 8 tests pass, no memory leaks detected
**Implementation**: 
- `PetStoreTestServer` for standard pet store tests (2 tests)
- `CustomServerTestFixture` for tests with custom handlers (6 tests)
  - `with_handler()` for basic custom handlers
  - `with_handler_and_schemas()` for validation tests

**Important Design Decision**: All test fixtures now include static and doc directories even when tests don't explicitly exercise them. This ensures comprehensive integration testing - if any change breaks static file serving or documentation endpoints, ALL tests will catch it, not just the dedicated static/docs tests. This provides robust regression detection.

```rust
struct CustomServerTestFixture {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}

impl Drop for CustomServerTestFixture {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
    }
}
```

#### 2. security_tests.rs - ✅ **COMPLETE**

| Test | Manual Cleanup | Should Use Drop | Priority | Status |
|------|----------------|-----------------|----------|--------|
| `test_api_key_auth` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** |
| `test_api_key_auth_via_authorization_bearer` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** |
| `test_bearer_jwks_success` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** |
| `test_bearer_jwks_invalid_signature` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** |
| `test_remote_apikey_success_and_failure` | ~~`handle.stop()` + `handle_verify.join()`~~ | ✅ YES | HIGH | ✅ **DONE** (Handled TWO servers!) |
| `test_multiple_security_providers` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** |
| `test_bearer_header_and_oauth_cookie` | ~~`handle.stop()`~~ | ✅ YES | HIGH | ✅ **DONE** |

**Progress**: 7/7 tests completed ✅
**Test Results**: All 28 tests pass, no memory leaks detected
**Implementation**: `SecurityTestServer` fixture with multiple constructors for different service types

**Important Design Decision**: All security test fixtures now include static and doc directories for comprehensive integration testing and regression detection.

**Recommendation**: Create a `SecurityTestServer` fixture.

```rust
struct SecurityTestServer {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
    // For tests with verification servers
    verify_handle: Option<JoinHandle<()>>,
}

impl Drop for SecurityTestServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
        if let Some(verify) = self.verify_handle.take() {
            verify.join().ok();
        }
    }
}
```

#### 3. static_server_tests.rs - ✅ **COMPLETE**

| Test | Manual Cleanup | Should Use Drop | Priority | Status |
|------|----------------|-----------------|----------|--------|
| `test_js_served` | ~~`handle.stop()`~~ | ✅ YES | MEDIUM | ✅ **DONE** |
| `test_root_served` | ~~`handle.stop()`~~ | ✅ YES | MEDIUM | ✅ **DONE** |
| `test_traversal_blocked` | ~~`handle.stop()`~~ | ✅ YES | MEDIUM | ✅ **DONE** |

**Progress**: 3/3 tests completed ✅
**Test Results**: All 3 tests pass, no memory leaks detected
**Implementation**: `StaticFileTestServer` fixture specifically for static file serving tests

**Special Notes**: 
- Uses `tests/staticdata` directory for test static files
- Includes doc directory for comprehensive integration testing
- Tests security (path traversal prevention) alongside functionality

#### 4. multi_response_tests.rs - ✅ **COMPLETE**

| Test | Manual Cleanup | Should Use Drop | Priority | Status |
|------|----------------|-----------------|----------|--------|
| `test_select_content_type_from_spec` | ~~`handle.stop()`~~ | ✅ YES | MEDIUM | ✅ **DONE** |

**Progress**: 1/1 test completed ✅
**Implementation**: `MultiResponseTestServer` fixture for content negotiation tests

#### 5. sse_tests.rs - ✅ **COMPLETE**

| Test | Manual Cleanup | Should Use Drop | Priority | Status |
|------|----------------|-----------------|----------|--------|
| `test_event_stream` | ~~`handle.stop()`~~ | ✅ YES | MEDIUM | ✅ **DONE** |

**Progress**: 1/1 test completed ✅
**Implementation**: `SseTestServer` fixture with API key authentication

#### 6. metrics_endpoint_tests.rs - ✅ **COMPLETE**

| Test | Manual Cleanup | Should Use Drop | Priority | Status |
|------|----------------|-----------------|----------|--------|
| `test_metrics_endpoint` | ~~`handle.stop()`~~ | ✅ YES | MEDIUM | ✅ **DONE** |

**Progress**: 1/1 test completed ✅
**Implementation**: `MetricsTestServer` fixture with MetricsMiddleware

#### 7. health_endpoint_tests.rs - ✅ **COMPLETE**

| Test | Manual Cleanup | Should Use Drop | Priority | Status |
|------|----------------|-----------------|----------|--------|
| `test_health_endpoint` | ~~`handle.stop()`~~ | ✅ YES | MEDIUM | ✅ **DONE** |

**Progress**: 1/1 test completed ✅
**Implementation**: `HealthTestServer` fixture

#### 8. docs_endpoint_tests.rs - ✅ **COMPLETE**

| Test | Manual Cleanup | Should Use Drop | Priority | Status |
|------|----------------|-----------------|----------|--------|
| `test_openapi_endpoint` | ~~`handle.stop()`~~ | ✅ YES | MEDIUM | ✅ **DONE** |
| `test_swagger_ui_endpoint` | ~~`handle.stop()`~~ | ✅ YES | MEDIUM | ✅ **DONE** |

**Progress**: 2/2 tests completed ✅
**Implementation**: `DocsTestServer` fixture for OpenAPI spec and Swagger UI

#### 9. hot_reload_tests.rs - ⚠️ SPECIAL CASE

| Test | Manual Cleanup | Should Use Drop | Priority | Notes |
|------|----------------|-----------------|----------|-------|
| `test_watch_spec_reload` | `drop(watcher)` + `std::fs::remove_file()` | ⚠️  MAYBE | LOW | Manual drop is explicit and clear |

**Total**: 1 test with manual cleanup

**Recommendation**: Could implement `TempSpecFile` fixture, but current code is clear enough.

```rust
struct TempSpecFile {
    path: PathBuf,
}

impl Drop for TempSpecFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
```

#### 10. generator_templates_tests.rs - ⚠️ FILE CLEANUP

| Test | Manual Cleanup | Should Use Drop | Priority | Notes |
|------|----------------|-----------------|----------|-------|
| `test_write_registry_rs` | `fs::remove_dir_all(&dir)` | ✅ YES | LOW | Could use tempfile crate |

**Total**: 1 test with file cleanup

**Recommendation**: Use `tempfile::TempDir` which has Drop built-in.

```rust
use tempfile::TempDir;

#[test]
fn test_write_registry_rs() {
    let dir = TempDir::new().unwrap();
    // ... test code ...
    // Automatic cleanup when dir goes out of scope
}
```

#### 11. generator_project_tests.rs - ⚠️ FILE CLEANUP

| Test | Manual Cleanup | Should Use Drop | Priority | Notes |
|------|----------------|-----------------|----------|-------|
| `test_generate_project_formats` | `fs::remove_dir_all(&dir)` | ✅ YES | LOW | Could use tempfile crate |

**Total**: 1 test with file cleanup

**Recommendation**: Use `tempfile::TempDir`.

#### 12. cli_tests.rs - ⚠️ IMPLICIT CLEANUP

| Test | Manual Cleanup | Should Use Drop | Priority | Notes |
|------|----------------|-----------------|----------|-------|
| `test_cli_generate_creates_project` | Uses `temp_dir()` | ❌ NO | N/A | Relies on OS temp cleanup |

**Total**: 0 tests need Drop trait (uses OS temp directory)

**Recommendation**: No change needed (temp dirs cleaned by OS).

#### 13. docker_integration_tests.rs - ⚠️ PARTIAL CLEANUP

| Test | Manual Cleanup | Should Use Drop | Priority | Notes |
|------|----------------|-----------------|----------|-------|
| `test_petstore_container_health` | `docker.remove_container()` | ⚠️  MAYBE | LOW | Already has explicit cleanup |

**Total**: 1 test with Docker cleanup

**Recommendation**: Could create `DockerContainerFixture` but current inline cleanup is adequate for single test.

#### 14. curl_harness.rs - ✅ HAS DROP IMPLEMENTATION

| Resource | Cleanup | Status | Notes |
|----------|---------|--------|-------|
| `ContainerHarness` | `impl Drop` | ✅ DONE | Properly implemented! |

**Status**: ✅ This is a model implementation - uses Drop correctly for Docker container cleanup.

#### 15. middleware_tests.rs - ❌ NO CLEANUP NEEDED

All tests are unit tests with no external resources.

**Status**: ✅ No action needed.

### Priority Summary

#### HIGH Priority (23 tests) - Server Handle Cleanup

**Modules needing Drop implementation**:
1. `server_tests.rs` - 6 tests (out of 8 total)
2. `security_tests.rs` - 7 tests
3. `static_server_tests.rs` - 3 tests
4. `multi_response_tests.rs` - 1 test
5. `sse_tests.rs` - 1 test
6. `metrics_endpoint_tests.rs` - 1 test
7. `health_endpoint_tests.rs` - 1 test
8. `docs_endpoint_tests.rs` - 2 tests

**Impact**: 
- Prevents memory leaks from server handles
- Ensures proper cleanup even on panic
- Makes tests more maintainable

#### MEDIUM Priority (3 tests) - Temporary File Cleanup

**Modules**:
1. `generator_templates_tests.rs` - 1 test
2. `generator_project_tests.rs` - 1 test
3. `hot_reload_tests.rs` - 1 test

**Impact**:
- Prevents temporary file accumulation
- Better for CI environments

#### LOW Priority - Special Cases

**Modules**:
1. `docker_integration_tests.rs` - Already has cleanup, could be improved
2. `cli_tests.rs` - Uses OS temp cleanup, no action needed

### Recommended Fixtures to Create

#### 1. CustomServerTestFixture (for server_tests.rs)

```rust
/// Test fixture for tests requiring custom handler registration
struct CustomServerTestFixture {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
}

impl CustomServerTestFixture {
    fn with_handler<F>(handler_name: &str, handler: F, route: RouteMeta) -> Self 
    where
        F: Fn(HandlerRequest) -> HandlerResponse + Send + Sync + 'static
    {
        may::config().set_stack_size(0x8000);
        let tracing = TestTracing::init();
        
        let router = Arc::new(RwLock::new(Router::new(vec![route])));
        let mut dispatcher = Dispatcher::new();
        unsafe {
            dispatcher.register_handler(handler_name, handler);
        }
        
        let service = AppService::new(
            router,
            Arc::new(RwLock::new(dispatcher)),
            HashMap::new(),
            PathBuf::from("examples/openapi.yaml"),
            None,
            None,
        );
        
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let handle = HttpServer(service).start(addr).unwrap();
        handle.wait_ready().unwrap();
        
        Self {
            _tracing: tracing,
            handle: Some(handle),
            addr,
        }
    }
    
    fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for CustomServerTestFixture {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
    }
}
```

#### 2. SecurityTestServer (for security_tests.rs)

```rust
/// Test fixture for security-related tests
struct SecurityTestServer {
    _tracing: TestTracing,
    handle: Option<ServerHandle>,
    addr: SocketAddr,
    verify_handle: Option<std::thread::JoinHandle<()>>,
}

impl SecurityTestServer {
    fn new_with_providers(providers: Vec<(&str, Arc<dyn SecurityProvider>)>) -> Self {
        // Similar setup to PetStoreTestServer but with custom providers
        // ...
        
        Self {
            _tracing: tracing,
            handle: Some(handle),
            addr,
            verify_handle: None,
        }
    }
    
    fn with_verify_server(mut self, verify_handle: std::thread::JoinHandle<()>) -> Self {
        self.verify_handle = Some(verify_handle);
        self
    }
    
    fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for SecurityTestServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
        if let Some(verify) = self.verify_handle.take() {
            verify.join().ok();
        }
    }
}
```

#### 3. TempSpecFile (for hot_reload_tests.rs)

```rust
/// Test fixture for temporary OpenAPI spec files
struct TempSpecFile {
    path: PathBuf,
}

impl TempSpecFile {
    fn new(content: &str) -> Self {
        use std::io::Write;
        let mut path = std::env::temp_dir();
        path.push(format!("test_spec_{}.yaml", std::process::id()));
        
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        
        Self { path }
    }
    
    fn path(&self) -> &Path {
        &self.path
    }
    
    fn update(&self, content: &str) {
        std::fs::write(&self.path, content).unwrap();
    }
}

impl Drop for TempSpecFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
```

### Migration Strategy

1. **Phase 1**: Create fixture structs in respective test modules
2. **Phase 2**: Migrate HIGH priority tests (server handles)
3. **Phase 3**: Migrate MEDIUM priority tests (file cleanup)
4. **Phase 4**: Document patterns and update test documentation

### Benefits of Implementation

1. **Prevents Memory Leaks**: Guaranteed cleanup even on panic
2. **Reduces Code Duplication**: Shared fixtures across tests
3. **Improves Maintainability**: Clear setup/teardown patterns
4. **Better CI Performance**: No orphaned resources
5. **Type Safety**: Compiler enforces cleanup

### Before/After Example

**Before** (23 tests like this):
```rust
#[test]
fn test_api_key_in_header() {
    let (_tracing, handle, addr) = start_service();
    let resp = send_request(&addr, "GET /secret HTTP/1.1\r\n...");
    let status = parse_status(&resp);
    assert_eq!(status, 200);
    handle.stop();  // ❌ Must remember!
}
```

**After**:
```rust
#[test]
fn test_api_key_in_header() {
    let server = SecurityTestServer::new();
    let resp = send_request(&server.addr(), "GET /secret HTTP/1.1\r\n...");
    let status = parse_status(&resp);
    assert_eq!(status, 200);
    // ✅ Automatic cleanup!
}
```

### Testing the Fixtures

```rust
#[test]
fn test_fixture_cleans_up_on_panic() {
    let result = std::panic::catch_unwind(|| {
        let _server = PetStoreTestServer::new();
        panic!("Simulated test panic");
    });
    
    assert!(result.is_err());
    // Server should be cleaned up even though we panicked
    // Verify by checking no leaked resources
}
```

### 10. ✅ `spec_tests.rs` - Spec Parsing Tests

**Status:** ✅ **COMPLETED** - Refactored to use `tempfile::NamedTempFile`

**Resources:**
- Temporary OpenAPI spec files (YAML, JSON)

**Previous Approach:**
- Custom `write_temp()` function created files
- **Never cleaned up** - leaked files to `/tmp`

**New Approach:**
All tests now use `tempfile::NamedTempFile`:

```rust
#[test]
fn test_load_spec_yaml_and_json() {
    use std::io::Write;
    
    // YAML spec - automatic cleanup via RAII
    let mut yaml_temp = tempfile::NamedTempFile::new().unwrap();
    yaml_temp.write_all(YAML_SPEC.as_bytes()).unwrap();
    yaml_temp.flush().unwrap();
    let (routes_yaml, slug_yaml) = load_spec(yaml_temp.path()).unwrap();
    
    // JSON spec - automatic cleanup via RAII
    let mut json_temp = tempfile::NamedTempFile::new().unwrap();
    json_temp.write_all(json_str.as_bytes()).unwrap();
    json_temp.flush().unwrap();
    let (routes_json, slug_json) = load_spec(json_temp.path()).unwrap();
    
    // Tests...
    
    // Temp files automatically cleaned up when variables drop
}
```

**Tests Refactored:**
- `test_load_spec_yaml_and_json` - 2 temp files, now auto-cleaned
- `test_missing_operation_id_exits` - 1 temp file, now auto-cleaned  
- `test_unsupported_method_ignored` - 1 temp file, now auto-cleaned
- `test_sse_spec_loading` - Already using `NamedTempFile` ✅

**Benefits:**
- ✅ No more `/tmp` pollution
- ✅ Consistent with `test_sse_spec_loading`
- ✅ Removed custom `write_temp()` helper and its dependencies
- ✅ Cleaner, more idiomatic code
- ✅ Automatic cleanup even on test failure/panic

**Files Modified:**
- Removed: `TEMP_COUNTER`, `TEMP_LOCK`, `write_temp()` function
- Updated: All 4 tests to use `tempfile::NamedTempFile`

## Signal Handling for Static Resources

### Problem: Static Resources and SIGINT

When resources are stored in static `OnceLock` (common for shared test fixtures), the `Drop` trait is **not sufficient** for cleanup when tests are interrupted:

```rust
static HARNESS: OnceLock<ContainerHarness> = OnceLock::new();

pub fn base_url() -> &'static str {
    let h = HARNESS.get_or_init(ContainerHarness::start);
    h.base_url.as_str()
}
```

**Why Drop Doesn't Work:**
1. Static variables have `'static` lifetime
2. They exist until process termination
3. When SIGINT is received (Ctrl+C), the process exits immediately
4. The static `OnceLock` never goes out of scope
5. The `Drop` implementation is **never called**

**Observed Issue:**
- Running `just nt` (nextest)
- Pressing Ctrl+C
- Docker containers left running
- Next test run hangs for 60+ seconds trying to use the same container name

### Solution: POSIX Signal Handling

For static resources that **must** be cleaned up on interruption, register signal handlers:

```rust
use std::sync::atomic::{AtomicBool, Ordering};

/// Flag to prevent recursive cleanup
static SIGNAL_CLEANUP_RUNNING: AtomicBool = AtomicBool::new(false);

fn register_signal_handlers() {
    extern "C" fn signal_handler(_: libc::c_int) {
        // Prevent recursive cleanup if multiple signals arrive
        if SIGNAL_CLEANUP_RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }
        
        eprintln!("\n🛑 Signal received - cleaning up Docker containers...");
        cleanup_orphaned_containers();
        eprintln!("✓ Cleanup complete");
        
        // Re-raise the signal to allow normal termination
        unsafe {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
            libc::raise(libc::SIGINT);
        }
    }
    
    unsafe {
        libc::signal(libc::SIGINT, signal_handler as libc::sighandler_t);
        libc::signal(libc::SIGTERM, signal_handler as libc::sighandler_t);
    }
}

// Register once on first use
static CLEANUP_REGISTERED: OnceLock<()> = OnceLock::new();

pub fn base_url() -> &'static str {
    CLEANUP_REGISTERED.get_or_init(|| {
        register_signal_handlers();
    });
    
    let h = HARNESS.get_or_init(ContainerHarness::start);
    h.base_url.as_str()
}
```

### Implementation in BRRTRouter

**File:** `tests/curl_harness.rs`

The Docker integration tests use signal handling to ensure cleanup even when `just nt` is interrupted:

1. **Signal Handler Registration**: Registers SIGINT/SIGTERM handlers on first test
2. **Cleanup Callback**: Calls `cleanup_orphaned_containers()` before process exit
3. **Aggressive Cleanup**: Always cleanup on container start (defense in depth)

**Benefits:**
- ✅ No orphaned containers on Ctrl+C
- ✅ Immediate feedback to users
- ✅ Fast test iteration (no manual cleanup needed)
- ✅ Works with nextest's parallel execution

**Testing:**
```bash
# Start tests
just nt curl

# Press Ctrl+C
# You should see:
# 🛑 Signal received - cleaning up Docker containers...
# ✓ Removed container: brrtrouter-e2e-12345
# ✓ Cleanup complete

# Verify cleanup
docker ps -a | grep brrtrouter-e2e
# (should return nothing)
```

### When to Use Signal Handling

Use signal handlers when **all** of these are true:

1. ✅ Resources stored in static `OnceLock` or `lazy_static!`
2. ✅ Cleanup is **critical** (e.g., Docker containers, network ports, file locks)
3. ✅ Tests may be interrupted with SIGINT (Ctrl+C)
4. ✅ Normal `Drop` is insufficient

**Don't use signal handlers when:**
- ❌ Resources are test-local (regular RAII with `Drop` is sufficient)
- ❌ Cleanup is non-critical (e.g., temporary files cleaned by OS)
- ❌ Resources are managed by external systems (e.g., Kubernetes pods)

### Dependencies

```toml
[dev-dependencies]
libc = "0.2"  # For signal handling in tests (SIGINT cleanup)
```

## Related Documentation

- `docs/SIGINT_CLEANUP_FIX.md` - Detailed explanation of signal handling fix
- `docs/MEMORY_LEAK_FIX.md` - Why proper teardown prevents leaks
- `docs/TEST_DOCUMENTATION.md` - Overview of all test modules
- `tests/server_tests.rs` - Implementation examples
- `tests/spec_tests.rs` - Temporary file cleanup example

