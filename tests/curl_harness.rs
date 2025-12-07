use std::net::SocketAddr;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};
#[path = "common/mod.rs"]
mod common;
use common::http::wait_for_http_200;

/// Environment variables set by cargo-llvm-cov that interfere with musl cross-compilation.
/// These must be cleared when spawning the cargo build for the musl target.
const COVERAGE_ENV_VARS: &[&str] = &[
    "CARGO_LLVM_COV",
    "CARGO_LLVM_COV_TARGET_DIR",
    "LLVM_PROFILE_FILE",
    "CARGO_INCREMENTAL",
    // RUSTFLAGS contains -C instrument-coverage which adds __llvm_profile_runtime
];

/// Flag to track if signal handler cleanup is already running
static SIGNAL_CLEANUP_RUNNING: AtomicBool = AtomicBool::new(false);

/// Register signal handlers and atexit cleanup to ensure containers are always removed
///
/// This is critical because `HARNESS` is a static `OnceLock`, so its `Drop` is never
/// called until process exit. We need explicit cleanup on:
/// 1. Normal exit (atexit)
/// 2. SIGINT (Ctrl+C)
/// 3. SIGTERM (kill command)
fn register_signal_handlers() {
    extern "C" fn cleanup_handler() {
        // Prevent recursive cleanup if multiple handlers fire
        if SIGNAL_CLEANUP_RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }

        eprintln!("\n🧹 Cleaning up Docker resources on exit...");

        // 1. Clean up the running container
        if let Some(harness) = HARNESS.get() {
            eprintln!("Stopping container: {}", harness.container_id);
            let _ = Command::new("docker")
                .args(["stop", "-t", "2", &harness.container_id])
                .status();
            let _ = Command::new("docker")
                .args(["rm", "-f", &harness.container_id])
                .status();
        }

        // Also cleanup by name (in case harness wasn't initialized)
        cleanup_orphaned_containers();

        // 2. Clean up dangling test images
        // Why cleanup images?
        // - Each test run creates a new image (even though content is identical)
        // - These accumulate quickly (6+ images per test run)
        // - They're all 8-9MB and clutter `docker images`
        // - Dangling images (<none>:<none>) serve no purpose
        //
        // Strategy:
        // 1. First try `docker image prune` (safe, won't remove in-use images)
        // 2. Then manually remove remaining <none> images (with safety checks)
        //
        // Safety:
        // - Never use --force on individual image removal
        // - Skip images that return "conflict" or "being used" errors
        // - This prevents removing images from running containers (like kind)
        eprintln!("Cleaning up dangling test images...");

        // Step 1: Try docker prune first (safest, won't touch in-use images)
        let prune_result = Command::new("docker")
            .args([
                "image",
                "prune",
                "-f", // Force (no prompt)
                "--filter",
                "dangling=true", // Only <none>:<none> images
                "--filter",
                "until=1h", // Only recent (from this test run)
            ])
            .output();

        match prune_result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if !stdout.trim().is_empty() && !stdout.contains("Total reclaimed space: 0B") {
                    eprintln!("✓ Pruned: {}", stdout.trim());
                }
            }
            Err(e) => {
                eprintln!("⚠ Could not prune images: {}", e);
            }
        }

        // Step 2: Clean up remaining <none> images that prune missed
        // Get list of <none>:<none> image IDs
        // Note: This uses shell commands which might not work in all environments
        // If it fails, we just skip it (prune in Step 1 already did the main cleanup)
        match Command::new("sh")
            .args(["-c", "docker images | grep '<none>' | awk '{print $3}'"])
            .output()
        {
            Ok(output) if output.status.success() => {
                let image_ids = String::from_utf8_lossy(&output.stdout);
                let ids: Vec<&str> = image_ids.lines().filter(|s| !s.is_empty()).collect();

                if !ids.is_empty() {
                    eprintln!(
                        "Found {} additional <none> image(s) to remove...",
                        ids.len()
                    );
                    let mut removed_count = 0;
                    let mut skipped_count = 0;

                    for image_id in ids {
                        // Try to remove without --force (won't remove in-use images)
                        match Command::new("docker")
                            .args(["image", "rm", image_id])
                            .output()
                        {
                            Ok(rm_output) => {
                                if rm_output.status.success() {
                                    removed_count += 1;
                                } else {
                                    let stderr = String::from_utf8_lossy(&rm_output.stderr);
                                    // Skip errors for in-use images (safe to ignore)
                                    if stderr.contains("conflict") || stderr.contains("being used")
                                    {
                                        skipped_count += 1;
                                    } else {
                                        eprintln!(
                                            "  ⚠ Could not remove {}: {}",
                                            image_id,
                                            stderr.trim()
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("  ⚠ Failed to remove {}: {}", image_id, e);
                            }
                        }
                    }

                    if removed_count > 0 {
                        eprintln!("✓ Removed {} <none> image(s)", removed_count);
                    }
                    if skipped_count > 0 {
                        eprintln!("✓ Skipped {} in-use image(s) (safe)", skipped_count);
                    }
                }
            }
            Ok(_) => {
                // Command ran but returned non-zero (e.g., grep found no matches)
                // This is fine, nothing to clean up
            }
            Err(e) => {
                // Shell command not available or other error
                // This is fine, Step 1 (prune) already did the main work
                eprintln!("  ℹ️  Manual image cleanup unavailable: {}", e);
                eprintln!("     (docker prune in Step 1 already cleaned up most images)");
            }
        }

        eprintln!("✓ Cleanup complete\n");
    }

    extern "C" fn signal_handler(_: libc::c_int) {
        cleanup_handler();

        // Re-raise the signal to allow normal termination
        unsafe {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
            libc::raise(libc::SIGINT);
        }
    }

    // Register signal handlers for SIGINT and SIGTERM
    unsafe {
        libc::signal(libc::SIGINT, signal_handler as libc::sighandler_t);
        libc::signal(libc::SIGTERM, signal_handler as libc::sighandler_t);
    }

    // ALSO register atexit handler for normal process termination
    // This handles the case where tests complete successfully
    extern "C" fn atexit_wrapper() {
        cleanup_handler();
    }
    unsafe {
        libc::atexit(atexit_wrapper);
    }
}

/// Register a cleanup handler to ensure Docker containers are removed on test exit
///
/// This ensures cleanup happens even if tests panic or are interrupted.
/// The handler is registered once and persists for the entire test process.
static CLEANUP_REGISTERED: OnceLock<()> = OnceLock::new();

/// Singleton to ensure image setup runs exactly once across all test threads
static IMAGE_SETUP: OnceLock<Result<(), String>> = OnceLock::new();

static HARNESS: OnceLock<ContainerHarness> = OnceLock::new();

/// Ensure Docker image exists before running tests
///
/// This function uses a singleton pattern to ensure the image check
/// runs exactly once, even with parallel test execution. Other threads will
/// block and wait for the first thread to complete the check.
///
/// # Panics
///
/// Panics if Docker is not available or the required image doesn't exist.
pub fn ensure_image_ready() {
    let result = IMAGE_SETUP.get_or_init(|| {
        // Only ONE thread will execute this block
        let start = Instant::now();
        let thread_id = thread::current().id();
        eprintln!("\n=== Docker Image Setup (Thread {:?}) ===", thread_id);

        // Ensure Docker is available
        eprintln!("[1/2] Checking Docker availability...");
        let docker_ok = Command::new("docker")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !docker_ok {
            return Err("Docker is required for curl e2e tests. Please install Docker and ensure it's running.".to_string());
        }
        eprintln!("      ✓ Docker is available");

        // STEP 2: Build the binary locally using cross-compilation
        // =========================================================
        // Why cross-compile?
        // - We're on macOS (likely ARM64), but Docker runs Linux x86_64 containers
        // - Building natively would produce aarch64-apple-darwin binary (wrong arch!)
        // - We need x86_64-unknown-linux-musl for Docker's Linux containers
        //
        // Why cargo-zigbuild?
        // - Handles cross-compilation without needing musl-gcc on macOS
        // - Already configured in .cargo/config.toml for this target
        // - Same tool used by Tilt workflow (consistency!)
        //
        // Why build here instead of in Dockerfile?
        // - Local builds use incremental compilation (10-30s vs 5-10min in Docker)
        // - Cargo cache is preserved between runs
        // - ALWAYS tests current code (impossible to forget to rebuild!)
        eprintln!("[2/5] Building pet_store binary for Linux x86_64...");
        // Determine host OS/arch to choose build strategy
        let uname_s = Command::new("uname").arg("-s").output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        let uname_m = Command::new("uname").arg("-m").output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        // On macOS, use cargo-zigbuild for cross-compilation to musl.
        // On Linux x86_64 runners, build normally for musl without zig.
        //
        // IMPORTANT: We must clear cargo-llvm-cov environment variables to prevent
        // LLVM coverage instrumentation (__llvm_profile_runtime) from being added
        // to the musl binary, which causes linker errors with zigbuild.
        let build_output = if uname_s.contains("Darwin") {
            eprintln!("      → Detected macOS host; using cargo zigbuild for cross-compilation");
            let mut cmd = Command::new("cargo");
            cmd.args([
                "zigbuild",
                "--release",
                "-p", "pet_store",
                "--target", "x86_64-unknown-linux-musl",
            ]);
            // Clear coverage env vars to prevent __llvm_profile_runtime linker errors
            for var in COVERAGE_ENV_VARS {
                cmd.env_remove(var);
            }
            // Clear RUSTFLAGS if it contains coverage instrumentation
            if let Ok(flags) = std::env::var("RUSTFLAGS") {
                if flags.contains("instrument-coverage") {
                    cmd.env_remove("RUSTFLAGS");
                }
            }
            cmd.output().expect("failed to run cargo zigbuild")
        } else if uname_s.contains("Linux") && uname_m.contains("x86_64") {
            eprintln!("      → Detected Linux x86_64 runner; using standard cargo build for musl");
            // Prefer musl-gcc if available to ensure compatibility with crates like ring
            let mut cmd = Command::new("cargo");
            cmd.args([
                "build",
                "--release",
                "-p", "pet_store",
                "--target", "x86_64-unknown-linux-musl",
            ])
            .env("CC_x86_64_unknown_linux_musl", "musl-gcc")
            .env("CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER", "musl-gcc");
            // Clear coverage env vars
            for var in COVERAGE_ENV_VARS {
                cmd.env_remove(var);
            }
            if let Ok(flags) = std::env::var("RUSTFLAGS") {
                if flags.contains("instrument-coverage") {
                    cmd.env_remove("RUSTFLAGS");
                }
            }
            cmd.output().expect("failed to run cargo build for musl target")
        } else {
            // Fallback: try zigbuild first; if that fails, try normal build
            eprintln!("      → Unknown host ({uname_s} {uname_m}); trying cargo zigbuild, then cargo build if needed");
            let mut zig_cmd = Command::new("cargo");
            zig_cmd.args([
                "zigbuild",
                "--release",
                "-p", "pet_store",
                "--target", "x86_64-unknown-linux-musl",
            ]);
            // Clear coverage env vars
            for var in COVERAGE_ENV_VARS {
                zig_cmd.env_remove(var);
            }
            if let Ok(flags) = std::env::var("RUSTFLAGS") {
                if flags.contains("instrument-coverage") {
                    zig_cmd.env_remove("RUSTFLAGS");
                }
            }
            let zig_attempt = zig_cmd.output();
            match zig_attempt {
                Ok(out) if out.status.success() => out,
                _ => {
                    let mut fallback_cmd = Command::new("cargo");
                    fallback_cmd.args([
                        "build",
                        "--release",
                        "-p", "pet_store",
                        "--target", "x86_64-unknown-linux-musl",
                    ]);
                    // Clear coverage env vars
                    for var in COVERAGE_ENV_VARS {
                        fallback_cmd.env_remove(var);
                    }
                    if let Ok(flags) = std::env::var("RUSTFLAGS") {
                        if flags.contains("instrument-coverage") {
                            fallback_cmd.env_remove("RUSTFLAGS");
                        }
                    }
                    fallback_cmd
                        .env("CC_x86_64_unknown_linux_musl", "musl-gcc")
                        .env("CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER", "musl-gcc")
                        .output()
                        .expect("failed to run cargo build for musl target")
                }
            }
        };

        if !build_output.status.success() {
            eprintln!("      ❌ Build failed!");
            eprintln!("{}", String::from_utf8_lossy(&build_output.stderr));
            return Err("Failed to build pet_store binary for musl target".to_string());
        }
        eprintln!("      ✓ Binary built for Linux x86_64");

        // STEP 3: Verify the cross-compiled binary exists
        // ================================================
        eprintln!("[3/5] Verifying binary...");
        let binary_path = "target/x86_64-unknown-linux-musl/release/pet_store";
        if !std::path::Path::new(binary_path).exists() {
            return Err(format!("Binary not found at {}", binary_path));
        }
        eprintln!("      ✓ Binary found at {}", binary_path);

        // STEP 4: Copy to staging area (CRITICAL - same as Tilt workflow!)
        // =================================================================
        // Why copy to build_artifacts/?
        //
        // Docker's build context has .dockerignore which blocks target/* for performance:
        //   target/*                    ← blocks ALL of target/
        //   !build_artifacts/pet_store  ← but allows this specific file
        //
        // Without this staging step:
        //   - Docker can't access target/x86_64-unknown-linux-musl/release/pet_store
        //   - Build fails with: "not found" error
        //   - Even though the file exists on host!
        //
        // The staging area pattern:
        //   1. Build locally: cargo zigbuild → target/x86_64-unknown-linux-musl/release/pet_store
        //   2. Stage: copy → build_artifacts/pet_store
        //   3. Docker: COPY build_artifacts/pet_store → /pet_store
        //
        // This is the SAME pattern used in Tilt (see Tiltfile lines ~70-90):
        //   - Tilt builds locally for fast iteration
        //   - Copies to build_artifacts/
        //   - Docker just copies the pre-built binary
        //   - Result: Instant Docker builds (<1s) + always testing current code!
        //
        // For future AI/contributors:
        // - Do NOT remove this staging step!
        // - Do NOT try to copy directly from target/ in Dockerfile.test
        // - Do NOT modify .dockerignore to allow target/* (kills Docker performance)
        // - This pattern is intentional and matches our Tilt workflow
        eprintln!("[4/5] Copying to staging area...");
        std::fs::create_dir_all("build_artifacts")
            .expect("failed to create build_artifacts directory");
        std::fs::copy(binary_path, "build_artifacts/pet_store")
            .expect("failed to copy binary to staging");
        eprintln!("      ✓ Binary staged at build_artifacts/pet_store");

        // STEP 5: Build the Docker image (instant - just copies the staged binary!)
        // ==========================================================================
        // This is super fast (<1s) because:
        // - dockerfiles/Dockerfile.test uses FROM scratch (no base image layers)
        // - Only copies pre-built files (no compilation in Docker)
        // - The binary is already compiled and staged
        //
        // Result: 15-30s for full cycle (compile + Docker) vs 5-10min if we compiled in Docker!
        eprintln!("[5/5] Building Docker image (copying binary)...");
        let docker_output = Command::new("docker")
            .args([
                "build",
                "-f", "dockerfiles/Dockerfile.test",
                "-t", "brrtrouter-petstore:e2e",
                "--rm",              // Remove intermediate containers after build
                "--force-rm",        // Always remove intermediate containers (even on failure)
                "."
            ])
            .output()
            .expect("failed to run docker build");

        if !docker_output.status.success() {
            eprintln!("      ❌ Docker build failed!");
            eprintln!("{}", String::from_utf8_lossy(&docker_output.stderr));
            return Err("Docker build failed".to_string());
        }
        eprintln!("      ✓ Image ready");
        eprintln!("");
        eprintln!("=== Setup Complete in {:.2}s ===", start.elapsed().as_secs_f64());
        eprintln!("    ✨ Testing CURRENT code (just compiled)");
        eprintln!("");
        Ok(())
    });

    // All threads (including the one that ran setup) check the result
    if let Err(e) = result {
        panic!("{}", e);
    }

    // If we get here, another thread might have done the setup - let them know
    let thread_id = thread::current().id();
    eprintln!(
        "[Thread {:?}] Image setup complete, proceeding with test...",
        thread_id
    );
}

/// Get the base URL for the shared test container
///
/// Lazily starts the container on first access and returns the URL for all subsequent calls.
/// The container is automatically cleaned up when the test process exits.
///
/// **Important:** Call `ensure_image_ready()` before running tests to avoid setup timeouts.
pub fn base_url() -> &'static str {
    // Register signal handlers once to ensure cleanup on SIGINT/SIGTERM
    CLEANUP_REGISTERED.get_or_init(|| {
        register_signal_handlers();
    });

    // Ensure image is ready before starting container
    ensure_image_ready();

    let h = HARNESS.get_or_init(ContainerHarness::start);
    h.base_url.as_str()
}

