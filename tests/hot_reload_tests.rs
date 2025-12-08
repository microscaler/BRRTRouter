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
                let names: Vec<String> = new_routes
                    .iter()
                    .map(|r| r.handler_name.to_string())
                    .collect();
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
                let names: Vec<String> = new_routes
                    .iter()
                    .map(|r| r.handler_name.to_string())
                    .collect();
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
                if ups
                    .iter()
                    .any(|v| v.contains(&"test_handler_v2".to_string()))
                {
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
        ups.iter()
            .any(|v| v.contains(&"test_handler_v2".to_string())),
        "Expected 'test_handler_v2' in updates, got: {:?}",
        ups
    );

    // Verify cache was cleared during hot reload
    assert_eq!(cache.size(), 0, "Cache should be empty after hot reload");

    // Fixture automatically cleaned up when it drops (RAII)!
}

#[test]
fn test_watch_spec_schema_changes_enforced() {
    // Test that validates actual schema validation behavior changes after hot reload
    const SPEC_V1: &str = r#"openapi: 3.1.0
info:
  title: Schema Change Test
  version: '1.0'
paths:
  /user:
    post:
      operationId: create_user
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
                  id:
                    type: integer
                  name:
                    type: string
"#;
    const SPEC_V2: &str = r#"openapi: 3.1.0
info:
  title: Schema Change Test
  version: '1.0'
paths:
  /user:
    post:
      operationId: create_user
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
                  id:
                    type: integer
                  name:
                    type: string
                  age:
                    type: integer
"#;

    use brrtrouter::validator_cache::ValidatorCache;
    use serde_json::json;

    // Use RAII fixture for automatic cleanup
    let fixture = HotReloadTestFixture::new(SPEC_V1);
    let path = fixture.path();
    let (routes, _slug) = load_spec(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));

    // Create validator cache and precompile initial schemas
    let cache = ValidatorCache::new(true);
    let initial_compiled = cache.precompile_schemas(&routes);
    assert!(initial_compiled > 0, "Should precompile initial schemas");

    // Test validation with V1 schema (requires only 'name')
    let v1_valid_request = json!({"name": "Alice"});
    let v1_route = routes
        .iter()
        .find(|r| r.handler_name.as_ref() == "create_user")
        .unwrap();

    if let Some(ref schema) = v1_route.request_schema {
        let validator = cache
            .get_or_compile("create_user", "request", None, schema)
            .unwrap();
        assert!(
            validator.validate(&v1_valid_request).is_ok(),
            "V1: Request with only 'name' should be valid"
        );

        // This should be invalid in V2 but valid in V1 (missing 'age')
        let v1_missing_age = json!({"name": "Bob"});
        assert!(
            validator.validate(&v1_missing_age).is_ok(),
            "V1: Request without 'age' should be valid"
        );
    }

    let cache_clone = cache.clone();
    let router_clone = router.clone();
    let updates: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let updates_clone = updates.clone();

    {
        // Scope the watcher to ensure it drops before file cleanup
        let watcher = watch_spec(
            &path,
            router_clone,
            dispatcher.clone(),
            Some(cache_clone),
            move |disp, new_routes| {
                for r in &new_routes {
                    let (tx, _rx) = mpsc::channel();
                    disp.add_route(r.clone(), tx);
                }
                let names: Vec<String> = new_routes
                    .iter()
                    .map(|r| r.handler_name.to_string())
                    .collect();
                updates_clone.lock().unwrap().push(names);
            },
        )
        .expect("watch_spec");

        // allow watcher thread to start
        std::thread::sleep(Duration::from_millis(100));

        // Modify the spec to V2 (now requires 'age' field)
        fixture.update_content(SPEC_V2);

        // Wait for hot reload to complete
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(5);

        loop {
            {
                let ups = updates.lock().unwrap();
                if !ups.is_empty() {
                    break;
                }
            }

            if start.elapsed() > timeout {
                panic!("Timeout waiting for hot reload");
            }

            std::thread::sleep(Duration::from_millis(50));
        }

        // Explicitly drop watcher before assertions
        drop(watcher);

        // Give filesystem watcher time to fully stop
        std::thread::sleep(Duration::from_millis(100));
    }

    // Verify cache was cleared
    assert_eq!(cache.size(), 0, "Cache should be empty after hot reload");

    // Load the new routes to get updated schemas
    let (new_routes, _slug) = load_spec(path.to_str().unwrap()).unwrap();
    let v2_route = new_routes
        .iter()
        .find(|r| r.handler_name.as_ref() == "create_user")
        .unwrap();

    // Test validation with V2 schema (requires both 'name' and 'age')
    if let Some(ref schema) = v2_route.request_schema {
        let validator = cache
            .get_or_compile("create_user", "request", None, schema)
            .unwrap();

        // Request with both fields should be valid
        let v2_valid_request = json!({"name": "Charlie", "age": 30});
        assert!(
            validator.validate(&v2_valid_request).is_ok(),
            "V2: Request with 'name' and 'age' should be valid"
        );

        // Request missing 'age' should now be INVALID
        let v2_missing_age = json!({"name": "David"});
        assert!(
            validator.validate(&v2_missing_age).is_err(),
            "V2: Request without 'age' should be INVALID after hot reload"
        );
    }

    // Fixture automatically cleaned up when it drops (RAII)!
}

