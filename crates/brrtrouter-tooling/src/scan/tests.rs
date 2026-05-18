use crate::scan::{
    extract_openapi_path, extract_package_name, get_package_names, get_suite_names,
    get_suite_services, scan_microservices, ServiceCatalog, ServiceEntry,
};
use std::fs;
use std::path::PathBuf;

// ── Fixture helpers ──────────────────────────────────────────────────

fn make_microservices_with_service(dir: &tempfile::TempDir, suite: &str, service: &str) -> PathBuf {
    let base = dir.path().join("microservices").join(suite).join(service);
    let gen = base.join("gen");
    fs::create_dir_all(&gen).unwrap();
    base
}

// ── scan_microservices tests ─────────────────────────────────────────

#[test]
fn test_scan_microservices_returns_empty_for_missing_dir() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = scan_microservices(&dir.path().join("nonexistent"));
    assert!(catalog.services.is_empty());
}

#[test]
fn test_scan_microservices_returns_empty_when_no_gen_cargo_toml() {
    let dir = tempfile::tempdir().unwrap();
    let microservices = dir
        .path()
        .join("microservices")
        .join("hauliage")
        .join("identity");
    fs::create_dir_all(&microservices).unwrap();
    // No gen/Cargo.toml created

    let catalog = scan_microservices(&dir.path().join("microservices"));
    assert!(catalog.services.is_empty());
}

#[test]
fn test_scan_microservices_finds_one_service() {
    let dir = tempfile::tempdir().unwrap();
    make_microservices_with_service(&dir, "hauliage", "identity");
    // Create a minimal gen/Cargo.toml
    let cargo = dir
        .path()
        .join("microservices")
        .join("hauliage")
        .join("identity")
        .join("gen")
        .join("Cargo.toml");
    fs::write(
        &cargo,
        r#"[package]
name = "hauliage_identity"
"#,
    )
    .unwrap();

    let catalog = scan_microservices(&dir.path().join("microservices"));
    assert_eq!(catalog.services.len(), 1);
    assert_eq!(catalog.services[0].name, "identity");
    assert_eq!(catalog.services[0].suite, "hauliage");
    assert_eq!(catalog.services[0].package_name, "hauliage_identity");
}

#[test]
fn test_scan_microservices_finds_multiple_services() {
    let dir = tempfile::tempdir().unwrap();
    make_microservices_with_service(&dir, "hauliage", "identity");
    make_microservices_with_service(&dir, "hauliage", "auth");

    for service in &["identity", "auth"] {
        let cargo = dir
            .path()
            .join("microservices")
            .join("hauliage")
            .join(service)
            .join("gen")
            .join("Cargo.toml");
        fs::write(
            &cargo,
            format!(
                r#"[package]
name = "hauliage_{}"
"#,
                service
            ),
        )
        .unwrap();
    }

    let catalog = scan_microservices(&dir.path().join("microservices"));
    assert_eq!(catalog.services.len(), 2);
    let names: Vec<&str> = catalog.services.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"identity"));
    assert!(names.contains(&"auth"));
}

#[test]
fn test_scan_microservices_finds_services_across_suites() {
    let dir = tempfile::tempdir().unwrap();
    make_microservices_with_service(&dir, "hauliage", "identity");
    make_microservices_with_service(&dir, "trader", "orders");

    for (suite, svc, pkg) in &[
        ("hauliage", "identity", "hauliage_identity"),
        ("trader", "orders", "trader_orders"),
    ] {
        let cargo = dir
            .path()
            .join("microservices")
            .join(suite)
            .join(svc)
            .join("gen")
            .join("Cargo.toml");
        fs::write(
            &cargo,
            format!(
                r#"[package]
name = "{}"
"#,
                pkg
            ),
        )
        .unwrap();
    }

    let catalog = scan_microservices(&dir.path().join("microservices"));
    assert_eq!(catalog.services.len(), 2);
    let suites: Vec<&str> = catalog.services.iter().map(|s| s.suite.as_str()).collect();
    assert!(suites.contains(&"hauliage"));
    assert!(suites.contains(&"trader"));
}

