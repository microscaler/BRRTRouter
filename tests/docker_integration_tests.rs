use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

/// Check if Docker is available
fn is_docker_available() -> bool {
    Command::new("docker")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Check if curl is available
fn is_curl_available() -> bool {
    Command::new("curl")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Check if release binary exists
fn is_release_binary_available() -> bool {
    std::path::Path::new("target/release/pet_store").exists()
}

#[test]
#[ignore] // Integration test - requires Docker, release build, and network access
fn test_petstore_container_health() {
    if !is_docker_available() {
        println!("Skipping test: Docker not available");
        return;
    }
    
    if !is_curl_available() {
        println!("Skipping test: curl not available");
        return;
    }
    
    if !is_release_binary_available() {
        println!("Skipping test: Release binary not available. Run 'cargo build --release --example pet_store' first");
        return;
    }
    assert!(Command::new("docker")
        .args(["compose", "up", "-d", "--build"])
        .status()
        .expect("docker compose up")
        .success());

    // Give the container a moment to start
    sleep(Duration::from_secs(2));
    
    // Check if container is running
    let container_status = Command::new("docker")
        .args(["compose", "ps", "--format", "json"])
        .output()
        .expect("docker compose ps");
    println!("Container status: {}", String::from_utf8_lossy(&container_status.stdout));

    for i in 0..30 {
        let status = Command::new("curl")
            .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "http://localhost:8080/health"])
            .output()
            .expect("curl");
        println!("Attempt {}: Health check returned: {:?}", i+1, String::from_utf8_lossy(&status.stdout));
        if status.stdout == b"200" {
            break;
        }
        sleep(Duration::from_secs(1));
    }

    let out = Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "http://localhost:8080/health"])
        .output()
        .expect("curl request");
    println!("Final health check result: {:?}", String::from_utf8_lossy(&out.stdout));
    
    // Clean up before assertion to ensure containers are stopped
    let _ = Command::new("docker").args(["compose", "down"]).status();
    
    assert_eq!(out.stdout, b"200");
}