/// Get the container name for this test process
///
/// Uses process ID to create unique container names for parallel test execution.
/// This allows nextest to run multiple test processes simultaneously without conflicts.
fn container_name() -> String {
    format!("brrtrouter-e2e-{}", std::process::id())
}

/// Manually clean up any orphaned containers from previous test runs
///
/// This is called automatically during container startup, but can also be invoked
/// manually if needed. Safe to call even if no container exists.
pub fn cleanup_orphaned_containers() {
    let name = container_name();
    eprintln!("Cleaning up container: {}", name);

    // Force kill and remove in one command (most aggressive)
    let kill_output = Command::new("docker").args(["rm", "-f", &name]).output();

    match kill_output {
        Ok(output) => {
            if output.status.success() {
                eprintln!("✓ Removed container: {}", name);
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("No such container") {
                    eprintln!("✓ No orphaned container found");
                } else {
                    eprintln!("⚠ Failed to remove container: {}", stderr);
                }
            }
        }
        Err(e) => {
            eprintln!("⚠ Docker command failed: {}", e);
        }
    }

    // Poll to verify the container is actually gone
    // This is critical to prevent "name already in use" errors
    for attempt in 1..=30 {
        // Increased from 20 to 30 attempts
        let check = Command::new("docker")
            .args(["ps", "-a", "--filter", &format!("name=^/{}$", name), "-q"])
            .output();

        if let Ok(output) = check {
            let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if container_id.is_empty() {
                if attempt > 1 {
                    eprintln!(
                        "✓ Container name '{}' is released (took {} attempts)",
                        name, attempt
                    );
                }
                return;
            }
        }

        if attempt == 30 {
            eprintln!(
                "❌ ERROR: Container name '{}' still in use after 30 attempts!",
                name
            );
            eprintln!("   This will cause 'name already in use' errors");
            eprintln!("   Try: docker rm -f {}", name);
        }

        thread::sleep(Duration::from_millis(100)); // Increased from 50ms to 100ms
    }
}

