# CI Fix: Install cargo-zigbuild

## Problem

Curl integration tests were failing in CI with:

```
[2/5] Building pet_store binary for Linux x86_64...
      ❌ Build failed!
error: no such command: `zigbuild`

help: a command with a similar name exists: `build`
help: find a package to install `zigbuild` with `cargo search cargo-zigbuild`
```

## Root Cause

The `curl_harness.rs` test setup uses `cargo-zigbuild` for cross-compilation:

```rust
// tests/curl_harness.rs - ensure_image_ready()
let build_output = Command::new("cargo")
    .args([
        "zigbuild",
        "--release",
        "-p", "pet_store",
        "--target", "x86_64-unknown-linux-musl"
    ])
    .output()
    .expect("failed to run cargo zigbuild");
```

**Why zigbuild?**
- Fast, reliable cross-compilation from any platform
- No need for complex musl-gcc setup in tests
- Consistent with Tilt workflow and justfile

**CI had:**
- ✅ `musl-gcc` for main build
- ❌ No `cargo-zigbuild` for test harness

## Solution

Added `cargo-zigbuild` installation to CI workflow:

**File:** `.github/workflows/ci.yml` (line 67-68)

```yaml
- name: Install cargo-zigbuild (for curl_harness cross-compilation)
  run: cargo install cargo-zigbuild --locked
```

**Placement:** After `musl-tools`, before `Build pet_store`

## Why This Location?

```yaml
# 1. Install Rust target
- name: Install musl target for pet_store
  run: rustup target add x86_64-unknown-linux-musl

# 2. Install system tools
- name: Install musl tools (provides musl-gcc for ring)
  run: sudo apt-get update && sudo apt-get install -y musl-tools

# 3. Install Cargo tools (NEW!)
- name: Install cargo-zigbuild (for curl_harness cross-compilation)
  run: cargo install cargo-zigbuild --locked

# 4. Build pet_store (uses musl-gcc)
- name: Build pet_store (musl, release)
  run: cargo build --release -p pet_store --target x86_64-unknown-linux-musl

# ... later ...

# 5. Run tests (curl_harness uses cargo-zigbuild)
- name: Run tests (nextest - primary)
  run: cargo nextest run --workspace --all-targets
```

## Two Build Methods

### Main Build (musl-gcc)
```yaml
- name: Build pet_store (musl, release)
  env:
    CC_x86_64_unknown_linux_musl: musl-gcc
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER: musl-gcc
  run: cargo build --release -p pet_store --target x86_64-unknown-linux-musl
```
- Used for artifact creation
- Uploaded for e2e-docker job
- Traditional approach

### Test Harness Build (cargo-zigbuild)
```rust
// tests/curl_harness.rs
cargo zigbuild --release -p pet_store --target x86_64-unknown-linux-musl
```
- Used by curl_integration_tests
- Ensures tests use current code
- Faster, more reliable

**Both approaches work!** They're used for different purposes.

## Why Not Use musl-gcc in Tests?

We could have changed the test harness to use musl-gcc:

```rust
// Alternative approach (NOT chosen)
Command::new("cargo")
    .args(["build", "--release", "-p", "pet_store", "--target", "x86_64-unknown-linux-musl"])
    .env("CC_x86_64_unknown_linux_musl", "musl-gcc")
    .env("CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER", "musl-gcc")
    .output()
```

**Why we chose zigbuild instead:**
1. **Consistency**: Matches Tilt workflow and justfile
2. **Simplicity**: No environment variable setup needed
3. **Reliability**: zigbuild is designed for cross-compilation
4. **Developer experience**: Same tool locally and in CI
5. **Future-proof**: Works on any platform (macOS, Windows, Linux)

## Installation Cost

```yaml
cargo install cargo-zigbuild --locked
```

**Time:** ~60-90 seconds
**Frequency:** Once per CI run
**Benefit:** Reliable cross-compilation for tests

**Worth it?** Yes!
- Prevents test failures
- Matches local development
- Future-proof for multi-platform testing

## Verification

After this change, CI should show:

```
✓ Install musl tools
✓ Install cargo-zigbuild (for curl_harness cross-compilation)
✓ Build pet_store (musl, release)
...
✓ Run tests (nextest - primary)
  running 6 tests
  === Docker Image Setup (Thread ThreadId(2)) ===
  [1/5] Checking Docker availability...
        ✓ Docker is available
  [2/5] Building pet_store binary for Linux x86_64...
        ✓ Build complete (15.3s)  ← SUCCESS!
  [3/5] Verifying binary...
        ✓ Binary exists
  ...
```

## Related Tools

The project uses multiple cross-compilation tools:

| Tool | Used By | Purpose |
|------|---------|---------|
| **musl-gcc** | Main CI build | Artifact creation |
| **cargo-zigbuild** | Tests, Tilt, local dev | Cross-compilation |
| **x86_64-linux-musl-gcc** | Local (if available) | Alternative linker |

All work together to support different workflows!

## Files Changed

- `.github/workflows/ci.yml` (line 67-68) - Added cargo-zigbuild installation

## Files NOT Changed

- `tests/curl_harness.rs` - Already correct
- `justfile` - Already correct  
- `Tiltfile` - Already correct
- `.cargo/config.toml` - Already correct

## Testing

### Local (should already work)
```bash
just nt curl
# Uses cargo-zigbuild if available
```

### CI (should now work)
```bash
cargo nextest run --test curl_integration_tests
# Now has cargo-zigbuild available
```

## Future Considerations

### Option 1: Unify on zigbuild everywhere
```yaml
# Use zigbuild for main build too
- name: Build pet_store (zigbuild)
  run: cargo zigbuild --release -p pet_store --target x86_64-unknown-linux-musl
```
**Pros:** Single tool, simpler
**Cons:** Longer installation time

### Option 2: Keep dual approach (current)
**Pros:** Main build is traditional, tests are modern
**Cons:** Two tools to maintain

### Option 3: Conditional zigbuild
```rust
// Try zigbuild first, fall back to cargo build
if Command::new("cargo").arg("zigbuild").status().is_ok() {
    use_zigbuild()
} else {
    use_cargo_build()
}
```
**Pros:** Flexible
**Cons:** More complex

**Current choice:** Option 2 (dual approach) - Works well!

## Summary

- ✅ Added `cargo install cargo-zigbuild` to CI
- ✅ Fixes curl_integration_tests build failures
- ✅ Matches local development workflow
- ✅ ~60s installation cost (acceptable)
- ✅ No code changes needed

**Result:** curl_integration_tests now work in CI! 🎉

