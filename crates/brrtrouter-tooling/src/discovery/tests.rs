use crate::discovery::{
    bff_suite_config_path, discover_suites, get_binary_names, get_package_names, get_service_info,
    get_suite_names, is_bff_service, service_spec_path,
};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a fake openapi fixture tree.
///
/// ```
/// openapi/
///   hauliage/
///     identity/
///       openapi.yaml
///     auth/
///       openapi.yaml
///   trader/
///     orders/
///       openapi.yaml
///     bff-suite-config.yaml
///       bff/
///         openapi.yaml
/// ```
fn create_fixture_tree() -> TempDir {
    let dir = TempDir::new().unwrap();
    let openapi = dir.path().join("openapi");

    // hauliage/identity/
    let identity = openapi.join("hauliage").join("identity");
    fs::create_dir_all(&identity).unwrap();
    fs::write(identity.join("openapi.yaml"), "# OpenAPI spec").unwrap();
    fs::write(identity.join("brrtrouter-dependencies.toml"), "").unwrap();

    // hauliage/auth/
    let auth = openapi.join("hauliage").join("auth");
    fs::create_dir_all(&auth).unwrap();
    fs::write(auth.join("openapi.yaml"), "# OpenAPI spec").unwrap();
    fs::write(auth.join("brrtrouter-dependencies.toml"), "").unwrap();

    // trader/orders/
    let orders = openapi.join("trader").join("orders");
    fs::create_dir_all(&orders).unwrap();
    fs::write(orders.join("openapi.yaml"), "# OpenAPI spec").unwrap();
    fs::write(orders.join("brrtrouter-dependencies.toml"), "").unwrap();

    // trader/bff-suite-config.yaml
    fs::write(
        openapi.join("trader").join("bff-suite-config.yaml"),
        "bff:\n  port: 8080\n",
    )
    .unwrap();

    // trader/bff/
    let bff = openapi.join("trader").join("bff");
    fs::create_dir_all(&bff).unwrap();
    fs::write(bff.join("openapi.yaml"), "# OpenAPI spec for bff").unwrap();
    fs::write(bff.join("brrtrouter-dependencies.toml"), "").unwrap();

    dir
}

/// Test that discover_suites returns 2 suites when the fixture tree is set up.
#[test]
fn test_discover_suites_returns_two_suites() {
    let dir = create_fixture_tree();
    let project_root = dir.path();
    let openapi_dir = project_root.join("openapi");

    let suites = discover_suites(project_root, &openapi_dir);
    assert_eq!(suites.len(), 2);
}

/// Test that hauliage suite has 2 services (identity, auth).
#[test]
fn test_hauliage_has_two_services() {
    let dir = create_fixture_tree();
    let project_root = dir.path();
    let openapi_dir = project_root.join("openapi");

    let suites = discover_suites(project_root, &openapi_dir);
    let hauliage = suites
        .iter()
        .find(|s| s.name == "hauliage")
        .expect("hauliage suite missing");
    assert_eq!(hauliage.services.len(), 2);
    let names: Vec<&str> = hauliage.services.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"identity"));
    assert!(names.contains(&"auth"));
}

/// Test that trader suite has 2 services (bff, orders) and has_bff is true.
#[test]
fn test_trader_has_bff() {
    let dir = create_fixture_tree();
    let project_root = dir.path();
    let openapi_dir = project_root.join("openapi");

    let suites = discover_suites(project_root, &openapi_dir);
    let trader = suites
        .iter()
        .find(|s| s.name == "trader")
        .expect("trader suite missing");
    assert!(trader.has_bff);
    let names: Vec<&str> = trader.services.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"bff"));
    assert!(names.contains(&"orders"));
}

/// Test that FileInfo paths are correctly constructed.
#[test]
fn test_file_info_paths() {
    let dir = create_fixture_tree();
    let project_root = dir.path();
    let openapi_dir = project_root.join("openapi");

    let suites = discover_suites(project_root, &openapi_dir);
    let hauliage = suites
        .iter()
        .find(|s| s.name == "hauliage")
        .expect("hauliage missing");
    let identity = hauliage
        .services
        .iter()
        .find(|s| s.name == "identity")
        .expect("identity service missing");

    assert_eq!(identity.name, "identity");
    assert!(identity
        .openapi_path
        .ends_with("openapi/hauliage/identity/openapi.yaml"));
    assert!(identity
        .gen_dir
        .ends_with("microservices/hauliage/identity/gen"));
    assert!(identity
        .impl_dir
        .ends_with("microservices/hauliage/identity/impl"));
    assert!(identity
        .deps_config_path
        .ends_with("openapi/hauliage/identity/brrtrouter-dependencies.toml"));
}

/// Test that get_suite_names returns only suite names.
#[test]
fn test_get_suite_names() {
    let dir = create_fixture_tree();
    let project_root = dir.path();
    let openapi_dir = project_root.join("openapi");

    let names = get_suite_names(project_root, &openapi_dir);
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"hauliage".to_string()));
    assert!(names.contains(&"trader".to_string()));
}

