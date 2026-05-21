use crate::paths::{brrtrouter_venv_root, discover_brrtrouter_root, is_brrtrouter_root, venv_bin};
use std::fs;
use tempfile::TempDir;

// ── venv_bin tests ────────────────────────────────────────────────────

#[test]
fn test_venv_bin_constructs_correct_path() {
    let bin = venv_bin(&["cargo"]);
    assert_eq!(bin.file_name().unwrap(), "cargo");

    let bin = venv_bin(&["bin", "rustc"]);
    assert_eq!(bin.file_name().unwrap(), "rustc");
}

// ── is_brrtrouter_root tests ──────────────────────────────────────────

#[test]
fn test_is_brrtrouter_root_true_when_cargo_toml_exists() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    assert!(is_brrtrouter_root(dir.path()));
}

#[test]
fn test_is_brrtrouter_root_false_when_no_cargo_toml() {
    let dir = TempDir::new().unwrap();
    assert!(!is_brrtrouter_root(dir.path()));
}

#[test]
fn test_is_brrtrouter_root_false_for_nonexistent() {
    let dir = TempDir::new().unwrap();
    let fake = dir.path().join("nonexistent");
    assert!(!is_brrtrouter_root(&fake));
}

// ── discover_brrtrouter_root tests ────────────────────────────────────

fn create_brrtrouter_fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    // Structure:
    // dir/
    //   project/
    //     microservices/   <-- project_root is here
    //   BRRTRouter/
    //     Cargo.toml
    let project = dir.path().join("project").join("microservices");
    fs::create_dir_all(&project).unwrap();
    let brrtrouter = dir.path().join("BRRTRouter");
    fs::create_dir_all(&brrtrouter).unwrap();
    fs::write(brrtrouter.join("Cargo.toml"), "[package]").unwrap();
    dir
}

fn create_env_override_fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let brrtrouter = dir.path().join("my_brrtrouter");
    fs::create_dir_all(&brrtrouter).unwrap();
    fs::write(brrtrouter.join("Cargo.toml"), "[package]").unwrap();

    std::env::set_var("BRRTROUTER_ROOT", brrtrouter.to_str().unwrap());
    dir
}

fn create_relative_env_override_fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    // Structure:
    // dir/
    //   project/
    //   project/override_dir/
    //     Cargo.toml
    let project = dir.path().join("project");
    let override_dir = project.join("override_dir");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&override_dir).unwrap();
    fs::write(override_dir.join("Cargo.toml"), "[package]").unwrap();

    // Use absolute path for env var since that's what the function expects
    std::env::set_var("BRRTROUTER_ROOT", override_dir.to_str().unwrap());
    dir
}

#[test]
fn test_discover_brrtrouter_root_finds_via_candidates() {
    let dir = create_brrtrouter_fixture();
    let project_root = dir.path().join("project").join("microservices");
    let root = discover_brrtrouter_root(&project_root);
    assert!(root.ends_with("BRRTRouter"));
}

#[test]
fn test_discover_brrtrouter_root_uses_env_override() {
    let dir = create_env_override_fixture();
    let project_root = dir.path().join("project");
    fs::create_dir_all(&project_root).unwrap();
    let root = discover_brrtrouter_root(&project_root);
    assert!(root.ends_with("my_brrtrouter"));

    std::env::remove_var("BRRTROUTER_ROOT");
}

#[test]
fn test_discover_brrtrouter_root_uses_relative_env_override() {
    let dir = create_relative_env_override_fixture();
    let project_root = dir.path().join("project");
    let root = discover_brrtrouter_root(&project_root);
    assert!(root.ends_with("override_dir"));

    std::env::remove_var("BRRTROUTER_ROOT");
}

#[test]
fn test_discover_brrtrouter_root_env_takes_precedence_over_candidates() {
    // Ensure no stale env var from previous tests
    std::env::remove_var("BRRTROUTER_ROOT");

    let dir = TempDir::new().unwrap();
    let override_dir = dir.path().join("custom_brrtrouter");
    let default_dir = dir.path().join("BRRTRouter");
    fs::create_dir_all(&default_dir).unwrap();
    fs::write(default_dir.join("Cargo.toml"), "[package]").unwrap();
    fs::create_dir_all(&override_dir).unwrap();
    fs::write(override_dir.join("Cargo.toml"), "[package]").unwrap();

    let project_root = dir.path().join("project");
    fs::create_dir_all(&project_root).unwrap();
    std::env::set_var("BRRTROUTER_ROOT", override_dir.to_str().unwrap());

    let root = discover_brrtrouter_root(&project_root);
    assert!(root.ends_with("custom_brrtrouter"));

    std::env::remove_var("BRRTROUTER_ROOT");
}

#[test]
fn test_discover_brrtrouter_root_returns_last_candidate_when_nothing_found() {
    let dir = TempDir::new().unwrap();
    let project_root = dir.path().join("some").join("path");
    fs::create_dir_all(&project_root).unwrap();

    let root = discover_brrtrouter_root(&project_root);
    assert!(root.ends_with("BRRTRouter"));
}

// ── brrtrouter_venv_root tests ────────────────────────────────────────

#[test]
fn test_brrtrouter_venv_root_default_path() {
    let root = brrtrouter_venv_root();
    assert!(root.ends_with("brrtrouter/venv"));
}

#[test]
fn test_brrtrouter_venv_root_uses_env_var_when_valid() {
    let dir = TempDir::new().unwrap();
    std::env::set_var("BRRTROUTER_VENV", dir.path());
    let root = brrtrouter_venv_root();
    assert_eq!(root, dir.path());
    std::env::remove_var("BRRTROUTER_VENV");
}

#[test]
fn test_brrtrouter_venv_root_ignores_nonexistent_env_var() {
    let dir = TempDir::new().unwrap();
    let fake = dir.path().join("nonexistent_venv");
    std::env::set_var("BRRTROUTER_VENV", fake.to_str().unwrap());
    let root = brrtrouter_venv_root();
    assert!(root.ends_with("brrtrouter/venv"));
    std::env::remove_var("BRRTROUTER_VENV");
}
