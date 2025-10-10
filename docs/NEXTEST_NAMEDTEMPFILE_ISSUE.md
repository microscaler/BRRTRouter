# Nextest + NamedTempFile Incompatibility

## Problem Discovered

`tempfile::NamedTempFile` works fine with `cargo test` but **FAILS with nextest**!

### Symptoms

- Tests pass with `cargo test`
- Same tests fail with `cargo nextest run` or `just nt`
- Error: `expected value at line 1 column 1` (serde_yaml error for empty file)

### Root Cause

When using `NamedTempFile`:
1. Create temp file with open handle
2. Write to the handle
3. Flush the handle
4. Try to read from the path

**In `cargo test`:** Works fine  
**In nextest:** File appears empty or locked

### Why Nextest Behaves Differently

Nextest runs each test in a separate process with different:
- File handle inheritance
- Buffer flushing behavior
- File locking semantics

The open file handle in `NamedTempFile` causes issues when another part of the code tries to read from the path.

## Solution

**Use manual file management instead of `NamedTempFile`:**

```rust
// ❌ BROKEN in nextest
let mut temp = tempfile::NamedTempFile::new().unwrap();
temp.write_all(content.as_bytes()).unwrap();
temp.flush().unwrap();
load_spec(temp.path().to_str().unwrap()).unwrap(); // ERROR: empty file!

// ✅ WORKS in both cargo test and nextest
let path = std::env::temp_dir().join(format!(
    "test_{}_{}.yaml",
    std::process::id(),
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
));
std::fs::write(&path, content.as_bytes()).unwrap();
load_spec(path.to_str().unwrap()).unwrap(); // Works!
let _ = std::fs::remove_file(&path); // Manual cleanup
```

## Files Fixed

1. **`tests/spec_tests.rs`** - 4 tests fixed
   - `test_load_spec_yaml_and_json`
   - `test_missing_operation_id_exits`
   - `test_unsupported_method_ignored`
   - `test_sse_spec_loading`

2. **`tests/hot_reload_tests.rs`** - 1 test fixed
   - `test_watch_spec_reload`

## Key Differences

| Aspect | NamedTempFile | Manual File |
|--------|---------------|-------------|
| **File Handle** | Kept open | Closed immediately |
| **cargo test** | ✅ Works | ✅ Works |
| **nextest** | ❌ Fails | ✅ Works |
| **RAII Cleanup** | Automatic | Manual (via Drop or direct call) |
| **Complexity** | Simple API | More code |
| **Reliability** | Depends on runner | Always works |

## When to Use Each

### Use Manual File Management When:
- ✅ File needs to be read by external code
- ✅ File needs to be watched (hot reload)
- ✅ Tests run with nextest
- ✅ File is passed to subprocesses
- ✅ Multiple processes access the file

### Use NamedTempFile When:
- ✅ Only writing to the file
- ✅ Never reading the path
- ✅ Single-process usage
- ✅ NOT running with nextest
- ⚠️ Be careful!

## Best Practice for BRRTRouter

**Always use manual file management in tests** to ensure compatibility with both:
- `cargo test` (single-process)
- `cargo nextest run` (multi-process)
- `just nt` (uses nextest)

## Pattern to Follow

```rust
// At start of test
let temp_path = std::env::temp_dir().join(format!(
    "brrtrouter_test_{}_{}.ext",
    std::process::id(),
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
));
std::fs::write(&temp_path, content).unwrap();

// ... use temp_path ...

// At end of test (or in Drop for RAII)
let _ = std::fs::remove_file(&temp_path);
```

## Alternative: Use Test Fixtures

For cleaner code, use RAII fixtures like `HotReloadTestFixture`:

```rust
struct HotReloadTestFixture {
    path: PathBuf,
}

impl HotReloadTestFixture {
    fn new(content: &str) -> Self {
        let path = std::env::temp_dir().join(format!("test_{}_{}.yaml", ...));
        std::fs::write(&path, content).unwrap();
        Self { path }
    }
}

impl Drop for HotReloadTestFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

// Usage
let fixture = HotReloadTestFixture::new(content);
// ... use fixture.path ...
// Automatic cleanup when fixture drops
```

## Verification

All tests now pass with both:
```bash
# Single-process runner
cargo test --test spec_tests
cargo test --test hot_reload_tests

# Multi-process runner (nextest)
cargo nextest run --test spec_tests
cargo nextest run --test hot_reload_tests
just nt  # Uses nextest
```

## Lesson Learned

**DO NOT assume test infrastructure works the same across runners!**

Always test with the actual CI/CD runner being used:
- If using nextest in CI → test locally with nextest
- Don't just rely on `cargo test` passing

---

**Status:** ✅ Fixed  
**Tests Affected:** 5  
**Root Cause:** File handle semantics differ between cargo test and nextest  
**Solution:** Manual file management instead of NamedTempFile  
**Verified:** Must test with actual runner (`just nt`)


