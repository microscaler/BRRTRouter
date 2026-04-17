# Curl Integration Tests - Docker Image Requirement

## Problem

The `curl_integration_tests` were hanging for 300+ seconds because:

1. The `dockerfiles/Dockerfile` compiles the entire Rust project from scratch (5-10 minutes)
2. Multiple test threads tried to build the image simultaneously  
3. The build process blocked test execution with no clear feedback

## Solution

### Singleton Pattern with Pre-built Image

The curl integration tests now use a **strict singleton pattern** that:

1. **Checks** if the Docker image exists (does NOT build it)
2. **Fails fast** with clear instructions if the image is missing
3. **Shows thread coordination** so you can see which thread does the check and which ones wait

### Implementation

```rust
/// Singleton to ensure image setup runs exactly once across all test threads
static IMAGE_SETUP: OnceLock<Result<(), String>> = OnceLock::new();

pub fn ensure_image_ready() {
    let result = IMAGE_SETUP.get_or_init(|| {
        // Only ONE thread executes this block
        let thread_id = thread::current().id();
        eprintln!("\n=== Docker Image Setup (Thread {:?}) ===", thread_id);
        
        // Check Docker availability
        // Check if image exists
        // Return Ok(()) or Err(message)
        
        Ok(())
    });
    
    // All threads check the result
    if let Err(e) = result {
        panic!("{}", e);
    }
    
    eprintln!("[Thread {:?}] Image setup complete, proceeding with test...", thread_id);
}
```

### How It Works

1. **First thread** calls `ensure_image_ready()`:
   - Enters the `get_or_init()` closure
   - Checks Docker and image existence
   - Stores `Ok(())` or `Err(message)` in the singleton
   - Prints completion message

2. **All other threads** call `ensure_image_ready()`:
   - Block waiting for first thread to complete
   - Get the cached result from the singleton
   - Panic if error, or proceed if OK
   - Print their own thread ID to show coordination

### Building the Image

Before running curl tests, you MUST build the Docker image:

```bash
# Build the image (takes 5-10 minutes on first build)
docker build -t brrtrouter-petstore:e2e .

# Verify the image exists
docker image ls | grep brrtrouter-petstore

# Now run the tests
cargo test --test curl_integration_tests
# or
just nt curl
```

### Why Not Auto-Build?

We explicitly chose **NOT** to auto-build because:

1. **Compilation takes 5-10 minutes** - unacceptable for test startup
2. **Docker layer caching** works better with explicit builds
3. **CI/CD pipelines** can build once and reuse the image
4. **Clear feedback** - developers know exactly what's needed
5. **No silent hangs** - fast failure with instructions

### CI Integration

In `.github/workflows/ci.yml`, the image is built once before running tests:

```yaml
- name: Build Docker image for integration tests
  run: docker build -t brrtrouter-petstore:e2e .

- name: Run integration tests
  run: cargo nextest run --test curl_integration_tests
```

### Local Development Workflow

```bash
# First time setup (or after Dockerfile changes)
docker build -t brrtrouter-petstore:e2e .

# Run tests as many times as you want
just nt curl

# Image is cached - rebuilds only what changed
# If you update code, rebuild the image:
docker build -t brrtrouter-petstore:e2e .
```

### Error Message

If you forget to build the image, you'll see:

```
=== Docker Image Setup (Thread ThreadId(2)) ===
[1/2] Checking Docker availability...
      ✓ Docker is available
[2/2] Checking for image brrtrouter-petstore:e2e...
      ❌ Image not found!

The curl integration tests require a pre-built Docker image.
Please build it first using:

  docker build -t brrtrouter-petstore:e2e .

This will take 5-10 minutes on first build (compiles Rust project).
Subsequent builds will be faster due to Docker layer caching.

thread 'main' panicked at 'Docker image brrtrouter-petstore:e2e not found. Build it first!'
```

### Thread Coordination Output

When tests run successfully, you'll see:

```
=== Docker Image Setup (Thread ThreadId(2)) ===
[1/2] Checking Docker availability...
      ✓ Docker is available
[2/2] Checking for image brrtrouter-petstore:e2e...
      ✓ Image is ready
=== Setup Complete in 0.05s ===

[Thread ThreadId(2)] Image setup complete, proceeding with test...
[Thread ThreadId(3)] Image setup complete, proceeding with test...
[Thread ThreadId(4)] Image setup complete, proceeding with test...
[Thread ThreadId(5)] Image setup complete, proceeding with test...
[Thread ThreadId(6)] Image setup complete, proceeding with test...
[Thread ThreadId(7)] Image setup complete, proceeding with test...
```

This shows that:
- Thread 2 did the actual check (0.05s)
- Threads 3-7 waited for Thread 2 to finish
- All threads proceeded once the singleton was initialized

## Files Modified

1. **tests/curl_harness.rs**
   - Changed `IMAGE_SETUP` from `OnceLock<()>` to `OnceLock<Result<(), String>>`
   - Added thread ID logging to show singleton coordination
   - Removed automatic image building - now fails fast with instructions
   - Added timing information

2. **docs/CURL_TESTS_DOCKER_IMAGE.md** (this file)
   - Documents the new requirement and workflow

## Related Documentation

- `docs/SIGINT_CLEANUP_FIX.md` - Signal handling for container cleanup
- `docs/DOCKER_CLEANUP_FIX.md` - RAII-based container cleanup
- `docs/DOCKER_IMAGE_SETUP.md` - Image setup strategy (now obsolete)

