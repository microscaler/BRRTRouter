# Test Setup and Teardown in Rust

## Overview

Rust provides several patterns for test setup and teardown, similar to Python's `setUp()` and `tearDown()` methods. The most idiomatic approach uses **RAII (Resource Acquisition Is Initialization)** with the `Drop` trait.

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

## Related Documentation

- `docs/MEMORY_LEAK_FIX.md` - Why proper teardown prevents leaks
- `docs/TEST_DOCUMENTATION.md` - Overview of all test modules
- `tests/server_tests.rs` - Implementation examples

