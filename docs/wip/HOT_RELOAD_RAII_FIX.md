# Hot Reload Test RAII Fix

## Final Solution: HotReloadTestFixture (Manual File Management)

After issues with `NamedTempFile` (file handle state confusion), we use a simpler approach: manually create and delete a temp file.

```rust
struct HotReloadTestFixture {
    path: PathBuf,
}

impl HotReloadTestFixture {
    fn new(initial_content: &str) -> Self {
        // Create unique temp file path
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "brrtrouter_hot_reload_test_{}_{}.yaml",
            std::process::id(),
            nanos
        ));
        
        // Write initial content (plain file, no handle)
        std::fs::write(&path, initial_content.as_bytes()).unwrap();
        
        Self { path }
    }
    
    fn path(&self) -> &PathBuf { &self.path }
    
    fn update_content(&self, new_content: &str) {
        std::fs::write(&self.path, new_content.as_bytes()).unwrap();
    }
}

impl Drop for HotReloadTestFixture {
    fn drop(&mut self) {
        // Cleanup on drop (RAII)
        let _ = std::fs::remove_file(&self.path);
    }
}
```

**Why Not `NamedTempFile`?**
- `NamedTempFile` keeps an open file handle
- Reading/writing the path while handle is open causes issues
- For hot reload tests, we need to freely read/write the file
- Manual file management is simpler and more predictable

**Rust's `with` Equivalent:**
The answer to "what is Rust's equivalent to Python's `with`?" is **RAII + Drop trait**. This fixture demonstrates it:
- Resource acquired in `new()` (Python's `__enter__`)
- Resource released in `drop()` (Python's `__exit__`)
- Guaranteed cleanup even on panic!

This is consistent with other test fixtures like `CliTestFixture`, `ProjectTestFixture`, etc.

---

## Problem (Initial Attempt)

When first converting `hot_reload_tests.rs` to use `tempfile::NamedTempFile`, the file was being deleted too early:

```rust
// ❌ BROKEN: temp dropped immediately, file deleted
let mut temp = tempfile::NamedTempFile::new().unwrap();
temp.write_all(SPEC_V1.as_bytes()).unwrap();
temp.flush().unwrap();
let path = temp.path().to_path_buf();  // Copy path
// temp drops here! File deleted before watcher can use it!

let (routes, _slug) = load_spec(path.to_str().unwrap()).unwrap();
// ERROR: File doesn't exist anymore!
```

**Error:**
```
thread 'test_watch_spec_reload' panicked at tests/hot_reload_tests.rs:69:61:
called `Result::unwrap()` on an `Err` value: expected value at line 1 column 1
```

## Root Cause

`NamedTempFile` has `Drop` implemented to delete the file when it goes out of scope. When we:
1. Create the temp file
2. Get a PathBuf copy of the path
3. Let temp go out of scope

The file is deleted immediately, but the test still needs it for:
- Initial `load_spec()` call
- Hot reload watcher monitoring the file
- Writing updated content during the test

## Solution

Keep the `NamedTempFile` alive for the entire test duration:

```rust
// ✅ FIXED: Keep temp alive through entire test
let temp = tempfile::NamedTempFile::new().unwrap();
let path = temp.path().to_path_buf();
std::fs::write(&path, SPEC_V1.as_bytes()).unwrap();  // Write directly to path

// ... entire test ...

// temp stays alive until here, then automatically cleans up
drop(temp);
```

**Key Changes:**
1. Don't use `temp.write_all()` which would close the file
2. Use `std::fs::write(&path, ...)` to write to the path
3. Keep `temp` variable in scope through entire test
4. Explicit `drop(temp)` at the end for clarity

## Why This Works

### NamedTempFile Lifecycle
- `NamedTempFile::new()` creates file and returns handle
- File exists as long as handle exists
- `drop(temp)` deletes the file

### Test Requirements
1. **Load spec initially**: File must exist ✅
2. **Start watcher**: File must exist and be watchable ✅
3. **Modify file**: Need to write to path ✅
4. **Watcher detects change**: File must still exist ✅
5. **Cleanup**: File should be deleted after test ✅

By keeping `temp` in scope, all requirements are met.

## Code Flow

```rust
fn test_watch_spec_reload() {
    // Create temp file (handle kept alive)
    let temp = tempfile::NamedTempFile::new().unwrap();
    let path = temp.path().to_path_buf();
    
    // Write initial content
    std::fs::write(&path, SPEC_V1.as_bytes()).unwrap();
    
    // Load initial spec (file exists ✅)
    let (routes, _slug) = load_spec(path.to_str().unwrap()).unwrap();
    
    // Start watcher (file exists ✅)
    let watcher = watch_spec(&path, ...).expect("watch_spec");
    
    // Modify file for hot reload test (file exists ✅)
    std::fs::write(&path, SPEC_V2.as_bytes()).unwrap();
    
    // Wait for change detection (file exists ✅)
    // ... polling loop ...
    
    // Stop watcher
    drop(watcher);
    
    // Assertions
    // ...
    
    // Cleanup (file deleted ✅)
    drop(temp);
}
```

## Alternative Approaches Considered

### 1. Use `persist()` (Not Ideal)
```rust
let (file, path) = temp.persist().unwrap();
// File persists even after handle drop
// Need manual cleanup: std::fs::remove_file(&path)
```
**Downside:** Loses RAII benefit, manual cleanup required.

### 2. Use `keep()` (Deprecated)
```rust
let path = temp.keep().unwrap();
```
**Downside:** Deprecated API, discouraged.

### 3. Scope Management (Chosen Solution ✅)
```rust
let temp = ...;  // Keep alive through test
// ... test ...
drop(temp);  // Automatic cleanup
```
**Upside:** RAII still works, file exists when needed, auto cleanup.

## Testing

```bash
# Should pass now
cargo test --test hot_reload_tests test_watch_spec_reload

# Verify no temp files leaked
ls /tmp/brrtrouter* 2>/dev/null | wc -l  # Should be 0
```

## Related Issues

This pattern applies to any test where:
- A temporary file must exist for duration of test
- File needs to be monitored/watched
- File content is modified during test
- Cleanup must still happen

## Files Modified

- `tests/hot_reload_tests.rs` - Fixed temp file lifetime management

## Status

✅ **FIXED** - Test now passes, no resource leaks


