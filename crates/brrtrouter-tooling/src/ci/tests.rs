use crate::ci::fix_cargo_toml;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn create_gen_crate_dir() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let microservices = dir.path().join("microservices");
    let gen = microservices.join("hauliage").join("identity").join("gen");
    fs::create_dir_all(&gen).unwrap();
    (dir, gen)
}

/// A Cargo.toml with both brrtrouter path deps.
const CARGO_WITH_PATH_DEPS: &str = r#"[package]
name = "test_gen"
version = "0.1.0"

[dependencies]
brrtrouter = { path = "../../../BRRTRouter" }
brrtrouter_macros = { path = "../../../BRRTRouter/brrtrouter_macros" }
serde = "1.0"
"#;

const CARGO_NO_BRRTRouter_DEPS: &str = r#"[package]
name = "test_gen"
version = "0.1.0"

[dependencies]
serde = "1.0"
"#;

const CARGO_WORKSPACE_DEPS: &str = r#"[package]
name = "test_gen"
version = "0.1.0"

[dependencies]
brrtrouter = { workspace = true }
brrtrouter_macros = { workspace = true }
serde = "1.0"
"#;

#[test]
fn test_fix_cargo_toml_works_in_microservices_gen() {
    let (_dir, gen_path) = create_gen_crate_dir();
    let cargo_path = gen_path.join("Cargo.toml");
    fs::write(&cargo_path, CARGO_WITH_PATH_DEPS).unwrap();

    let changed = fix_cargo_toml(&cargo_path, None).unwrap();
    assert!(changed, "Expected content to be changed");

    let content = fs::read_to_string(&cargo_path).unwrap();
    assert!(content.contains("brrtrouter = { workspace = true }"));
    assert!(content.contains("brrtrouter_macros = { workspace = true }"));
    assert!(!content.contains("path = \"../../../BRRTRouter\""));
}

#[test]
fn test_fix_cargo_toml_no_change_when_no_deps() {
    let (_dir, gen_path) = create_gen_crate_dir();
    let cargo_path = gen_path.join("Cargo.toml");
    fs::write(&cargo_path, CARGO_NO_BRRTRouter_DEPS).unwrap();

    let changed = fix_cargo_toml(&cargo_path, None).unwrap();
    assert!(!changed);
}

#[test]
fn test_fix_cargo_toml_no_change_when_already_workspace() {
    let (_dir, gen_path) = create_gen_crate_dir();
    let cargo_path = gen_path.join("Cargo.toml");
    fs::write(&cargo_path, CARGO_WORKSPACE_DEPS).unwrap();

    let changed = fix_cargo_toml(&cargo_path, None).unwrap();
    assert!(!changed);
}

#[test]
fn test_fix_cargo_toml_returns_false_for_nonexistent_file() {
    let changed = fix_cargo_toml(
        &TempDir::new()
            .unwrap()
            .path()
            .join("nonexistent")
            .join("Cargo.toml"),
        None,
    )
    .unwrap();
    assert!(!changed);
}

#[test]
fn test_fix_cargo_toml_preserves_other_deps() {
    let (_dir, gen_path) = create_gen_crate_dir();
    let cargo_path = gen_path.join("Cargo.toml");
    let cargo = r#"[package]
name = "test_gen"
version = "0.1.0"

[dependencies]
brrtrouter = { path = "foo" }
serde = "1.0"
anyhow = "1.0"
tokio = { version = "1", features = ["full"] }
"#;
    fs::write(&cargo_path, cargo).unwrap();

    let changed = fix_cargo_toml(&cargo_path, None).unwrap();
    assert!(changed);

    let content = fs::read_to_string(&cargo_path).unwrap();
    assert!(content.contains("brrtrouter = { workspace = true }"));
    assert!(content.contains("serde = \"1.0\""));
    assert!(content.contains("anyhow = \"1.0\""));
    assert!(content.contains("tokio = { version = \"1\", features = [\"full\"] }"));
}

#[test]
fn test_fix_cargo_toml_partial_deps_only_brrtrouter() {
    let (_dir, gen_path) = create_gen_crate_dir();
    let cargo_path = gen_path.join("Cargo.toml");
    let cargo = r#"[package]
name = "test_gen"

[dependencies]
brrtrouter = { path = "foo" }
serde = "1.0"
"#;
    fs::write(&cargo_path, cargo).unwrap();

    let changed = fix_cargo_toml(&cargo_path, None).unwrap();
    assert!(changed);

    let content = fs::read_to_string(&cargo_path).unwrap();
    assert!(content.contains("brrtrouter = { workspace = true }"));
    assert!(content.contains("serde = \"1.0\""));
    assert!(!content.contains("brrtrouter_macros"));
}

#[test]
fn test_fix_cargo_toml_partial_deps_only_macros() {
    let (_dir, gen_path) = create_gen_crate_dir();
    let cargo_path = gen_path.join("Cargo.toml");
    let cargo = r#"[package]
name = "test_gen"

[dependencies]
brrtrouter_macros = { path = "foo" }
serde = "1.0"
"#;
    fs::write(&cargo_path, cargo).unwrap();

    let changed = fix_cargo_toml(&cargo_path, None).unwrap();
    assert!(changed);

    let content = fs::read_to_string(&cargo_path).unwrap();
    assert!(content.contains("brrtrouter_macros = { workspace = true }"));
    assert!(content.contains("serde = \"1.0\""));
}
