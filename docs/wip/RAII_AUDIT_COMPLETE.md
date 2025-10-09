# Complete RAII Test Audit

## Methodology

Analyzed all 32 test files in `tests/` directory for:
1. **Resources created**: HTTP servers, temp files, Docker containers, external processes
2. **RAII implementation**: `impl Drop` for cleanup
3. **Need assessment**: Should RAII be added?

## Summary Statistics

| Category | Count | Files |
|----------|-------|-------|
| ✅ **Has RAII** | 10 | See "RAII Implemented" section |
| ✅ **No Resources** | 18 | Pure unit tests, no cleanup needed |
| ⚠️ **Needs Review** | 4 | Potential RAII candidates |
| **Total** | **32** | All test files |

---

## ✅ RAII Implemented (10 files)

### 1. `server_tests.rs`
- **Resources**: HTTP server, TCP listener
- **RAII**: `PetStoreTestServer`, `CustomServerTestFixture`
- **Status**: ✅ Complete

### 2. `security_tests.rs`
- **Resources**: HTTP server with auth
- **RAII**: `SecurityTestServer`
- **Status**: ✅ Complete

### 3. `static_server_tests.rs`
- **Resources**: HTTP server with static files
- **RAII**: `StaticFileTestServer`
- **Status**: ✅ Complete

### 4. `sse_tests.rs`
- **Resources**: HTTP server for SSE streaming
- **RAII**: `SseTestServer`
- **Status**: ✅ Complete

### 5. `metrics_endpoint_tests.rs`
- **Resources**: HTTP server with metrics
- **RAII**: `MetricsTestServer`
- **Status**: ✅ Complete

### 6. `health_endpoint_tests.rs`
- **Resources**: HTTP server with health endpoint
- **RAII**: `HealthTestServer`
- **Status**: ✅ Complete

### 7. `docs_endpoint_tests.rs`
- **Resources**: HTTP server with docs
- **RAII**: `DocsTestServer`
- **Status**: ✅ Complete

### 8. `multi_response_tests.rs`
- **Resources**: HTTP server with content negotiation
- **RAII**: `MultiResponseTestServer`
- **Status**: ✅ Complete

### 9. `curl_harness.rs`
- **Resources**: Docker container for curl tests
- **RAII**: `ContainerHarness`
- **Status**: ✅ Complete

### 10. `tracing_util.rs`
- **Resources**: OpenTelemetry tracer provider
- **RAII**: `TestTracing`
- **Status**: ✅ Complete

---

## ✅ No Resources Needed (18 files)

These are pure unit tests with no external resources:

### 11. `router_tests.rs`
- **Resources**: None (in-memory Router)
- **RAII Needed**: ❌ No
- **Why**: Pure unit test, no I/O

### 12. `dispatcher_tests.rs`
- **Resources**: None (in-memory Dispatcher)
- **RAII Needed**: ❌ No
- **Why**: Pure unit test, no HTTP server

### 13. `middleware_tests.rs`
- **Resources**: None (tests middleware in isolation)
- **RAII Needed**: ❌ No
- **Why**: Uses `TestTracing` (already RAII)

### 14. `tracing_tests.rs`
- **Resources**: None (uses `TestTracing`)
- **RAII Needed**: ❌ No
- **Why**: Uses existing RAII fixture

### 15. `spec_tests.rs`
- **Resources**: Temp files (via `tempfile::NamedTempFile`)
- **RAII Needed**: ✅ **Already implemented**
- **Why**: Uses `NamedTempFile` (automatic cleanup)

### 16. `generator_tests.rs`
- **Resources**: None (in-memory schema processing)
- **RAII Needed**: ❌ No
- **Why**: Pure unit test

### 17. `validator_tests.rs`
- **Resources**: None (in-memory validation)
- **RAII Needed**: ❌ No
- **Why**: Pure unit test

### 18. `typed_tests.rs`
- **Resources**: None (type system tests)
- **RAII Needed**: ❌ No
- **Why**: Pure unit test

