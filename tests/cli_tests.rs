use std::fs;
use std::os::unix::fs::PermissionsExt;
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
    // stub cargo binary
    let stub = dir.join("cargo");
    fs::write(&stub, "#!/bin/sh\nexit 0\n").unwrap();
    let mut perms = fs::metadata(&stub).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&stub, perms).unwrap();

    let exe = env!("CARGO_BIN_EXE_brrtrouter-gen");
    let old_path = std::env::var("PATH").unwrap();
    let status = Command::new(exe)
        .current_dir(&dir)
        .env("PATH", format!("{}:{}", dir.display(), old_path))
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
