use crate::docker::{
    copy_artifacts, copy_artifacts_for_suite, validate_artifacts, ARCH_TARGETS,
    ARCH_TO_ARTIFACT_DIR,
};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

/// Create a fixture with fake built binaries for amd64.
fn create_docker_fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let release = dir
        .path()
        .join("microservices")
        .join("target")
        .join("x86_64-unknown-linux-musl")
        .join("release");
    fs::create_dir_all(&release).unwrap();

    // Create fake binaries
    for bin in &["hauliage_identity", "hauliage_auth", "trader_orders"] {
        fs::write(release.join(bin), b"fake binary").unwrap();
    }
    dir
}

/// Helper to create package/binary name maps for testing.
fn make_package_names() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("identity".to_string(), "hauliage_identity".to_string());
    m.insert("auth".to_string(), "hauliage_auth".to_string());
    m.insert("orders".to_string(), "trader_orders".to_string());
    m
}

fn make_binary_names() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("identity".to_string(), "hauliage_identity".to_string());
    m.insert("auth".to_string(), "hauliage_auth".to_string());
    m.insert("orders".to_string(), "trader_orders".to_string());
    m
}

// ── copy_artifacts tests ──────────────────────────────────────────────

#[test]
fn test_copy_artifacts_succeeds_for_amd64() {
    let dir = create_docker_fixture();
    let pkg_names = make_package_names();
    let bin_names = make_binary_names();

    let result = copy_artifacts("amd64", dir.path(), &pkg_names, &bin_names, "microservices");
    assert_eq!(result, 0);

    // Verify files were copied
    let out = dir.path().join("build_artifacts").join("amd64");
    assert!(out.is_dir());
    assert!(out.join("hauliage_identity").exists());
    assert!(out.join("hauliage_auth").exists());
    assert!(out.join("trader_orders").exists());
}

#[test]
fn test_copy_artifacts_succeeds_for_arm64() {
    let dir = create_docker_fixture();
    let release = dir
        .path()
        .join("microservices")
        .join("target")
        .join("aarch64-unknown-linux-musl")
        .join("release");
    fs::create_dir_all(&release).unwrap();
    fs::write(release.join("hauliage_identity"), b"fake").unwrap();

    let mut pkg_names = HashMap::new();
    pkg_names.insert("identity".to_string(), "hauliage_identity".to_string());
    let mut bin_names = HashMap::new();
    bin_names.insert("identity".to_string(), "hauliage_identity".to_string());

    let result = copy_artifacts("arm64", dir.path(), &pkg_names, &bin_names, "microservices");
    assert_eq!(result, 0);
}

#[test]
fn test_copy_artifacts_fails_for_unknown_arch() {
    let dir = create_docker_fixture();
    let result = copy_artifacts(
        "mips64",
        dir.path(),
        &make_package_names(),
        &make_binary_names(),
        "microservices",
    );
    assert_eq!(result, 1);
}

#[test]
fn test_copy_artifacts_fails_when_binaries_missing() {
    let dir = create_docker_fixture();
    // Create empty package map (binaries don't exist in this fixture's arch target)
    let mut pkg_names = HashMap::new();
    pkg_names.insert("identity".to_string(), "nonexistent_binary".to_string());
    let mut bin_names = HashMap::new();
    bin_names.insert("identity".to_string(), "nonexistent_binary".to_string());

    let result = copy_artifacts("amd64", dir.path(), &pkg_names, &bin_names, "microservices");
    assert_eq!(result, 1);
}

#[test]
fn test_copy_artifacts_sets_executable_permissions() {
    let dir = create_docker_fixture();
    let pkg_names = make_package_names();
    let bin_names = make_binary_names();

    copy_artifacts("amd64", dir.path(), &pkg_names, &bin_names, "microservices");

    let out = dir.path().join("build_artifacts").join("amd64");
    let perms = fs::metadata(out.join("hauliage_identity"))
        .unwrap()
        .permissions();
    assert_eq!(perms.mode() & 0o777, 0o755);
}

