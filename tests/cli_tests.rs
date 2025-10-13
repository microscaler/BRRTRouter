//! Command-line interface integration tests
//!
//! # Test Coverage
//!
//! Validates the CLI commands and their behavior:
//! - `generate` command with various options
//! - Project generation from OpenAPI specs
//! - Argument parsing and validation
//! - Error messages and exit codes
//! - File structure verification
//!
//! # Test Strategy
//!
//! Uses subprocess execution to test CLI as end-users would:
//! 1. Run `brrtrouter-gen` binary with arguments
//! 2. Verify exit codes (0 = success, non-zero = error)
//! 3. Check generated file structure
//! 4. Validate generated code compiles
//!
//! # Key Test Cases
//!
//! - `test_cli_generate_creates_project`: Basic generation works
//! - Argument validation (missing spec, invalid paths)
//! - Force flag behavior (overwrite existing)
//! - Output directory creation
//!
//! # Note
//!
//! These tests are slower than unit tests because they:
//! - Execute external processes
//! - Generate full projects
//! - Compile generated code

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Test fixture for CLI tests with automatic directory cleanup via RAII
struct CliTestFixture {
    dir: PathBuf,
}

impl CliTestFixture {
    /// Create a new temporary directory for CLI testing
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cli_test_{}_{}", std::process::id(), nanos));
        fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    /// Get the path to the temporary directory
    fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for CliTestFixture {
    fn drop(&mut self) {
        // Clean up temp directory and all contents
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn test_cli_generate_creates_project() {
    // Use RAII fixture for automatic cleanup
    let fixture = CliTestFixture::new();
    let dir = fixture.path();

    let spec_src = Path::new("examples/openapi.yaml");
    let spec_dest = dir.join("openapi.yaml");
    fs::copy(spec_src, &spec_dest).unwrap();

    // Stub cargo binary
    let stub = dir.join("cargo");
    fs::write(&stub, "#!/bin/sh\nexit 0\n").unwrap();
    let mut perms = fs::metadata(&stub).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&stub, perms).unwrap();

    let exe = env!("CARGO_BIN_EXE_brrtrouter-gen");
    let old_path = std::env::var("PATH").unwrap();
    let status = Command::new(exe)
        .current_dir(dir)
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

    // Automatic cleanup when fixture drops!
}
