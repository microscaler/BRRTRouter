# Cross-Compilation Fix for Curl Tests

## The Problem

When I initially implemented the auto-build solution, I forgot about **cross-compilation**!

```bash
$ just nt curl

[2/4] Building pet_store binary...
      ✓ Binary built
[4/4] Building Docker image...
      ✓ Image ready

# Tests run...
exec /pet_store: exec format error 💀
```

**The issue:** Building with `cargo build --release` on Apple Silicon produces an **ARM64 macOS binary**, but Docker on your Mac runs **Linux x86_64 containers**!

## The Solution

Use the **same cross-compilation setup as Tilt**:

```rust
// Build for Linux x86_64, not host architecture
cargo zigbuild --release -p pet_store --target x86_64-unknown-linux-musl
```

## Implementation

### File: tests/curl_harness.rs

**Before (Wrong):**
```rust
Command::new("cargo")
    .args(["build", "--release", "-p", "pet_store"])
    // Builds for host: aarch64-apple-darwin (ARM64 macOS)
```

**After (Correct):**
```rust
Command::new("cargo")
    .args([
        "zigbuild",
        "--release",
        "-p", "pet_store",
        "--target", "x86_64-unknown-linux-musl"  // Linux x86_64!
    ])
```

### File: Dockerfile.test

**Before (Wrong):**
```dockerfile
COPY target/release/pet_store /pet_store
# Tries to copy macOS ARM64 binary
```

**After (Correct):**
```dockerfile
COPY target/x86_64-unknown-linux-musl/release/pet_store /pet_store
# Copies Linux x86_64 binary
```

### File: justfile

**Before (Wrong):**
```bash
build-test-image:
    cargo build --release -p pet_store
```

**After (Correct):**
```bash
build-test-image:
    cargo zigbuild --release -p pet_store --target x86_64-unknown-linux-musl
```

## Why cargo-zigbuild?

From `.cargo/config.toml`:
```toml
[target.x86_64-unknown-linux-musl]
linker = "cargo-zigbuild"
rustflags = ["-C", "target-cpu=x86-64"]
```

**Benefits:**
- ✅ Works on macOS without installing musl-gcc
- ✅ Handles cross-compilation linker issues
- ✅ Same tool as Tilt workflow (consistency!)
- ✅ Already configured in the project

## Prerequisites

Make sure `cargo-zigbuild` is installed:

```bash
cargo install cargo-zigbuild
```

The test harness will give a helpful error if it's missing:
```
❌ Build failed!
Failed to build pet_store binary. Do you have cargo-zigbuild installed?
```

## Verification

Now the flow works correctly:

```bash
$ just nt curl

=== Docker Image Setup (Thread ThreadId(2)) ===
[1/4] Checking Docker availability...
      ✓ Docker is available
[2/4] Building pet_store binary for Linux x86_64...
      ✓ Binary built for Linux x86_64              # Cross-compiled!
[3/4] Verifying binary...
      ✓ Binary found at target/x86_64-unknown-linux-musl/release/pet_store
[4/4] Building Docker image (copying binary)...
      ✓ Image ready

=== Setup Complete in 28.5s ===
    ✨ Testing CURRENT code (just compiled)

# Tests run successfully!
```

## Architecture-Specific Build Paths

| Platform | Host Target | Docker Target | Binary Path |
|----------|-------------|---------------|-------------|
| **Apple Silicon (M1/M2/M3)** | aarch64-apple-darwin | x86_64-unknown-linux-musl | target/x86_64-unknown-linux-musl/release/ |
| **Intel Mac** | x86_64-apple-darwin | x86_64-unknown-linux-musl | target/x86_64-unknown-linux-musl/release/ |
| **Linux x86_64** | x86_64-unknown-linux-gnu | x86_64-unknown-linux-musl | target/x86_64-unknown-linux-musl/release/ |

**Key insight:** Even on Linux x86_64, we still cross-compile to **musl** for the static binary that works in `FROM scratch` containers!

## Why This Matters

### Before Fix (Exec Format Error)

```
Container starts → tries to run ARM64 macOS binary on Linux x86_64
→ exec format error
→ container crashes
→ tests hang waiting for health check
→ timeout after 60 seconds
```

### After Fix (Works)

```
Container starts → runs Linux x86_64 binary on Linux x86_64
→ starts successfully
→ health check passes
→ tests run
→ success!
```

## Consistency with Tilt

Both now use the same build process:

**Tilt:**
```bash
cargo zigbuild --release --target x86_64-unknown-linux-musl
→ copy to build_artifacts/
→ Docker copies from build_artifacts/
```

**Curl Tests:**
```bash
cargo zigbuild --release -p pet_store --target x86_64-unknown-linux-musl
→ Docker copies from target/x86_64-unknown-linux-musl/release/
```

Same cross-compilation, just different source paths!

## Files Modified

1. **tests/curl_harness.rs**
   - Changed `cargo build` to `cargo zigbuild`
   - Added `--target x86_64-unknown-linux-musl`
   - Updated binary path check
   - Added helpful error message

2. **Dockerfile.test**
   - Updated `COPY` path to cross-compiled binary
   - Added comment about cross-compilation

3. **justfile**
   - Updated `build-test-image` to use `cargo zigbuild`
   - Added target specification

4. **docs/CROSS_COMPILE_FIX.md** (this file)
   - Documents the cross-compilation requirement

## Related Documentation

- `docs/AUTO_BUILD_SOLUTION.md` - The auto-build pattern
- `docs/TILT_IMPLEMENTATION.md` - Tilt's cross-compilation setup
- `.cargo/config.toml` - Cross-compilation configuration

## Summary

The fix: **Remember to cross-compile for the Docker target architecture!**

- ❌ `cargo build` → Host architecture (macOS ARM64)
- ✅ `cargo zigbuild --target x86_64-unknown-linux-musl` → Linux x86_64

Now the auto-build solution works correctly on all platforms! 🎉