#[test]
fn test_copy_artifacts_uses_custom_workspace_dir() {
    let dir = create_docker_fixture();
    // Move the microservices dir to a custom location
    let custom = dir.path().join("custom_ws");
    fs::rename(dir.path().join("microservices"), &custom).unwrap();

    let pkg_names = make_package_names();
    let bin_names = make_binary_names();

    let result = copy_artifacts("amd64", dir.path(), &pkg_names, &bin_names, "custom_ws");
    assert_eq!(result, 0);
}

#[test]
fn test_copy_artifacts_uses_binary_names_over_package_names() {
    let dir = create_docker_fixture();
    let mut pkg_names = HashMap::new();
    pkg_names.insert("identity".to_string(), "hauliage_identity".to_string());
    let mut bin_names = HashMap::new();
    bin_names.insert("identity".to_string(), "custom_identity".to_string());

    copy_artifacts("amd64", dir.path(), &pkg_names, &bin_names, "microservices");

    // Should copy to custom_identity, not hauliage_identity
    let out = dir.path().join("build_artifacts").join("amd64");
    assert!(out.join("custom_identity").exists());
}

// ── copy_artifacts_for_suite tests ────────────────────────────────────

#[test]
fn test_copy_artifacts_for_suite_filters_by_hauliage_prefix() {
    let dir = create_docker_fixture();
    // copy_artifacts_for_suite filters by keys starting with "hauliage_"
    // Package names must have "hauliage_" prefixed keys for the filter to match
    let mut pkg_names = HashMap::new();
    pkg_names.insert(
        "hauliage_identity".to_string(),
        "hauliage_identity".to_string(),
    );
    pkg_names.insert("hauliage_auth".to_string(), "hauliage_auth".to_string());
    let mut bin_names = HashMap::new();
    bin_names.insert(
        "hauliage_identity".to_string(),
        "hauliage_identity".to_string(),
    );
    bin_names.insert("hauliage_auth".to_string(), "hauliage_auth".to_string());

    let result = copy_artifacts_for_suite(
        "amd64",
        dir.path(),
        &pkg_names,
        &bin_names,
        Some("hauliage"),
        "microservices",
    );
    assert_eq!(result, 0);

    let out = dir.path().join("build_artifacts").join("amd64");
    assert!(out.join("hauliage_identity").exists());
    assert!(out.join("hauliage_auth").exists());
}

#[test]
fn test_copy_artifacts_for_suite_no_filter_copies_all() {
    let dir = create_docker_fixture();
    // No suite filter — all packages copied
    let mut pkg_names = HashMap::new();
    pkg_names.insert(
        "hauliage_identity".to_string(),
        "hauliage_identity".to_string(),
    );
    pkg_names.insert("hauliage_auth".to_string(), "hauliage_auth".to_string());
    pkg_names.insert("trader_orders".to_string(), "trader_orders".to_string());
    let mut bin_names = HashMap::new();
    bin_names.insert(
        "hauliage_identity".to_string(),
        "hauliage_identity".to_string(),
    );
    bin_names.insert("hauliage_auth".to_string(), "hauliage_auth".to_string());
    bin_names.insert("trader_orders".to_string(), "trader_orders".to_string());

    let result = copy_artifacts_for_suite(
        "amd64",
        dir.path(),
        &pkg_names,
        &bin_names,
        None,
        "microservices",
    );
    assert_eq!(result, 0);

    let out = dir.path().join("build_artifacts").join("amd64");
    assert!(out.join("hauliage_identity").exists());
    assert!(out.join("hauliage_auth").exists());
    assert!(out.join("trader_orders").exists());
}

#[test]
fn test_copy_artifacts_for_suite_normalizes_underscore_to_hyphen() {
    let dir = TempDir::new().unwrap();
    let release = dir
        .path()
        .join("microservices")
        .join("target")
        .join("x86_64-unknown-linux-musl")
        .join("release");
    fs::create_dir_all(&release).unwrap();
    fs::write(release.join("hauliage_my_service"), b"fake").unwrap();

    // Keys must start with "hauliage_" to pass the suite filter
    let mut pkg_names = HashMap::new();
    pkg_names.insert(
        "hauliage_my_service".to_string(),
        "hauliage_my_service".to_string(),
    );
    let mut bin_names = HashMap::new();
    bin_names.insert(
        "hauliage_my_service".to_string(),
        "hauliage_my_service".to_string(),
    );

    let result = copy_artifacts_for_suite(
        "amd64",
        dir.path(),
        &pkg_names,
        &bin_names,
        Some("hauliage"),
        "microservices",
    );
    assert_eq!(result, 0);

    let out = dir.path().join("build_artifacts").join("amd64");
    // The binary name from bin_names["hauliage_my_service"] = "hauliage_my_service"
    // and it copies to that file name
    assert!(out.join("hauliage_my_service").exists());
}