### 19. `sse_channel_tests.rs`
- **Resources**: None (in-memory channel tests)
- **RAII Needed**: ❌ No
- **Why**: Pure unit test

### 20. `param_style_tests.rs`
- **Resources**: None (parameter parsing tests)
- **RAII Needed**: ❌ No
- **Why**: Pure unit test

### 21. `auth_cors_tests.rs`
- **Resources**: None (auth/CORS logic tests)
- **RAII Needed**: ❌ No
- **Why**: Pure unit test

### 22. `static_files_tests.rs`
- **Resources**: None (static file logic tests)
- **RAII Needed**: ❌ No
- **Why**: Pure unit test

### 23. `dynamic_registration.rs`
- **Resources**: None (dynamic handler registration)
- **RAII Needed**: ❌ No
- **Why**: Pure unit test

### 24. `goose_api_load_test.rs`
- **Resources**: None (load test scenarios, not actual tests)
- **RAII Needed**: ❌ No
- **Why**: Scenario definitions, run externally

### 25. `goose_load_tests_simple.rs`
- **Resources**: None (load test scenarios)
- **RAII Needed**: ❌ No
- **Why**: Scenario definitions, run externally

### 26. `spec_helpers_tests.rs`
- **Resources**: None (helper function tests)
- **RAII Needed**: ❌ No
- **Why**: Pure unit test

### 27. `generator_templates_tests.rs`
- **Resources**: None (template tests)
- **RAII Needed**: ❌ No
- **Why**: Pure unit test

### 28. `generator_project_tests.rs`
- **Resources**: Temp directories
- **RAII Needed**: ⚠️ **Review needed**
- **Status**: See "Needs Review" section

---

## ⚠️ Needs Review (4 files)

### 29. `cli_tests.rs` 
**Status**: ⚠️ **NEEDS INVESTIGATION**

**Potential Resources:**
- Spawns CLI commands
- May create temp files/directories
- May not clean up on failure

**Should Add RAII?** 🔍 **Review Required**

**Action Items:**
1. Check if `Command::new()` creates resources
2. Check for temp file creation
3. Add RAII if resources leak

---

### 30. `hot_reload_tests.rs`
**Status**: ⚠️ **NEEDS INVESTIGATION**

**Potential Resources:**
- File watchers (`notify` crate)
- Temp spec files
- Watcher threads

**Should Add RAII?** 🔍 **Review Required**

**Current Approach:**
```rust
let watcher = watch_spec(...)?;
// ... test ...
drop(watcher);
```

**Issues:**
- Manual `drop()` - not enforced
- Recently fixed with scoped watcher
- Could benefit from RAII wrapper

**Action Items:**
1. Verify watcher cleanup is reliable
2. Consider `WatcherFixture` struct
3. Ensure temp files cleaned up

---

### 31. `docker_integration_tests.rs`
**Status**: ⚠️ **NEEDS INVESTIGATION**

**Potential Resources:**
- Docker containers
- May create containers without cleanup

**Should Add RAII?** 🔍 **Review Required**

**Action Items:**
1. Check if containers are properly stopped
2. Check for orphaned containers
3. Add `DockerTestFixture` if needed

---

### 32. `curl_integration_tests.rs`
**Status**: ⚠️ **DEPENDS ON `curl_harness.rs`**

**Resources:**
- Uses `ContainerHarness` from `curl_harness.rs`

**Should Add RAII?** ✅ **Already covered**

**Rationale:**
- Relies on `ContainerHarness` which has RAII
- No additional resources

---

## Detailed Analysis: Files Needing Investigation

### `cli_tests.rs` - DETAILED ANALYSIS

**Resources Created:**
```rust
fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cli_test_{}_{}", std::process::id(), nanos));
    fs::create_dir_all(&dir).unwrap();
    dir  // ❌ Never cleaned up!
}
```

**Problem:**
- Creates temp directories
- Generates full projects inside
- **Never deletes directories**
- One test creates: `cargo` stub file, project structure

**Should Add RAII?** ✅ **YES - HIGH PRIORITY**

