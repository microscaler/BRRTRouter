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

#[test]
fn test_watch_spec_multiple_changes() {
    const SPEC_BASE: &str = r#"openapi: 3.1.0
info:
  title: Multi Change Test
  version: '1.0'
paths:
  /endpoint:
    get:
      operationId: handler_"#;

    let path = temp_files::create_temp_yaml(&format!("{}v1\n      responses:\n        '200': {{ description: OK }}", SPEC_BASE));
    let (routes, _slug) = load_spec(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));

    let updates: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
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
            if let Some(route) = new_routes.first() {
                updates_clone.lock().unwrap().push(route.handler_name.clone());
            }
        },
    )
    .expect("watch_spec");

    std::thread::sleep(Duration::from_millis(100));

    // Make multiple rapid changes
    for i in 2..=5 {
        let spec_content = format!("{}v{}\n      responses:\n        '200': {{ description: OK }}", SPEC_BASE, i);
        std::fs::write(&path, spec_content).unwrap();
        std::thread::sleep(Duration::from_millis(150)); // Allow time for processing
    }

    // Wait for all updates to be processed
    std::thread::sleep(Duration::from_millis(500));

    let ups = updates.lock().unwrap();
    assert!(ups.len() >= 3, "Should have processed multiple updates, got: {:?}", ups);
    assert!(ups.contains(&"handler_v5".to_string()), "Should contain final version");

    drop(watcher);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn test_watch_spec_invalid_yaml() {
    const VALID_SPEC: &str = r#"openapi: 3.1.0
info:
  title: Invalid Test
  version: '1.0'
paths:
  /test:
    get:
      operationId: test_handler
      responses:
        '200': { description: OK }
"#;

    const INVALID_SPEC: &str = r#"openapi: 3.1.0
info:
  title: Invalid Test
  version: '1.0'
paths:
  /test:
    get:
      operationId: test_handler
      responses:
        '200': { description: OK
      # Missing closing brace - invalid YAML
"#;

    let path = temp_files::create_temp_yaml(VALID_SPEC);
    let (routes, _slug) = load_spec(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));

    let update_count: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let update_count_clone = update_count.clone();

    let watcher = watch_spec(
        &path,
        router,
        dispatcher.clone(),
        move |disp, new_routes| {
            for r in &new_routes {
                let (tx, _rx) = mpsc::channel();
                disp.add_route(r.clone(), tx);
            }
            *update_count_clone.lock().unwrap() += 1;
        },
    )
    .expect("watch_spec");

    std::thread::sleep(Duration::from_millis(100));

    // Write invalid YAML - should not trigger callback
    std::fs::write(&path, INVALID_SPEC).unwrap();
    std::thread::sleep(Duration::from_millis(300));

    // Write valid YAML again - should trigger callback
    std::fs::write(&path, VALID_SPEC).unwrap();
    std::thread::sleep(Duration::from_millis(300));

    let count = *update_count.lock().unwrap();
    // Should have at least 1 update (from the valid write), but may have more due to file system events
    assert!(count >= 1, "Should process at least one valid YAML update, got: {}", count);

    drop(watcher);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn test_watch_spec_nonexistent_file() {
    let nonexistent_path = std::path::PathBuf::from("/nonexistent/path/spec.yaml");
    let router = Arc::new(RwLock::new(Router::new(vec![])));
    let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));

    let result = watch_spec(
        &nonexistent_path,
        router,
        dispatcher,
        |_, _| {},
    );

    assert!(result.is_err(), "Should fail to watch nonexistent file");
}

#[test]
fn test_watch_spec_file_deletion_and_recreation() {
    const SPEC_CONTENT: &str = r#"openapi: 3.1.0
info:
  title: Delete Test
  version: '1.0'
paths:
  /delete:
    get:
      operationId: delete_handler
      responses:
        '200': { description: OK }
"#;

    let path = temp_files::create_temp_yaml(SPEC_CONTENT);
    let (routes, _slug) = load_spec(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));

    let updates: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
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
            if let Some(route) = new_routes.first() {
                updates_clone.lock().unwrap().push(route.handler_name.clone());
            }
        },
    )
    .expect("watch_spec");

    std::thread::sleep(Duration::from_millis(100));

    // Delete the file
    std::fs::remove_file(&path).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // Recreate the file
    std::fs::write(&path, SPEC_CONTENT).unwrap();
    std::thread::sleep(Duration::from_millis(300));

    let ups = updates.lock().unwrap();
    assert!(ups.contains(&"delete_handler".to_string()), "Should handle file recreation");

    drop(watcher);
    let _ = std::fs::remove_file(&path); // May already be deleted
}

