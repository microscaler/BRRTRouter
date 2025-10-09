use std::net::SocketAddr;
use std::process::Command;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;
#[path = "common/mod.rs"]
mod common;
use common::http::wait_for_http_200;

/// Register a cleanup handler to ensure Docker containers are removed on test exit
///
/// This ensures cleanup happens even if tests panic or are interrupted.
/// The handler is registered once and persists for the entire test process.
static CLEANUP_REGISTERED: OnceLock<()> = OnceLock::new();

static HARNESS: OnceLock<ContainerHarness> = OnceLock::new();

/// Get the base URL for the shared test container
///
/// Lazily starts the container on first access and returns the URL for all subsequent calls.
/// The container is automatically cleaned up when the test process exits.
pub fn base_url() -> &'static str {
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
    eprintln!("Checking for orphaned test containers ({})...", name);

    // First, try to stop the container (may not exist, that's OK)
    let stop_output = Command::new("docker")
        .args(["stop", "-t", "2", &name])
        .output();

    if let Ok(output) = &stop_output {
        if output.status.success() {
            eprintln!("Stopped orphaned container: {}", name);
        }
    }

    // Then force remove it (works on stopped or running containers)
    let rm_output = Command::new("docker").args(["rm", "-f", &name]).output();

    if let Ok(output) = &rm_output {
        if output.status.success() {
            eprintln!("Removed orphaned container: {}", name);

            // Poll to verify the container name is actually released
            // This prevents race conditions in parallel test execution
            for attempt in 1..=20 {
                let check = Command::new("docker")
                    .args(["ps", "-a", "--filter", &format!("name=^/{}$", name), "-q"])
                    .output();

                if let Ok(output) = check {
                    let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if container_id.is_empty() {
                        eprintln!("Verified container name '{}' is released", name);
                        break;
                    }
                }

                if attempt == 20 {
                    eprintln!(
                        "Warning: Container name '{}' may still be in use after cleanup",
                        name
                    );
                }

                thread::sleep(Duration::from_millis(50));
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("No such container") {
                eprintln!("Warning: Failed to remove container: {}", stderr);
            }
        }
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
        // Register cleanup handler on first container start
        CLEANUP_REGISTERED.get_or_init(|| {
            // The Drop implementation will handle cleanup at process exit,
            // but we also register an explicit cleanup for any orphaned containers
            // from previous failed runs
            cleanup_orphaned_containers();
        });

        // Ensure Docker is available
        let docker_ok = Command::new("docker")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !docker_ok {
            panic!("Docker is required for curl e2e tests");
        }

        // Check if image already exists (e.g., pre-built in CI)
        let image_exists = Command::new("docker")
            .args(["image", "inspect", "brrtrouter-petstore:e2e"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !image_exists {
            // Build image if it doesn't exist (local dev workflow)
            eprintln!("Building brrtrouter-petstore:e2e image...");
            let status = Command::new("docker")
                .args(["build", "--no-cache", "-t", "brrtrouter-petstore:e2e", "."])
                .status()
                .expect("failed to build e2e image");
            assert!(status.success(), "docker build failed");
        } else {
            eprintln!("Using existing brrtrouter-petstore:e2e image");
        }

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