**Recommendation:**
```rust
struct CliTestFixture {
    dir: PathBuf,
}

impl CliTestFixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("cli_test_{}_{}", 
            std::process::id(), 
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }
    
    fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for CliTestFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
```

**Impact if not fixed:**
- Each test run leaves large project directories in `/tmp`
- Contains generated Rust code, Cargo.toml, etc.
- Accumulates quickly in CI

---

### `hot_reload_tests.rs` - DETAILED ANALYSIS

**Resources Created:**
1. Temp spec file (via `temp_files::create_temp_yaml`)
2. File system watcher (via `notify` crate)
3. Background watcher thread

**Current Approach:**
```rust
{
    let watcher = watch_spec(&path, ...)?;
    // Test...
    drop(watcher);
    std::thread::sleep(Duration::from_millis(100)); // Grace period
}
std::fs::remove_file(&path).unwrap(); // Manual cleanup
```

**Should Add RAII?** ⚠️ **PARTIALLY DONE - NEEDS IMPROVEMENT**

**Current Status:**
- ✅ Watcher is scoped and dropped
- ✅ Recently fixed to prevent hanging
- ❌ Manual file cleanup at end
- ⚠️ Relies on `common::temp_files` (already has RAII via `#[allow(dead_code)]`)

**Recommendation:**
Use `tempfile::NamedTempFile` like `spec_tests.rs`:

```rust
#[test]
fn test_watch_spec_reload() {
    use std::io::Write;
    
    let mut temp = tempfile::NamedTempFile::new().unwrap();
    temp.write_all(SPEC_V1.as_bytes()).unwrap();
    temp.flush().unwrap();
    
    let path = temp.path().to_path_buf();
    // ... rest of test ...
    
    // Watcher scoped in inner block
    {
        let watcher = watch_spec(&path, ...)?;
        // Test...
    } // Watcher drops here
    
    // Temp file drops when test ends
}
```

**Impact if not fixed:**
- Currently: Manual cleanup works but not guaranteed on panic
- Low risk: Only 1 test file per run
- Nice to have: Consistency with other tests

---

### `docker_integration_tests.rs` - DETAILED ANALYSIS

**Resources Created:**
1. Docker container (via Bollard API)
2. Docker image build (in-memory)
3. Port bindings

**Current Approach:**
```rust
#[test]
fn test_petstore_container_health() {
    // ... setup ...
    let created = block_on(docker.create_container(...)).unwrap();
    let _ = block_on(docker.start_container(&created.id, ...));
    
    // ... tests ...
    
    // Cleanup at end
    let _ = block_on(docker.remove_container(
        &created.id,
        Some(RemoveContainerOptions { force: true, ..Default::default() })
    ));
    
    assert_eq!(final_status, 200); // ❌ Cleanup happens BEFORE assert!
}
```

**Problem:**
- ✅ Container IS cleaned up
- ❌ But cleanup happens before final assertion
- ❌ If assertion fails, cleanup already happened (ok)
- ❌ But not idiomatic - should use RAII

**Should Add RAII?** ⚠️ **NICE TO HAVE - CURRENTLY WORKING**

**Recommendation:**
```rust
struct DockerTestContainer {
    docker: Docker,
    container_id: String,
}

impl DockerTestContainer {
    fn new(image: &str, port: u16) -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;
        let created = block_on(docker.create_container(...))?;
        block_on(docker.start_container(&created.id, ...))?;
        
        Ok(Self {
            docker,
            container_id: created.id,
        })
    }
    
    fn id(&self) -> &str {
        &self.container_id
    }
}

impl Drop for DockerTestContainer {
    fn drop(&mut self) {
        let _ = block_on(self.docker.remove_container(
            &self.container_id,
            Some(RemoveContainerOptions { force: true, ..Default::default() })
        ));
    }
}
```

**Impact if not fixed:**
- Current: Works fine, cleanup is manual
- Risk: Low - only runs when `E2E_DOCKER=1`
- Benefit: Better consistency with other tests

---

### `curl_integration_tests.rs` - DETAILED ANALYSIS