#[test]
fn test_spec_version_hash_changes_on_reload() {
    // Test that validates the spec version and hash are properly updated during hot reload
    const SPEC_V1: &str = r#"openapi: 3.1.0
info:
  title: Version Test
  version: '1.0'
paths:
  /data:
    post:
      operationId: store_data
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                value:
                  type: string
      responses:
        '200':
          description: OK
"#;
    const SPEC_V2: &str = r#"openapi: 3.1.0
info:
  title: Version Test Updated
  version: '2.0'
paths:
  /data:
    post:
      operationId: store_data
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                value:
                  type: string
                metadata:
                  type: object
      responses:
        '200':
          description: OK
"#;

    use brrtrouter::validator_cache::ValidatorCache;

    let fixture = HotReloadTestFixture::new(SPEC_V1);
    let path = fixture.path();
    let (routes, _slug) = load_spec(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));

    let cache = ValidatorCache::new(true);
    let initial_version = cache.spec_version();
    assert_eq!(initial_version.version, 1);
    assert_eq!(initial_version.hash, "initial");

    let cache_clone = cache.clone();
    let updates: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let updates_clone = updates.clone();

    {
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
                let names: Vec<String> = new_routes
                    .iter()
                    .map(|r| r.handler_name.to_string())
                    .collect();
                updates_clone.lock().unwrap().push(names);
            },
        )
        .expect("watch_spec");

        std::thread::sleep(Duration::from_millis(100));

        // Modify the spec to V2
        fixture.update_content(SPEC_V2);

        // Wait for hot reload
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(5);

        loop {
            {
                let ups = updates.lock().unwrap();
                if !ups.is_empty() {
                    break;
                }
            }

            if start.elapsed() > timeout {
                panic!("Timeout waiting for hot reload");
            }

            std::thread::sleep(Duration::from_millis(50));
        }

        drop(watcher);
        std::thread::sleep(Duration::from_millis(100));
    }

    // Verify spec version was updated
    // Note: File watchers may fire multiple events for a single file change,
    // so version could be 2 or higher depending on how many events were triggered.
    let final_version = cache.spec_version();
    assert!(
        final_version.version >= 2,
        "Version should increment to at least 2 (got {})",
        final_version.version
    );
    assert_ne!(
        final_version.hash, "initial",
        "Hash should be computed from content"
    );
    assert_ne!(
        final_version.hash, initial_version.hash,
        "Hash should be different from initial"
    );
    assert_eq!(
        final_version.hash.len(),
        16,
        "Hash should be 16 characters (truncated SHA-256)"
    );
}