#[test]
fn test_scan_microservices_ignores_non_dirs() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("microservices")).unwrap();
    let suite_path = dir.path().join("microservices").join("not_a_dir");
    // Create a file instead of a directory
    fs::write(&suite_path, "not a dir").unwrap();

    let catalog = scan_microservices(&dir.path().join("microservices"));
    assert!(catalog.services.is_empty());
}

#[test]
fn test_scan_microservices_falls_back_to_service_name() {
    let dir = tempfile::tempdir().unwrap();
    make_microservices_with_service(&dir, "hauliage", "identity");
    // Create gen/Cargo.toml without a name field
    let cargo = dir
        .path()
        .join("microservices")
        .join("hauliage")
        .join("identity")
        .join("gen")
        .join("Cargo.toml");
    fs::write(
        &cargo,
        r#"[package]
version = "0.1.0"
"#,
    )
    .unwrap();

    let catalog = scan_microservices(&dir.path().join("microservices"));
    assert_eq!(catalog.services.len(), 1);
    assert_eq!(catalog.services[0].package_name, "identity");
}

// ── extract_package_name tests ───────────────────────────────────────

#[test]
fn test_extract_package_name_found() {
    let dir = tempfile::tempdir().unwrap();
    let cargo = dir.path().join("Cargo.toml");
    fs::write(
        &cargo,
        r#"[package]
name = "my_service"
version = "0.1.0"
"#,
    )
    .unwrap();
    assert_eq!(extract_package_name(&cargo).unwrap(), "my_service");
}

#[test]
fn test_extract_package_name_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let cargo = dir.path().join("Cargo.toml");
    fs::write(
        &cargo,
        r#"[package]
version = "0.1.0"
"#,
    )
    .unwrap();
    assert!(extract_package_name(&cargo).is_none());
}

#[test]
fn test_extract_package_name_missing_file() {
    let path = PathBuf::from("/nonexistent/Cargo.toml");
    assert!(extract_package_name(&path).is_none());
}

// ── extract_openapi_path tests ───────────────────────────────────────

#[test]
fn test_extract_openapi_path_prefers_service_level() {
    let dir = tempfile::tempdir().unwrap();
    let service_dir = dir.path().join("hauliage").join("identity");
    fs::create_dir_all(service_dir.join("openapi")).unwrap();
    fs::write(
        service_dir.join("openapi").join("openapi.yaml"),
        "openapi: '3.0.0'",
    )
    .unwrap();

    let path = extract_openapi_path(&dir.path().join("hauliage"), "identity");
    assert!(path.contains("identity/openapi/openapi.yaml"));
}

#[test]
fn test_extract_openapi_path_falls_to_suite_level() {
    let dir = tempfile::tempdir().unwrap();
    let suite = dir.path().join("hauliage");
    fs::create_dir_all(suite.join("openapi")).unwrap();
    fs::write(
        suite.join("openapi").join("openapi.yaml"),
        "openapi: '3.0.0'",
    )
    .unwrap();

    let path = extract_openapi_path(&suite, "identity");
    assert!(path.contains("hauliage/openapi/openapi.yaml"));
}

#[test]
fn test_extract_openapi_path_falls_to_default() {
    let dir = tempfile::tempdir().unwrap();
    let suite = dir.path().join("hauliage");
    fs::create_dir_all(&suite).unwrap();
    // No openapi dirs exist

    let path = extract_openapi_path(&suite, "identity");
    assert_eq!(
        path,
        format!("{}/openapi.yaml", suite.join("identity").display())
    );
}

// ── get_suite_names tests ────────────────────────────────────────────

#[test]
fn test_get_suite_names_returns_sorted_unique() {
    let catalog = ServiceCatalog {
        services: vec![
            ServiceEntry {
                name: "identity".to_string(),
                suite: "hauliage".to_string(),
                package_name: "hauliage_identity".to_string(),
                openapi_path: "/path/to/spec.yaml".to_string(),
            },
            ServiceEntry {
                name: "auth".to_string(),
                suite: "hauliage".to_string(),
                package_name: "hauliage_auth".to_string(),
                openapi_path: "/path/to/spec2.yaml".to_string(),
            },
            ServiceEntry {
                name: "orders".to_string(),
                suite: "trader".to_string(),
                package_name: "trader_orders".to_string(),
                openapi_path: "/path/to/spec3.yaml".to_string(),
            },
        ],
    };

    let suites = get_suite_names(&catalog);
    assert_eq!(suites, vec!["hauliage", "trader"]);
}

