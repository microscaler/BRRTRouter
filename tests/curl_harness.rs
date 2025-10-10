use std::net::SocketAddr;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};
#[path = "common/mod.rs"]
mod common;
use common::http::wait_for_http_200;

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
        
        eprintln!("\n🧹 Cleaning up Docker containers on exit...");
        
        // Try to get the container from the harness and clean it up
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

        // Build the binary first (cross-compile for Linux x86_64)
        eprintln!("[2/4] Building pet_store binary for Linux x86_64...");
        let build_output = Command::new("cargo")
            .args([
                "zigbuild",
                "--release",
                "-p", "pet_store",
                "--target", "x86_64-unknown-linux-musl"
            ])
            .output()
            .expect("failed to run cargo zigbuild");
        
        if !build_output.status.success() {
            eprintln!("      ❌ Build failed!");
            eprintln!("{}", String::from_utf8_lossy(&build_output.stderr));
            return Err("Failed to build pet_store binary. Do you have cargo-zigbuild installed?".to_string());
        }
        eprintln!("      ✓ Binary built for Linux x86_64");
        
        // Check if binary exists
        eprintln!("[3/4] Verifying binary...");
        let binary_path = "target/x86_64-unknown-linux-musl/release/pet_store";
        if !std::path::Path::new(binary_path).exists() {
            return Err(format!("Binary not found at {}", binary_path));
        }
        eprintln!("      ✓ Binary found at {}", binary_path);

        // Build/rebuild the Docker image (instant - just copies files!)
        eprintln!("[4/4] Building Docker image (copying binary)...");
        let docker_output = Command::new("docker")
            .args([
                "build",
                "-f", "Dockerfile.test",
                "-t", "brrtrouter-petstore:e2e",
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
    eprintln!("[Thread {:?}] Image setup complete, proceeding with test...", thread_id);
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
    let kill_output = Command::new("docker")
        .args(["rm", "-f", &name])
        .output();

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
    for attempt in 1..=30 {  // Increased from 20 to 30 attempts
        let check = Command::new("docker")
            .args(["ps", "-a", "--filter", &format!("name=^/{}$", name), "-q"])
            .output();

        if let Ok(output) = check {
            let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if container_id.is_empty() {
                if attempt > 1 {
                    eprintln!("✓ Container name '{}' is released (took {} attempts)", name, attempt);
                }
                return;
            }
        }

        if attempt == 30 {
            eprintln!("❌ ERROR: Container name '{}' still in use after 30 attempts!", name);
            eprintln!("   This will cause 'name already in use' errors");
            eprintln!("   Try: docker rm -f {}", name);
        }

        thread::sleep(Duration::from_millis(100));  // Increased from 50ms to 100ms
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

        // Query mapped port
        let port_out = Command::new("docker")
            .args([
                "inspect",
                "-f",
                "{{(index (index .NetworkSettings.Ports \"8080/tcp\") 0).HostPort}}",
                &container_id,
            ])
            .output()
            .expect("failed to inspect container port");
        assert!(port_out.status.success(), "docker inspect failed");
        let host_port = String::from_utf8_lossy(&port_out.stdout).trim().to_string();
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
