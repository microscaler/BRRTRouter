use bollard::models::{ContainerCreateBody, HostConfig, PortBinding};
use bollard::query_parameters::{
    BuildImageOptionsBuilder, CreateContainerOptionsBuilder, RemoveContainerOptionsBuilder,
    StartContainerOptions,
};
use bollard::{body_full, Docker};
use bytes::Bytes;
use futures::executor::block_on;
use futures_util::stream::TryStreamExt;
use std::fs;
use std::net::SocketAddr;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use tar::Builder as TarBuilder;
use walkdir::WalkDir;
#[path = "common/mod.rs"]
mod common;
use common::http::wait_for_http_200;

/// RAII wrapper for Docker test containers to ensure cleanup
///
/// Automatically removes the container when dropped, even on panic.
/// This prevents the accumulation of orphaned containers from test failures.
struct DockerTestContainer {
    docker: Docker,
    container_id: String,
}

impl DockerTestContainer {
    /// Wrap an existing container ID for automatic cleanup
    fn from_id(docker: Docker, container_id: String) -> Self {
        Self {
            docker,
            container_id,
        }
    }

    /// Get the container ID
    fn id(&self) -> &str {
        &self.container_id
    }
}

impl Drop for DockerTestContainer {
    fn drop(&mut self) {
        // Always clean up container, even on panic
        // This is the fix for "dozens of uncleaned containers"!
        let opts = RemoveContainerOptionsBuilder::default().force(true).build();
        let _ = block_on(self.docker.remove_container(&self.container_id, Some(opts)));
    }
}

//

/// Check if Docker is available
fn is_docker_available() -> bool {
    Command::new("docker")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Check if release binary exists
fn is_release_binary_available() -> bool {
    std::path::Path::new("target/release/pet_store").exists()
}

/// Check if a Docker image exists locally to avoid network pulls in tests
// removed: not used

#[test]
fn test_petstore_container_health() {
    if std::env::var("E2E_DOCKER").is_err() {
        println!("Skipping: set E2E_DOCKER=1 to enable Docker e2e test");
        return;
    }
    let started_at = std::time::Instant::now();
    if !is_docker_available() {
        println!("Skipping test: Docker not available");
        return;
    }

    if !is_release_binary_available() {
        println!("Skipping test: Release binary not available. Run 'cargo build --release --example pet_store' first");
        return;
    }

    // Build Docker image via Bollard with no cache
    let docker = Docker::connect_with_local_defaults().expect("docker client");
    let mut archive = Vec::new();
    {
        let mut builder = TarBuilder::new(&mut archive);
        // Allowlist essential files for the multi-stage build
        let allow_prefixes = [
            "./Cargo.toml",
            "./Cargo.lock",
            "./dockerfiles/Dockerfile",
            "./src/",
            "./templates/",
            "./brrtrouter_macros/",
            "./examples/pet_store/",
        ];
        for entry in WalkDir::new(".") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            let p = path.to_string_lossy();
            // Exclude heavy/noise paths
            if p.starts_with("./target/") || p.starts_with("./.git/") || p.starts_with("./.github/")
            {
                continue;
            }
            if !allow_prefixes.iter().any(|pre| p.starts_with(pre)) {
                continue;
            }
            let rel = path.strip_prefix(".").unwrap();
            builder.append_path_with_name(path, rel).unwrap();
        }
        builder.finish().unwrap();
    }
    let build_opts = BuildImageOptionsBuilder::default()
        .dockerfile("dockerfiles/Dockerfile")
        .t("brrtrouter-petstore:e2e")
        .rm(true)
        .nocache(true)
        .build();
    let mut stream = docker.build_image(build_opts, None, Some(body_full(Bytes::from(archive))));
    while let Some(_chunk) = block_on(stream.try_next()).unwrap_or(None) {}

    // Create and start container with random host port for 8080/tcp
    let port_key = "8080/tcp".to_string();
    let bindings = std::collections::HashMap::from([(
        port_key.clone(),
        Some(vec![PortBinding {
            host_ip: Some("127.0.0.1".into()),
            host_port: Some("0".into()),
        }]),
    )]);
    let host_config = HostConfig {
        port_bindings: Some(bindings),
        ..Default::default()
    };
    let cfg = ContainerCreateBody {
        image: Some("brrtrouter-petstore:e2e".to_string()),
        host_config: Some(host_config),
        ..Default::default()
    };
    let create_opts = CreateContainerOptionsBuilder::default()
        .name("brrtrouter-e2e")
        .build();
    let created = block_on(docker.create_container(Some(create_opts), cfg)).unwrap();

    // Wrap container in RAII guard for automatic cleanup
    let container = DockerTestContainer::from_id(docker.clone(), created.id);

    block_on(docker.start_container(container.id(), None::<StartContainerOptions>)).unwrap();

    // Give the container a moment to start
    sleep(Duration::from_secs(2));

    // Poll health endpoint via raw TCP to avoid curl dependency
    let inspect = block_on(docker.inspect_container(
        container.id(),
        None::<bollard::query_parameters::InspectContainerOptions>,
    ))
    .unwrap();
    let mapped = inspect
        .network_settings
        .and_then(|ns| ns.ports)
        .and_then(|mut p| p.remove(&port_key).flatten())
        .and_then(|mut v| v.pop())
        .and_then(|b| b.host_port);
    let mapped_port = mapped.unwrap().parse::<u16>().unwrap();
    let addr: SocketAddr = format!("127.0.0.1:{mapped_port}").parse().unwrap();

    let mut final_status = 0;
    if wait_for_http_200(&addr, "/health", Duration::from_secs(30), None).is_ok() {
        final_status = 200;
    }

    // Container will be automatically stopped and removed when `container` drops!
    // No more orphaned containers! 🎉

    // --- JUnit XML report (for GitHub PRs) ---
    let duration = started_at.elapsed().as_secs_f64();
    let _ = fs::create_dir_all("target/e2e");
    let junit = if final_status == 200 {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"e2e\" tests=\"1\" failures=\"0\" time=\"{duration:.3}\">\n  <testcase classname=\"docker\" name=\"petstore_health\" time=\"{duration:.3}\"/>\n</testsuite>\n"
        )
    } else {
        let msg = format!("Expected 200 from /health, got {final_status}");
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"e2e\" tests=\"1\" failures=\"1\" time=\"{duration:.3}\">\n  <testcase classname=\"docker\" name=\"petstore_health\" time=\"{duration:.3}\">\n    <failure message=\"health check failed\"><![CDATA[{msg}]]></failure>\n  </testcase>\n</testsuite>\n"
        )
    };
    let _ = fs::write("target/e2e/junit-e2e.xml", junit);

    assert_eq!(final_status, 200);
}