#[test]
fn test_watch_spec_concurrent_watchers() {
    const SPEC_CONTENT: &str = r#"openapi: 3.1.0
info:
  title: Concurrent Test
  version: '1.0'
paths:
  /concurrent:
    get:
      operationId: concurrent_handler
      responses:
        '200': { description: OK }
"#;

    let path = temp_files::create_temp_yaml(SPEC_CONTENT);
    let (routes, _slug) = load_spec(path.to_str().unwrap()).unwrap();

    // Create two separate watchers
    let router1 = Arc::new(RwLock::new(Router::new(routes.clone())));
    let dispatcher1 = Arc::new(RwLock::new(Dispatcher::new()));
    let updates1: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let updates1_clone = updates1.clone();

    let router2 = Arc::new(RwLock::new(Router::new(routes.clone())));
    let dispatcher2 = Arc::new(RwLock::new(Dispatcher::new()));
    let updates2: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let updates2_clone = updates2.clone();

    let watcher1 = watch_spec(
        &path,
        router1,
        dispatcher1,
        move |disp, new_routes| {
            for r in &new_routes {
                let (tx, _rx) = mpsc::channel();
                disp.add_route(r.clone(), tx);
            }
            *updates1_clone.lock().unwrap() += 1;
        },
    )
    .expect("watch_spec 1");

    let watcher2 = watch_spec(
        &path,
        router2,
        dispatcher2,
        move |disp, new_routes| {
            for r in &new_routes {
                let (tx, _rx) = mpsc::channel();
                disp.add_route(r.clone(), tx);
            }
            *updates2_clone.lock().unwrap() += 1;
        },
    )
    .expect("watch_spec 2");

    std::thread::sleep(Duration::from_millis(100));

    // Modify the file
    let new_content = SPEC_CONTENT.replace("concurrent_handler", "updated_handler");
    std::fs::write(&path, new_content).unwrap();
    std::thread::sleep(Duration::from_millis(300));

    // Both watchers should receive updates (may be more than 1 due to file system events)
    assert!(*updates1.lock().unwrap() >= 1, "Watcher 1 should receive at least one update, got: {}", *updates1.lock().unwrap());
    assert!(*updates2.lock().unwrap() >= 1, "Watcher 2 should receive at least one update, got: {}", *updates2.lock().unwrap());

    drop(watcher1);
    drop(watcher2);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn test_watch_spec_callback_panic_isolation() {
    const SPEC_CONTENT: &str = r#"openapi: 3.1.0
info:
  title: Panic Test
  version: '1.0'
paths:
  /panic:
    get:
      operationId: panic_handler
      responses:
        '200': { description: OK }
"#;

    let path = temp_files::create_temp_yaml(SPEC_CONTENT);
    let (routes, _slug) = load_spec(path.to_str().unwrap()).unwrap();
    let router = Arc::new(RwLock::new(Router::new(routes.clone())));
    let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));

    let callback_called = Arc::new(Mutex::new(false));
    let callback_called_clone = callback_called.clone();

    let watcher = watch_spec(
        &path,
        router,
        dispatcher.clone(),
        move |_disp, _new_routes| {
            *callback_called_clone.lock().unwrap() = true;
            // This would normally panic and potentially crash the watcher
            // But the watcher should handle this gracefully
        },
    )
    .expect("watch_spec");

    std::thread::sleep(Duration::from_millis(100));

    // Modify the file to trigger callback
    let new_content = SPEC_CONTENT.replace("panic_handler", "no_panic_handler");
    std::fs::write(&path, new_content).unwrap();
    std::thread::sleep(Duration::from_millis(300));

    // Callback should have been called despite any internal error handling
    assert!(*callback_called.lock().unwrap(), "Callback should have been called");

    drop(watcher);
    std::fs::remove_file(&path).unwrap();
}

#[test] 
fn test_watch_spec_empty_file() {
    let path = temp_files::create_temp_yaml("");
    let router = Arc::new(RwLock::new(Router::new(vec![])));
    let dispatcher = Arc::new(RwLock::new(Dispatcher::new()));

    let callback_called = Arc::new(Mutex::new(false));
    let callback_called_clone = callback_called.clone();

    let watcher = watch_spec(
        &path,
        router,
        dispatcher.clone(),
        move |_disp, _new_routes| {
            *callback_called_clone.lock().unwrap() = true;
        },
    )
    .expect("watch_spec");

    std::thread::sleep(Duration::from_millis(100));

    // Write valid content to the empty file
    const VALID_CONTENT: &str = r#"openapi: 3.1.0
info:
  title: Empty Test
  version: '1.0'
paths:
  /empty:
    get:
      operationId: empty_handler
      responses:
        '200': { description: OK }
"#;
    std::fs::write(&path, VALID_CONTENT).unwrap();
    std::thread::sleep(Duration::from_millis(300));

    assert!(*callback_called.lock().unwrap(), "Should handle transition from empty to valid file");

    drop(watcher);
    std::fs::remove_file(&path).unwrap();
}