#[test]
fn test_get_suite_names_empty_catalog() {
    let catalog = ServiceCatalog { services: vec![] };
    let suites = get_suite_names(&catalog);
    assert!(suites.is_empty());
}

// ── get_suite_services tests ─────────────────────────────────────────

#[test]
fn test_get_suite_services_filters_correctly() {
    let catalog = ServiceCatalog {
        services: vec![
            ServiceEntry {
                name: "identity".to_string(),
                suite: "hauliage".to_string(),
                package_name: "hauliage_identity".to_string(),
                openapi_path: "/path1.yaml".to_string(),
            },
            ServiceEntry {
                name: "auth".to_string(),
                suite: "hauliage".to_string(),
                package_name: "hauliage_auth".to_string(),
                openapi_path: "/path2.yaml".to_string(),
            },
            ServiceEntry {
                name: "orders".to_string(),
                suite: "trader".to_string(),
                package_name: "trader_orders".to_string(),
                openapi_path: "/path3.yaml".to_string(),
            },
        ],
    };

    let hauliage_services = get_suite_services(&catalog, "hauliage");
    assert_eq!(hauliage_services.len(), 2);
    let trader_services = get_suite_services(&catalog, "trader");
    assert_eq!(trader_services.len(), 1);
    assert_eq!(trader_services[0].name, "orders");
}

#[test]
fn test_get_suite_services_empty_for_unknown_suite() {
    let catalog = ServiceCatalog {
        services: vec![ServiceEntry {
            name: "identity".to_string(),
            suite: "hauliage".to_string(),
            package_name: "hauliage_identity".to_string(),
            openapi_path: "/path.yaml".to_string(),
        }],
    };

    let services = get_suite_services(&catalog, "nonexistent");
    assert!(services.is_empty());
}

// ── get_package_names tests ──────────────────────────────────────────

#[test]
fn test_get_package_names_returns_map() {
    let catalog = ServiceCatalog {
        services: vec![
            ServiceEntry {
                name: "identity".to_string(),
                suite: "hauliage".to_string(),
                package_name: "hauliage_identity".to_string(),
                openapi_path: "/path.yaml".to_string(),
            },
            ServiceEntry {
                name: "auth".to_string(),
                suite: "hauliage".to_string(),
                package_name: "hauliage_auth".to_string(),
                openapi_path: "/path2.yaml".to_string(),
            },
        ],
    };

    let pkgs = get_package_names(&catalog);
    assert_eq!(pkgs.len(), 2);
    assert_eq!(pkgs.get("identity").unwrap(), "hauliage_identity");
    assert_eq!(pkgs.get("auth").unwrap(), "hauliage_auth");
}

#[test]
fn test_get_package_names_empty_catalog() {
    let catalog = ServiceCatalog { services: vec![] };
    let pkgs = get_package_names(&catalog);
    assert!(pkgs.is_empty());
}

// ── ServiceEntry serialisation tests ─────────────────────────────────

#[test]
fn test_service_entry_serialization() {
    let entry = ServiceEntry {
        name: "identity".to_string(),
        suite: "hauliage".to_string(),
        package_name: "hauliage_identity".to_string(),
        openapi_path: "/spec.yaml".to_string(),
    };

    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("identity"));
    assert!(json.contains("hauliage"));

    let deserialized: ServiceEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "identity");
    assert_eq!(deserialized.suite, "hauliage");
}

#[test]
fn test_service_catalog_serialization() {
    let catalog = ServiceCatalog {
        services: vec![ServiceEntry {
            name: "identity".to_string(),
            suite: "hauliage".to_string(),
            package_name: "hauliage_identity".to_string(),
            openapi_path: "/spec.yaml".to_string(),
        }],
    };

    let json = serde_json::to_string(&catalog).unwrap();
    let deserialized: ServiceCatalog = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.services.len(), 1);
}
