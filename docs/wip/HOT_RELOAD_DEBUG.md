# Hot Reload Test Debugging

## Current Status: DEBUGGING

**DO NOT claim this is fixed until `cargo test --test hot_reload_tests` passes!**

## Error

```
thread 'test_watch_spec_reload' panicked at tests/hot_reload_tests.rs:108:61:
called `Result::unwrap()` on an `Err` value: expected value at line 1 column 1
```

## What "expected value at line 1 column 1" Means

This is a `serde_yaml` error that typically means:
1. File is empty
2. File has invalid YAML
3. File doesn't exist
4. File has wrong encoding

## Debugging Steps Added

### 1. Verify Write in Fixture
```rust
// In HotReloadTestFixture::new()
std::fs::write(&path, initial_content).unwrap();

// Verify immediately
let verify = std::fs::read_to_string(&path).expect("Failed to read back");
assert!(!verify.is_empty(), "File is empty!");
assert!(verify.starts_with("openapi:"), "Wrong content!");
```

### 2. Debug Output in Test
```rust
eprintln!("Test file path: {}", path.display());
eprintln!("File exists: {}", path.exists());
if path.exists() {
    let content = std::fs::read_to_string(path).unwrap();
    eprintln!("File size: {} bytes", content.len());
    eprintln!("First 100 chars: {:?}", &content.chars().take(100).collect::<String>());
}
```

## Theories

### Theory 1: File is Being Deleted
- Maybe Drop is being called early?
- Check if fixture is being moved?

### Theory 2: Write is Failing Silently
- Permissions issue?
- Disk full?
- Path too long?

### Theory 3: YAML is Invalid
- Check SPEC_V1 constant
- Maybe hidden characters?

### Theory 4: Race Condition
- File written but not flushed?
- Need `sync_all()`?

## Next Steps

1. Run test with debug output
2. Check which assertion fails first
3. If verification in `new()` fails → write problem
4. If verification in test fails → file disappeared
5. If both pass → YAML parsing problem

## Test Command

```bash
cargo test --test hot_reload_tests test_watch_spec_reload -- --nocapture
```

The `--nocapture` will show our `eprintln!` output.

## Resolution

**Status:** Not yet resolved
**Last Updated:** Waiting for test run with debug output