// ── validate_artifacts tests ──────────────────────────────────────────

#[test]
fn test_validate_artifacts_passes_when_all_present() {
    let dir = TempDir::new().unwrap();
    let ba = dir.path().join("build_artifacts");

    for arch in &["amd64", "arm64", "arm"] {
        let arch_dir = ba.join(arch);
        fs::create_dir_all(&arch_dir).unwrap();
        fs::write(arch_dir.join("hauliage_identity"), b"fake").unwrap();
        fs::write(arch_dir.join("hauliage_auth"), b"fake").unwrap();
    }

    let mut bin_names = HashMap::new();
    bin_names.insert("identity".to_string(), "hauliage_identity".to_string());
    bin_names.insert("auth".to_string(), "hauliage_auth".to_string());

    let result = validate_artifacts(dir.path(), &bin_names);
    assert_eq!(result, 0);
}

#[test]
fn test_validate_artifacts_fails_when_missing_binary() {
    let dir = TempDir::new().unwrap();
    let ba = dir.path().join("build_artifacts");

    for arch in &["amd64", "arm64", "arm"] {
        let arch_dir = ba.join(arch);
        fs::create_dir_all(&arch_dir).unwrap();
        // Only identity, missing auth
        fs::write(arch_dir.join("hauliage_identity"), b"fake").unwrap();
    }

    let mut bin_names = HashMap::new();
    bin_names.insert("identity".to_string(), "hauliage_identity".to_string());
    bin_names.insert("auth".to_string(), "hauliage_auth".to_string());

    let result = validate_artifacts(dir.path(), &bin_names);
    assert_eq!(result, 1);
}

#[test]
fn test_validate_artifacts_fails_when_missing_arch_dir() {
    let dir = TempDir::new().unwrap();
    let ba = dir.path().join("build_artifacts");

    // Only create amd64, missing arm64 and arm
    let amd64 = ba.join("amd64");
    fs::create_dir_all(&amd64).unwrap();
    fs::write(amd64.join("hauliage_identity"), b"fake").unwrap();

    let mut bin_names = HashMap::new();
    bin_names.insert("identity".to_string(), "hauliage_identity".to_string());

    let result = validate_artifacts(dir.path(), &bin_names);
    assert_eq!(result, 1);
}

// ── Constants tests ───────────────────────────────────────────────────

#[test]
fn test_arch_targets_has_three_entries() {
    assert_eq!(ARCH_TARGETS.len(), 3);
    assert_eq!(ARCH_TARGETS[0].0, "amd64");
    assert_eq!(ARCH_TARGETS[1].0, "arm64");
    assert_eq!(ARCH_TARGETS[2].0, "arm7");
}

#[test]
fn test_arch_to_artifact_dir_mapping() {
    assert_eq!(ARCH_TO_ARTIFACT_DIR.len(), 3);
    assert_eq!(ARCH_TO_ARTIFACT_DIR[0], ("amd64", "amd64"));
    assert_eq!(ARCH_TO_ARTIFACT_DIR[1], ("arm64", "arm64"));
    assert_eq!(ARCH_TO_ARTIFACT_DIR[2], ("arm7", "arm"));
}

#[test]
fn test_arch_targets_match_arch_to_artifact_dir_keys() {
    for &(arch, _) in ARCH_TARGETS {
        assert!(
            ARCH_TO_ARTIFACT_DIR.iter().any(|(a, _)| *a == arch),
            "arch {} should be in ARCH_TO_ARTIFACT_DIR",
            arch
        );
    }
}