/// Test get_package_names returns correct cargo package names.
#[test]
fn test_get_package_names_hauliage() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    let names = get_package_names(project_root, Some("hauliage"));
    assert_eq!(
        names.get("identity"),
        Some(&"hauliage_identity".to_string())
    );
    assert_eq!(names.get("auth"), Some(&"hauliage_auth".to_string()));
    assert_eq!(names.len(), 2); // no bff in hauliage
}

/// Test get_package_names includes bff when suite has_bff.
/// Note: bff service entry and has_bff both insert "bff", so has_bff
/// overwrites the same key — total count is 2, not 3.
#[test]
fn test_get_package_names_with_bff() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    let names = get_package_names(project_root, Some("trader"));
    assert_eq!(names.get("bff"), Some(&"hauliage_bff".to_string()));
    assert_eq!(names.get("orders"), Some(&"hauliage_orders".to_string()));
    assert_eq!(names.len(), 2); // bff + orders (has_bff overwrites bff key)
}

/// Test get_binary_names returns hyphen-safe binary names.
#[test]
fn test_get_binary_names() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    let names = get_binary_names(project_root, Some("trader"));
    assert_eq!(names.get("bff"), Some(&"bff".to_string()));
    assert_eq!(names.get("orders"), Some(&"orders".to_string()));
}

/// Test that non-existent openapi dir returns empty suites.
#[test]
fn test_discover_suites_empty_on_missing_dir() {
    let dir = TempDir::new().unwrap();
    let project_root = dir.path();
    let openapi_dir = project_root.join("nonexistent");

    let suites = discover_suites(project_root, &openapi_dir);
    assert!(suites.is_empty());
}

/// Test that discovery ignores top-level config files (not dirs).
#[test]
fn test_discovery_ignores_non_dir_entries() {
    let dir = create_fixture_tree();
    let openapi = dir.path().join("openapi");

    // Add a file at the top level that should be ignored
    fs::write(openapi.join("README.md"), "# Docs").unwrap();
    // Add a file inside hauliage/ that should be ignored
    fs::write(openapi.join("hauliage").join("notes.txt"), "").unwrap();

    let project_root = dir.path();
    let suites = discover_suites(project_root, &openapi);
    let hauliage = suites
        .iter()
        .find(|s| s.name == "hauliage")
        .expect("hauliage missing");
    // Should still only find identity and auth, not "notes.txt"
    assert_eq!(hauliage.services.len(), 2);
}

/// Test get_service_info returns Some for existing service.
#[test]
fn test_get_service_info_found() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    let info = get_service_info(project_root, "hauliage", "identity");
    assert!(info.is_some());
    let info = info.unwrap();
    assert_eq!(info.name, "identity");
}

/// Test get_service_info returns None for non-existent service.
#[test]
fn test_get_service_info_not_found() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    let info = get_service_info(project_root, "hauliage", "nonexistent");
    assert!(info.is_none());
}

/// Test get_service_info returns None for non-existent suite.
#[test]
fn test_get_service_info_wrong_suite() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    let info = get_service_info(project_root, "nonexistent", "identity");
    assert!(info.is_none());
}

/// Test is_bff_service returns true for BFF.
#[test]
fn test_is_bff_service_true() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    assert!(is_bff_service(project_root, "trader", "bff"));
}

/// Test is_bff_service returns false for non-BFF.
#[test]
fn test_is_bff_service_false() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    assert!(!is_bff_service(project_root, "hauliage", "identity"));
    assert!(!is_bff_service(project_root, "trader", "orders"));
}

/// Test is_bff_service returns false for non-existent service in bff suite.
#[test]
fn test_is_bff_service_nonexistent() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    // "nonexistent" is not the bff service
    assert!(!is_bff_service(project_root, "trader", "nonexistent"));
    assert!(!is_bff_service(project_root, "trader", "orders"));
    assert!(!is_bff_service(project_root, "hauliage", "bff"));
}

/// Test service_spec_path returns correct path for regular service.
#[test]
fn test_service_spec_path_regular() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    let path = service_spec_path(project_root, "hauliage", "identity");
    assert!(path.ends_with("openapi/hauliage/identity/openapi.yaml"));
}

/// Test service_spec_path returns correct path for BFF.
#[test]
fn test_service_spec_path_bff() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    let path = service_spec_path(project_root, "trader", "bff");
    assert!(path.ends_with("openapi/openapi_bff.yaml"));
}

/// Test service_spec_path returns correct path for non-BFF in non-BFF suite.
#[test]
fn test_service_spec_path_non_bff_suite() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    let path = service_spec_path(project_root, "hauliage", "identity");
    assert!(path.ends_with("openapi/hauliage/identity/openapi.yaml"));
}

/// Test bff_suite_config_path returns the correct path when config exists.
#[test]
fn test_bff_suite_config_path_exists() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    let config = bff_suite_config_path(project_root, "trader");
    assert!(config.is_some());
    let config = config.unwrap();
    assert!(config.ends_with("openapi/trader/bff-suite-config.yaml"));
}

/// Test bff_suite_config_path returns None for suite without config.
#[test]
fn test_bff_suite_config_path_missing() {
    let dir = create_fixture_tree();
    let project_root = dir.path();

    let config = bff_suite_config_path(project_root, "hauliage");
    assert!(config.is_none());
}
