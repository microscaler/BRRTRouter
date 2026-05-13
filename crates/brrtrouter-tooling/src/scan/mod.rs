//! # Microservices workspace scanner
//!
//! Scans `microservices/{suite}/` for gen crates and builds a service catalog
//! with their OpenAPI specs, ports, and dependencies.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// A discovered service in the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCatalog {
    pub services: Vec<ServiceEntry>,
}

/// A service entry in the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    pub name: String,
    pub suite: String,
    pub package_name: String,
    pub openapi_path: String,
}

/// Scan the microservices directory for all gen crates.
///
/// # Arguments
/// * `microservices_dir` - Path to the microservices/ directory.
///
/// # Returns
/// A ServiceCatalog with all discovered services, or an empty catalog
/// if no gen crates are found.
pub fn scan_microservices(microservices_dir: &Path) -> ServiceCatalog {
    let mut services = Vec::new();

    if !microservices_dir.exists() {
        return ServiceCatalog { services };
    }

    let suite_entries = match fs::read_dir(microservices_dir) {
        Ok(entries) => entries,
        Err(_) => return ServiceCatalog { services },
    };

    for suite_entry in suite_entries {
        let suite_entry = match suite_entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        let suite_name = match suite_entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => continue,
        };

        let suite_path = suite_entry.path();

        if !suite_path.is_dir() {
            continue;
        }

        let service_entries = match fs::read_dir(&suite_path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for service_entry in service_entries {
            let service_entry = match service_entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let service_name = match service_entry.file_name().into_string() {
                Ok(name) => name,
                Err(_) => continue,
            };

            let service_path = service_entry.path();

            if !service_path.is_dir() {
                continue;
            }

            let gen_path = service_path.join("gen");
            let cargo_toml = gen_path.join("Cargo.toml");

            if cargo_toml.exists() {
                let package_name = extract_package_name(&cargo_toml);
                let openapi_path = extract_openapi_path(&suite_path, &service_name);

                services.push(ServiceEntry {
                    name: service_name.clone(),
                    suite: suite_name.clone(),
                    package_name: package_name.unwrap_or_else(|| service_name.clone()),
                    openapi_path,
                });
            }
        }
    }

    ServiceCatalog { services }
}

/// Extract the package name from a Cargo.toml file.
fn extract_package_name(cargo_toml: &Path) -> Option<String> {
    let content = fs::read_to_string(cargo_toml).ok()?;

    for line in content.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix("name = \"") {
            if let Some(name) = name.strip_suffix('"') {
                return Some(name.to_string());
            }
        }
    }

    None
}

/// Extract the OpenAPI spec path for a service.
fn extract_openapi_path(suite_path: &Path, service_name: &str) -> String {
    let service_dir = suite_path.join(service_name);
    let inner = service_dir.join("openapi").join("openapi.yaml");
    let suite_inner = suite_path.join("openapi").join("openapi.yaml");
    let candidate_paths = vec![service_dir.join("openapi.yaml"), inner, suite_inner];

    for path in candidate_paths {
        if path.exists() {
            return path.to_string_lossy().to_string();
        }
    }

    format!("{}/openapi.yaml", service_dir.display())
}

/// Get all unique suite names from the catalog.
pub fn get_suite_names(catalog: &ServiceCatalog) -> Vec<String> {
    let mut suites: Vec<String> = catalog.services.iter().map(|s| s.suite.clone()).collect();
    suites.sort();
    suites.dedup();
    suites
}

/// Get all services in a suite.
pub fn get_suite_services<'a>(
    catalog: &'a ServiceCatalog,
    suite: &str,
) -> Vec<&'a ServiceEntry> {
    catalog
        .services
        .iter()
        .filter(|s| s.suite == suite)
        .collect()
}

/// Get all package names.
pub fn get_package_names(catalog: &ServiceCatalog) -> HashMap<String, String> {
    catalog
        .services
        .iter()
        .map(|s| (s.name.clone(), s.package_name.clone()))
        .collect()
}