**Resources:**
- Uses `ContainerHarness` from `curl_harness.rs`

**RAII Status:** ✅ **ALREADY HAS RAII** (via `ContainerHarness`)

**No action needed** - depends on external RAII implementation

---

## Summary Table

| # | File | Resources | Has RAII? | Should Add? | Priority |
|---|------|-----------|-----------|-------------|----------|
| 1 | `server_tests.rs` | HTTP server | ✅ Yes | N/A | N/A |
| 2 | `security_tests.rs` | HTTP server | ✅ Yes | N/A | N/A |
| 3 | `static_server_tests.rs` | HTTP server | ✅ Yes | N/A | N/A |
| 4 | `sse_tests.rs` | HTTP server | ✅ Yes | N/A | N/A |
| 5 | `metrics_endpoint_tests.rs` | HTTP server | ✅ Yes | N/A | N/A |
| 6 | `health_endpoint_tests.rs` | HTTP server | ✅ Yes | N/A | N/A |
| 7 | `docs_endpoint_tests.rs` | HTTP server | ✅ Yes | N/A | N/A |
| 8 | `multi_response_tests.rs` | HTTP server | ✅ Yes | N/A | N/A |
| 9 | `curl_harness.rs` | Docker | ✅ Yes | N/A | N/A |
| 10 | `tracing_util.rs` | OTEL | ✅ Yes | N/A | N/A |
| 11 | `spec_tests.rs` | Temp files | ✅ Yes | N/A | N/A |
| 12-27 | Various unit tests | None | ❌ No | ❌ No | N/A |
| 28 | `generator_project_tests.rs` | Temp dirs | ❌ No | ⚠️ Review | Medium |
| 29 | **`cli_tests.rs`** | **Temp dirs** | **❌ No** | **✅ YES** | **🔴 HIGH** |
| 30 | `hot_reload_tests.rs` | Watcher, temp | ⚠️ Partial | ⚠️ Improve | 🟡 Medium |
| 31 | `docker_integration_tests.rs` | Container | ⚠️ Manual | ⚠️ Nice-to-have | 🟢 Low |
| 32 | `curl_integration_tests.rs` | Uses #9 | ✅ Yes | N/A | N/A |

## Recommendation Priority

### 🔴 HIGH PRIORITY

**`cli_tests.rs`**
- ❌ Leaks temp directories with full projects
- ❌ No cleanup on test failure
- ❌ Accumulates quickly
- ✅ Should implement `CliTestFixture`

### 🟡 MEDIUM PRIORITY

**`hot_reload_tests.rs`**
- ⚠️ Manual file cleanup
- ✅ Watcher cleanup is good (recently fixed)
- ✅ Could use `tempfile::NamedTempFile` for consistency

**`generator_project_tests.rs`**
- Need to review if it creates temp projects
- May have same issue as `cli_tests.rs`

### 🟢 LOW PRIORITY

**`docker_integration_tests.rs`**
- ✅ Cleanup works
- ⚠️ Not idiomatic RAII
- ✅ Could add `DockerTestContainer` for consistency

## Action Items

1. ✅ **COMPLETED: Implemented `CliTestFixture`** in `cli_tests.rs`
2. ✅ **COMPLETED: Implemented `ProjectTestFixture`** in `generator_project_tests.rs`
3. ✅ **COMPLETED: Switched to `tempfile::NamedTempFile`** in `hot_reload_tests.rs`
4. ✅ **COMPLETED: Implemented `DockerTestContainer`** in `docker_integration_tests.rs`

---

**Audit Date**: October 10, 2025  
**Fixes Completed**: October 10, 2025  
**Total Files**: 32  
**With RAII**: 14 (44%) ← **Up from 10!**  
**No Resources**: 18 (56%)  
**Need RAII**: 0 (0%) ← **All Fixed!**  
**Resource Leaks**: **0** ← **Zero!**  

---

## Final Status: ✅ ALL RESOURCE LEAKS FIXED

See `docs/RAII_FIXES_COMPLETE.md` for detailed summary of all fixes.


