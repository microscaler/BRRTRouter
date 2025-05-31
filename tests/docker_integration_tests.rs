use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

#[test]
#[ignore]
fn test_petstore_container_health() {
    if Command::new("docker").arg("--version").output().is_err() {
        eprintln!("Docker not installed; skipping");
        return;
    }
    assert!(Command::new("docker")
        .args(["compose", "up", "-d", "--build"])
        .status()
        .expect("docker compose up")
        .success());

    for _ in 0..30 {
        let status = Command::new("curl")
            .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "http://localhost:8080/health"])
            .output()
            .expect("curl");
        if status.stdout == b"200" {
            break;
        }
        sleep(Duration::from_secs(1));
    }

    let out = Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "http://localhost:8080/health"])
        .output()
        .expect("curl request");
    assert_eq!(out.stdout, b"200");

    let _ = Command::new("docker").args(["compose", "down"]).status();
}