struct ContainerHarness {
    container_id: String,
    pub base_url: String,
}

impl Drop for ContainerHarness {
    /// Clean up the Docker container when tests complete
    ///
    /// Stops and removes the container to prevent naming conflicts in subsequent test runs.
    /// This is critical for local development where tests may be run repeatedly.
    fn drop(&mut self) {
        eprintln!("Cleaning up Docker container: {}", self.container_id);

        // Stop the container (with timeout)
        let stop_result = Command::new("docker")
            .args(["stop", "-t", "2", &self.container_id])
            .status();

        if let Err(e) = stop_result {
            eprintln!(
                "Warning: Failed to stop container {}: {}",
                self.container_id, e
            );
        }

        // Remove the container (force flag handles already-stopped containers)
        let rm_result = Command::new("docker")
            .args(["rm", "-f", &self.container_id])
            .status();

        if let Err(e) = rm_result {
            eprintln!(
                "Warning: Failed to remove container {}: {}",
                self.container_id, e
            );
        } else {
            eprintln!("Successfully cleaned up container: {}", self.container_id);
        }
    }
}

impl ContainerHarness {
    /// Start the Docker container for end-to-end tests
    ///
    /// This function:
    /// 1. Verifies Docker is available
    /// 2. Builds the image if needed (or reuses existing)
    /// 3. Cleans up any orphaned containers from previous runs
    /// 4. Starts a new container with a random port
    /// 5. Waits for the service to be ready
    ///
    /// # Panics
    ///
    /// Panics if Docker is unavailable, build fails, or the container doesn't become ready.
    fn start() -> Self {
        // ALWAYS cleanup orphaned containers first (not just once)
        // This is critical because if tests were cancelled, Drop may not have run
        eprintln!("Cleaning up any orphaned containers from previous runs...");
        cleanup_orphaned_containers();

        // Image setup is now handled by ensure_image_ready() called from base_url()
        // This ensures the image is built once for all tests, not per-container

        // Run container detached with random host port for 8080
        // Use unique container name per process to allow parallel test execution
        let container_name = container_name();
        eprintln!("Starting container: {}", container_name);
        let output = Command::new("docker")
            .args([
                "run",
                "-d",
                "-p",
                "127.0.0.1::8080", // random host port, loopback only
                "--name",
                &container_name,
                "brrtrouter-petstore:e2e",
            ])
            .output()
            .expect("failed to run container");
        assert!(
            output.status.success(),
            "docker run failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Query mapped port with retry - Docker needs a moment to set up network settings
        // Use `docker port` which is simpler and more reliable than `docker inspect` template
        // Retry up to 15 times with exponential backoff (max ~10 seconds total)
        let mut host_port = String::new();
        let mut retries = 0;
        let max_retries = 15;
        loop {
            // First check if container is still running
            let status_out = Command::new("docker")
                .args(["inspect", "-f", "{{.State.Running}}", &container_id])
                .output()
                .expect("failed to check container status");
            
            if !status_out.status.success() {
                let stderr = String::from_utf8_lossy(&status_out.stderr);
                panic!(
                    "Container {} is not running or does not exist: {}",
                    container_id, stderr
                );
            }
            
            // Use `docker port` which is more reliable than inspect template
            let port_out = Command::new("docker")
                .args(["port", &container_id, "8080/tcp"])
                .output()
                .expect("failed to get container port");
            
            if port_out.status.success() {
                let output = String::from_utf8_lossy(&port_out.stdout);
                // docker port output format: "0.0.0.0:PORT" or "127.0.0.1:PORT"
                // Extract just the port number
                if let Some(colon_pos) = output.rfind(':') {
                    let port_str = output[colon_pos + 1..].trim().to_string();
                    if !port_str.is_empty() && port_str.parse::<u16>().is_ok() {
                        host_port = port_str;
                        break;
                    }
                }
            }
            
            retries += 1;
            if retries >= max_retries {
                let stderr = String::from_utf8_lossy(&port_out.stderr);
                let stdout = String::from_utf8_lossy(&port_out.stdout);
                panic!(
                    "docker port failed after {} retries: {}\nContainer ID: {}\nStdout: {}\nStderr: {}",
                    max_retries,
                    if port_out.status.code().is_some() {
                        format!("exit code {:?}", port_out.status.code())
                    } else {
                        "unknown error".to_string()
                    },
                    container_id,
                    stdout,
                    stderr
                );
            }
            
            // Exponential backoff: 100ms, 200ms, 400ms, 800ms, 1.6s, 3.2s, etc.
            let delay_ms = 100 * (1 << (retries - 1).min(6)); // Cap at 6.4s
            thread::sleep(Duration::from_millis(delay_ms));
        }
        let base_url = format!("http://127.0.0.1:{}", host_port);

        // Wait for readiness using shared helper
        let addr: SocketAddr = format!("127.0.0.1:{}", host_port).parse().unwrap();
        wait_for_http_200(
            &addr,
            "/health",
            Duration::from_secs(15),
            Some(&container_id),
        )
        .expect("container readiness check failed");

        Self {
            container_id,
            base_url,
        }
    }
}

// Note: The container is automatically cleaned up via the Drop implementation when
// the test process exits. This prevents naming conflicts in subsequent test runs.
