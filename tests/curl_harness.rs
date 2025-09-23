use std::net::SocketAddr;
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;
#[path = "common/mod.rs"]
mod common;
use common::http::wait_for_http_200;

static HARNESS: OnceLock<ContainerHarness> = OnceLock::new();

pub fn base_url() -> &'static str {
    let h = HARNESS.get_or_init(ContainerHarness::start);
    h.base_url.as_str()
}

struct ContainerHarness {
    container_id: String,
    pub base_url: String,
}

impl ContainerHarness {
    fn start() -> Self {
        // Ensure Docker is available
        let docker_ok = Command::new("docker")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !docker_ok {
            panic!("Docker is required for curl e2e tests");
        }

        // Always (re)build image to keep binary/spec in sync for CI
        let status = Command::new("docker")
            .args(["build", "--no-cache", "-t", "brrtrouter-petstore:e2e", "."])
            .status()
            .expect("failed to build e2e image");
        assert!(status.success(), "docker build failed");

        // Run container detached with random host port for 8080
        // Clean up any old container name from previous runs
        let _ = Command::new("docker")
            .args(["rm", "-f", "brrtrouter-e2e-shared"])
            .status();
        let output = Command::new("docker")
            .args([
                "run",
                "-d",
                "-p",
                "127.0.0.1::8080", // random host port, loopback only
                "--name",
                "brrtrouter-e2e-shared",
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
        wait_for_http_200(&addr, "/health", Duration::from_secs(15), Some(&container_id))
            .expect("container readiness check failed");

        Self {
            container_id,
            base_url,
        }
    }
}

// Note: We rely on CI runner cleanup for container removal. Local runs may leave
// the shared container running; users can `docker rm -f brrtrouter-e2e-shared`.
