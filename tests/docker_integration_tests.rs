use std::fs;
use std::io::{Read, Write};
use bollard::Docker;
use bollard::image::BuildImageOptions;
use bollard::container::{CreateContainerOptions, StartContainerOptions, RemoveContainerOptions, Config as ContainerConfig};
use bollard::models::{HostConfig, PortBinding};
use futures_util::stream::TryStreamExt;
use futures::executor::block_on;
use tar::Builder as TarBuilder;
use walkdir::WalkDir;
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

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
fn is_docker_image_available(image: &str) -> bool {
    Command::new("docker")
        .args(["image", "inspect", image])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
#[ignore] // Requires Docker; builds image from compiled binary and runs end-to-end checks
fn test_petstore_container_health() {
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
            "./Dockerfile",
            "./src/",
            "./templates/",
            "./brrtrouter_macros/",
            "./examples/pet_store/",
        ];
        for entry in WalkDir::new(".") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() { continue; }
            let p = path.to_string_lossy();
            // Exclude heavy/noise paths
            if p.starts_with("./target/") || p.starts_with("./.git/") || p.starts_with("./.github/") { continue; }
            if !allow_prefixes.iter().any(|pre| p.starts_with(pre)) { continue; }
            let rel = path.strip_prefix(".").unwrap();
            builder.append_path_with_name(path, rel).unwrap();
        }
        builder.finish().unwrap();
    }
    let build_opts = BuildImageOptions::<String> {
        dockerfile: "Dockerfile".to_string(),
        t: "brrtrouter-petstore:e2e".to_string(),
        rm: true,
        nocache: true,
        ..Default::default()
    };
    let mut stream = docker.build_image(build_opts, None, Some(archive.into()));
    while let Some(_chunk) = block_on(stream.try_next()).unwrap_or(None) {}

    // Create and start container with random host port for 8080/tcp
    let port_key = "8080/tcp".to_string();
    let bindings = std::collections::HashMap::from([(port_key.clone(), Some(vec![PortBinding { host_ip: Some("127.0.0.1".into()), host_port: Some("0".into()) }]))]);
    let host_config = HostConfig { port_bindings: Some(bindings), ..Default::default() };
    let cfg = ContainerConfig { image: Some("brrtrouter-petstore:e2e"), host_config: Some(host_config), ..Default::default() };
    let created = block_on(docker.create_container(Some(CreateContainerOptions { name: "brrtrouter-e2e", platform: None }), cfg)).unwrap();
    block_on(docker.start_container(&created.id, None::<StartContainerOptions<String>>)).unwrap();

    // Give the container a moment to start
    sleep(Duration::from_secs(2));

    // Poll health endpoint via raw TCP to avoid curl dependency
    let inspect = block_on(docker.inspect_container(&created.id, None)).unwrap();
    let mapped = inspect
        .network_settings
        .and_then(|ns| ns.ports)
        .and_then(|mut p| p.remove(&port_key).flatten())
        .and_then(|mut v| v.pop())
        .and_then(|b| b.host_port);
    let mapped_port = mapped.unwrap().parse::<u16>().unwrap();
    let addr: SocketAddr = format!("127.0.0.1:{}", mapped_port).parse().unwrap();

    let mut final_status = 0;
    for i in 0..30 {
        let mut s = match TcpStream::connect(addr) { Ok(s) => s, Err(_) => { sleep(Duration::from_millis(200)); continue; } };
        let req = b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let _ = s.write_all(req);
        let mut buf = Vec::new();
        let _ = s.set_read_timeout(Some(Duration::from_millis(200)));
        let _ = s.read_to_end(&mut buf);
        let resp = String::from_utf8_lossy(&buf);
        if let Some(line) = resp.lines().next() {
            if let Some(code) = line.split_whitespace().nth(1).and_then(|c| c.parse::<u16>().ok()) {
                final_status = code;
                if code == 200 { break; }
            }
        }
        println!("Attempt {}: response: {}", i + 1, resp.lines().next().unwrap_or(""));
        sleep(Duration::from_millis(500));
    }

    // Stop and remove container
    let _ = block_on(docker.remove_container(&created.id, Some(RemoveContainerOptions { force: true, ..Default::default() })));

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
