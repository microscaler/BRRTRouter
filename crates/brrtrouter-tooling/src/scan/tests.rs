use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use crate::scan::{scan_microservices, get_suite_names, get_suite_services, get_package_names};

/// Create a fake microservices fixture tree.
fn create_fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let microservices = dir.path().join("microservices");

    // hauliage/identity/gen/Cargo.toml
    let identity_gen = microservices.join("hauliage").join("identity").join("gen");
    fs::create_dir_all(&identity_gen).unwrap();
    fs::write(
        identity_gen.join("Cargo.toml"),
        r#"name = "hauliage_identity_gen"
version = "0.1.0"
"#,
    ).unwrap();

    // hauliage/auth/gen/Cargo.toml
    let auth_gen = microservices.join("hauliage").join("auth").join("gen");
    fs::create_dir_all(&auth_gen).unwrap();
    fs::write(
        auth_gen.join("Cargo.toml"),
        r#"name = "hauliage_auth_gen"
version = "0.1.0"
"#,
    ).unwrap();

    // trader/orders/gen/Cargo.toml
    let orders_gen = microservices.join("trader").join("orders").join("gen");
    fs::create_dir_all(&orders_gen).unwrap();
    fs::write(
        orders_gen.join("Cargo.toml"),
        r#"name = "trader_orders_gen"
version = "0.1.0"
"#,
    ).unwrap();

    dir
}

#[test]
fn test_scan_microservices_returns_three_services() {
    let dir = create_fixture();
    let catalog = scan_microservices(&dir.path().join("microservices"));
    assert_eq!(catalog.services.len(), 3);
}

#[test]
fn test_scan_microservices_empty_on_missing_dir() {
    let dir = TempDir::new().unwrap();
    let catalog = scan_microservices(&dir.path().join("nonexistent"));
    assert!(catalog.services.is_empty());
}

#[test]
fn test_scan_microservices_only_finds_gen_crates() {
    let dir = create_fixture();
    let microservices = dir.path().join("microservices");

    // Add a service WITHOUT gen crate - should be ignored
    let no_gen = microservices.join("hauliage").join("impl");
    fs::create_dir_all(&no_gen).unwrap();

    let catalog = scan_microservices(&microservices);
    assert_eq!(catalog.services.len(), 3); // only gen crates
}

#[test]
fn test_scan_microservices_package_names() {
    let dir = create_fixture();
    let catalog = scan_microservices(&dir.path().join("microservices"));

    for entry in &catalog.services {
        assert!(entry.package_name.ends_with("_gen"));
    }
}

#[test]
fn test_get_suite_names_returns_sorted_unique() {
    let dir = create_fixture();
    let catalog = scan_microservices(&dir.path().join("microservices"));

    let suites = get_suite_names(&catalog);
    assert_eq!(suites, vec!["hauliage", "trader"]);
}

#[test]
fn test_get_suite_services_filters_correctly() {
    let dir = create_fixture();
    let catalog = scan_microservices(&dir.path().join("microservices"));

    let hauliage = get_suite_services(&catalog, "hauliage");
    assert_eq!(hauliage.len(), 2);
    assert!(hauliage.iter().any(|s| s.name == "identity"));
    assert!(hauliage.iter().any(|s| s.name == "auth"));

    let trader = get_suite_services(&catalog, "trader");
    assert_eq!(trader.len(), 1);
    assert_eq!(trader[0].name, "orders");
}

#[test]
fn test_get_suite_services_nonexistent_returns_empty() {
    let dir = create_fixture();
    let catalog = scan_microservices(&dir.path().join("microservices"));

    let missing = get_suite_services(&catalog, "nonexistent");
    assert!(missing.is_empty());
}

#[test]
fn test_get_package_names_returns_all_packages() {
    let dir = create_fixture();
    let catalog = scan_microservices(&dir.path().join("microservices"));

    let names = get_package_names(&catalog);
    assert_eq!(names.len(), 3);
    assert_eq!(names.get("identity"), Some(&"hauliage_identity_gen".to_string()));
    assert_eq!(names.get("auth"), Some(&"hauliage_auth_gen".to_string()));
    assert_eq!(names.get("orders"), Some(&"trader_orders_gen".to_string()));
}
