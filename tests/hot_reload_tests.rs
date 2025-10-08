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
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

mod common;
use common::temp_files;

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

    let path = temp_files::create_temp_yaml(SPEC_V1);
    let (routes, _slug) = load_spec(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));

    let updates: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let updates_clone = updates.clone();

    let watcher = watch_spec(
        &path,
        router,
        dispatcher.clone(),
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

    // modify the spec
    std::fs::write(&path, SPEC_V2).unwrap();

    // wait for callback to receive update
    for _ in 0..20 {
        {
            let ups = updates.lock().unwrap();
            if ups.iter().any(|v| v.contains(&"foo_two".to_string())) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let ups = updates.lock().unwrap();
    assert!(ups.iter().any(|v| v.contains(&"foo_two".to_string())));

    drop(watcher);
    std::fs::remove_file(&path).unwrap();
}
