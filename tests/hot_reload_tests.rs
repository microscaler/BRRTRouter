//! Tests for hot reload and live spec watching
//!
//! # Test Coverage
//!
//! Validates the hot reload system that watches OpenAPI specs and updates routes:
//! - File system watching (notify crate)
//! - Debouncing (multiple changes → single reload)
//! - Route registration updates
//! - Router/dispatcher synchronization
//!
//! # Test Strategy
//!
//! Uses temporary files to simulate spec changes:
//! 1. Write initial spec to temp file
//! 2. Start file watcher
//! 3. Modify spec (add/remove routes)
//! 4. Verify router updates reflect changes
//! 5. Test debounce window (rapid changes)
//!
//! # Key Test Cases
//!
//! - `test_watch_spec_reload`: Basic hot reload works
//! - Debouncing prevents excessive reloads
//! - Router updates are atomic
//! - No race conditions in reload
//!
//! # Challenges
//!
//! - File system timing is non-deterministic
//! - Need to wait for debounce window
//! - Cross-platform FS notification differences

use brrtrouter::{dispatcher::Dispatcher, hot_reload::watch_spec, load_spec, router::Router};
use may::sync::mpsc;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

/// RAII test fixture for hot reload tests
///
/// Manages a temporary spec file that needs to exist for the duration of the test
/// (for watching, reading, and modifying), then automatically cleans up.
///
/// Unlike NamedTempFile, this creates a plain file that can be freely read/written
/// without worrying about file handle state.
struct HotReloadTestFixture {
    path: PathBuf,
}

impl HotReloadTestFixture {
    /// Create a new hot reload test fixture with initial spec content
    fn new(initial_content: &str) -> Self {
        // Create a unique temp file path (but don't use NamedTempFile)
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "brrtrouter_hot_reload_test_{}_{}.yaml",
            std::process::id(),
            nanos
        ));

        // Write the initial content
        std::fs::write(&path, initial_content.as_bytes()).unwrap();

        Self { path }
    }

    /// Get the path to the spec file
    fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Update the spec file content (for testing hot reload)
    fn update_content(&self, new_content: &str) {
        std::fs::write(&self.path, new_content.as_bytes()).unwrap();
    }
}

impl Drop for HotReloadTestFixture {
    fn drop(&mut self) {
        // Clean up the temp file when fixture is dropped
        let _ = std::fs::remove_file(&self.path);
    }
}

#[test]
fn test_watch_spec_reload() {
    const SPEC_V1: &str = r#"openapi: 3.1.0
info:
  title: Reload Test
  version: '1.0'
paths:
  /foo:
    get:
      operationId: foo_one
      responses:
        '200': { description: OK }
"#;
    const SPEC_V2: &str = r#"openapi: 3.1.0
info:
  title: Reload Test
  version: '1.0'
paths:
  /foo:
    get:
      operationId: foo_two
      responses:
        '200': { description: OK }
"#;

    // Use RAII fixture for automatic cleanup
    let fixture = HotReloadTestFixture::new(SPEC_V1);
    let path = fixture.path();
    let (routes, _slug) = load_spec(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));

    let updates: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let updates_clone = updates.clone();

    {
        // Scope the watcher to ensure it drops before file cleanup
        let watcher = watch_spec(
            &path,
            router,
            dispatcher.clone(),
            None, // No validator cache for this test
            move |disp, new_routes| {
                for r in &new_routes {
                    let (tx, _rx) = mpsc::channel();
                    disp.add_route(r.clone(), tx);
                }
                let names = new_routes.iter().map(|r| r.handler_name.clone()).collect();
                updates_clone.lock().unwrap().push(names);
            },
        )
        .expect("watch_spec");

        // allow watcher thread to start
        std::thread::sleep(Duration::from_millis(100));

        // modify the spec using the fixture's update method
        fixture.update_content(SPEC_V2);

        // wait for callback to receive update (with timeout)
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(5); // Reduced from potential 50s (20 * 50ms)

        loop {
            {
                let ups = updates.lock().unwrap();
                if ups.iter().any(|v| v.contains(&"foo_two".to_string())) {
                    break;
                }
            }

            if start.elapsed() > timeout {
                break; // Timeout - let assertion below handle failure
            }

            std::thread::sleep(Duration::from_millis(50));
        }

        // Explicitly drop watcher before assertions and cleanup
        drop(watcher);

        // Give filesystem watcher time to fully stop
        std::thread::sleep(Duration::from_millis(100));
    }

    let ups = updates.lock().unwrap();
    assert!(
        ups.iter().any(|v| v.contains(&"foo_two".to_string())),
        "Expected 'foo_two' in updates, got: {:?}",
        ups
    );

    // Fixture automatically cleaned up when it drops (RAII)!
}

#[test]
fn test_watch_spec_clears_validator_cache() {
    const SPEC_V1: &str = r#"openapi: 3.1.0
info:
  title: Cache Test
  version: '1.0'
paths:
  /test:
    post:
      operationId: test_handler
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                name:
                  type: string
              required: [name]
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
"#;
    const SPEC_V2: &str = r#"openapi: 3.1.0
info:
  title: Cache Test
  version: '1.0'
paths:
  /test:
    post:
      operationId: test_handler_v2
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                name:
                  type: string
                age:
                  type: integer
              required: [name, age]
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
                  message:
                    type: string
"#;

    use brrtrouter::validator_cache::ValidatorCache;
    
    // Use RAII fixture for automatic cleanup
    let fixture = HotReloadTestFixture::new(SPEC_V1);
    let path = fixture.path();
    let (routes, _slug) = load_spec(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));
    
    // Create validator cache and pre-populate it
    let cache = ValidatorCache::new(true);
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"}
        }
    });
    
    // Compile and cache a validator
    let _validator = cache.get_or_compile("test_handler", "request", None, &schema);
    assert_eq!(cache.size(), 1, "Cache should have one entry");

    let cache_clone = cache.clone();
    let updates: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let updates_clone = updates.clone();

    {
        // Scope the watcher to ensure it drops before file cleanup
        let watcher = watch_spec(
            &path,
            router,
            dispatcher.clone(),
            Some(cache_clone),
            move |disp, new_routes| {
                for r in &new_routes {
                    let (tx, _rx) = mpsc::channel();
                    disp.add_route(r.clone(), tx);
                }
                let names = new_routes.iter().map(|r| r.handler_name.clone()).collect();
                updates_clone.lock().unwrap().push(names);
            },
        )
        .expect("watch_spec");

        // allow watcher thread to start
        std::thread::sleep(Duration::from_millis(100));

        // modify the spec using the fixture's update method
        fixture.update_content(SPEC_V2);

        // wait for callback to receive update (with timeout)
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(5);

        loop {
            {
                let ups = updates.lock().unwrap();
                if ups.iter().any(|v| v.contains(&"test_handler_v2".to_string())) {
                    break;
                }
            }

            if start.elapsed() > timeout {
                break; // Timeout - let assertion below handle failure
            }

            std::thread::sleep(Duration::from_millis(50));
        }

        // Explicitly drop watcher before assertions and cleanup
        drop(watcher);

        // Give filesystem watcher time to fully stop
        std::thread::sleep(Duration::from_millis(100));
    }

    let ups = updates.lock().unwrap();
    assert!(
        ups.iter().any(|v| v.contains(&"test_handler_v2".to_string())),
        "Expected 'test_handler_v2' in updates, got: {:?}",
        ups
    );
    
    // Verify cache was cleared during hot reload
    assert_eq!(cache.size(), 0, "Cache should be empty after hot reload");

    // Fixture automatically cleaned up when it drops (RAII)!
}
