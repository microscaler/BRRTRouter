use crate::gen::{regenerate_suite_services, GenResult};
use std::path::PathBuf;

// ── GenResult structural tests ────────────────────────────────────────

#[test]
fn test_gen_result_fields() {
    let result = GenResult {
        success: true,
        service_name: "test_service".to_string(),
        output_dir: PathBuf::from("/test/output"),
    };
    assert!(result.success);
    assert_eq!(result.service_name, "test_service");
    assert_eq!(result.output_dir, PathBuf::from("/test/output"));
}

#[test]
fn test_gen_result_debug_fmt() {
    let result = GenResult {
        success: false,
        service_name: "debug_me".to_string(),
        output_dir: PathBuf::from("/debug/path"),
    };
    let debug_str = format!("{:?}", result);
    assert!(debug_str.contains("GenResult"));
    assert!(debug_str.contains("debug_me"));
}

// ── regenerate_suite_services failure cases ───────────────────────────

#[test]
fn test_regenerate_suite_services_handles_missing_suite() {
    // When the suite doesn't exist, the function should return 1
    // without panicking
    let temp_dir = tempfile::tempdir().unwrap();
    let failure_count = regenerate_suite_services(temp_dir.path(), "nonexistent_suite", &[], None);
    assert_eq!(failure_count, 1);
}

#[test]
fn test_regenerate_suite_services_empty_suite_list() {
    // With empty service_names, it tries to discover from suite_infos.
    // If suite doesn't exist, returns 1.
    let temp_dir = tempfile::tempdir().unwrap();
    let failure_count = regenerate_suite_services(
        temp_dir.path(),
        "fake_suite",
        &["identity".to_string()],
        None,
    );
    // Since the suite doesn't exist in the temp dir, it returns 1
    assert_eq!(failure_count, 1);
}

#[test]
fn test_regenerate_suite_services_empty_names_returns_one_on_missing_suite() {
    let temp_dir = tempfile::tempdir().unwrap();
    let failure_count = regenerate_suite_services(temp_dir.path(), "missing", &[], None);
    assert_eq!(failure_count, 1);
}

// ── find_cargo returns valid string ───────────────────────────────────

#[test]
fn test_find_cargo_returns_non_empty_string() {
    // find_cargo should always return something
    let cargo_path = super::find_cargo();
    assert!(!cargo_path.is_empty());
    // Either the path exists or it's just "cargo"
    assert!(
        std::path::Path::new(&cargo_path).exists() || cargo_path == "cargo",
        "find_cargo returned '{}', which doesn't exist and isn't 'cargo'",
        cargo_path
    );
}
