use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("cli_test_{}_{}", std::process::id(), nanos));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_cli_generate_creates_project() {
    let dir = temp_dir();
    let spec_src = std::path::Path::new("examples/openapi.yaml");
    let spec_dest = dir.join("openapi.yaml");
    fs::copy(spec_src, &spec_dest).unwrap();
    let exe = env!("CARGO_BIN_EXE_brrtrouter-gen");
    let status = Command::new(exe)
        .current_dir(&dir)
        .arg("generate")
        .arg("--spec")
        .arg(spec_dest.to_str().unwrap())
        .status()
        .expect("run cli");
    assert!(status.success());
    let project = dir.join("examples").join("pet_store");
    assert!(project.join("Cargo.toml").exists());
    assert!(project.join("src").join("main.rs").exists());
}
