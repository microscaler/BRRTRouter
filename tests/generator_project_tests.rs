use brrtrouter::generator::{format_project, generate_project_from_spec};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("gen_proj_test_{}_{}", std::process::id(), nanos));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_generate_project_and_format() {
    let dir = temp_dir();
    let spec_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("openapi.yaml");

    let prev_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();

    let project = generate_project_from_spec(&spec_path, true).expect("generate project");

    assert!(project.join("Cargo.toml").exists());
    assert!(project.join("src").join("main.rs").exists());
    assert!(project.join("src").join("registry.rs").exists());
    assert!(project.join("src").join("handlers").exists());
    assert!(project.join("src").join("controllers").exists());

    // Provide a stub cargo binary to satisfy format_project in environments
    // without rustfmt installed.
    let stub = dir.join("cargo");
    fs::write(&stub, "#!/bin/sh\nexit 0\n").unwrap();
    let mut perms = fs::metadata(&stub).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&stub, perms).unwrap();

    let old_path = std::env::var("PATH").unwrap();
    let new_path = format!("{}:{}", dir.display(), old_path);
    std::env::set_var("PATH", &new_path);
    let fmt_result = format_project(&project);
    std::env::set_var("PATH", old_path);
    assert!(fmt_result.is_ok());

    std::env::set_current_dir(&prev_dir).unwrap();
    fs::remove_dir_all(&dir).unwrap();
}
