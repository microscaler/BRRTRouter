//! # Service discovery from openapi/{suite}/ layout
//!
//! Mirrors the Python `workspace/discovery/__init__.py` surface.
//! Derives package names, binary names, ports, and suite/service metadata
//! from the filesystem — no hardcoding.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};



/// A discovered suite with its services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteInfo {
    /// Suite name (e.g. "hauliage", "trader")
    pub name: String,
    /// Services in this suite
    pub services: Vec<FileInfo>,
    /// Whether this suite has a BFF
    pub has_bff: bool,
}

/// Metadata for a discovered service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    /// Service name (e.g. "identity", "bff")
    pub name: String,
    /// Path to the OpenAPI spec
    pub openapi_path: PathBuf,
    /// BFF suite config path (if applicable)
    pub bff_config_path: Option<PathBuf>,
    /// Generated crate directory
    pub gen_dir: PathBuf,
    /// Implementation crate directory
    pub impl_dir: PathBuf,
    /// BRRTRouter dependency config
    pub deps_config_path: PathBuf,
}

/// Discover all suites and services under `openapi/`.
pub fn discover_suites(project_root: &Path, openapi_dir: &Path) -> Vec<SuiteInfo> {
    let suites_dir = openapi_dir;
    if !suites_dir.exists() {
        return Vec::new();
    }

    let mut suites = Vec::new();

    for entry in walkdir::WalkDir::new(suites_dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_dir() {
            continue;
        }

        let suite_name = entry.file_name().to_string_lossy().to_string();
        let suite_dir = entry.path();

        let mut services = Vec::new();
        let mut has_bff = false;

        // Check for BFF suite config
        if suite_dir.join("bff-suite-config.yaml").exists()
            || suite_dir.join("bff-suite-config.yml").exists()
        {
            has_bff = true;
        }

        // Discover services in this suite
        for service_entry in walkdir::WalkDir::new(suite_dir)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !service_entry.file_type().is_dir() {
                continue;
            }

            let service_name = service_entry.file_name().to_string_lossy().to_string();

            // Skip config files
            if matches!(
                service_name.as_str(),
                "bff-suite-config.yaml"
                    | "bff-suite-config.yml"
                    | "openapi.yaml"
                    | "openapi.yml"
            ) {
                continue;
            }

            let openapi_path = service_entry.path().join("openapi.yaml");
            if openapi_path.exists() {
                let gen_dir = project_root
                    .join("microservices")
                    .join(&suite_name)
                    .join(&service_name)
                    .join("gen");
                let impl_dir = project_root
                    .join("microservices")
                    .join(&suite_name)
                    .join(&service_name)
                    .join("impl");

                let deps_config_path = openapi_path.parent().unwrap().join("brrtrouter-dependencies.toml");

                services.push(FileInfo {
                    name: service_name,
                    openapi_path,
                    bff_config_path: None,
                    gen_dir,
                    impl_dir,
                    deps_config_path,
                });
            }
        }

        suites.push(SuiteInfo {
            name: suite_name,
            services,
            has_bff,
        });
    }

    suites
}

/// Get all suite names from the openapi directory.
pub fn get_suite_names(project_root: &Path, openapi_dir: &Path) -> Vec<String> {
    discover_suites(project_root, openapi_dir)
        .into_iter()
        .map(|s| s.name)
        .collect()
}

/// Derive package names for all services: service_name -> cargo package name.
///
/// Convention: `hauliage_{service_name_snake}`.
pub fn get_package_names(project_root: &Path, suite: Option<&str>) -> HashMap<String, String> {
    let mut out = HashMap::new();

    for suite_info in discover_suites(project_root, &project_root.join("openapi")) {
        if let Some(target) = suite {
            if suite_info.name != target {
                continue;
            }
        }

        for service in &suite_info.services {
            let snake = service.name.replace('-', "_");
            out.insert(
                service.name.clone(),
                format!("hauliage_{}", snake),
            );
        }

        // Also include BFF if present
        if suite_info.has_bff {
            let snake = "bff".replace('-', "_");
            out.insert("bff".to_string(), format!("hauliage_{}", snake));
        }
    }

    out
}

/// Derive binary names for all services: service_name -> artifact binary name.
///
/// Convention: `service_name` with hyphens replaced by underscores.
pub fn get_binary_names(project_root: &Path, suite: Option<&str>) -> HashMap<String, String> {
    let mut out = HashMap::new();

    for suite_info in discover_suites(project_root, &project_root.join("openapi")) {
        if let Some(target) = suite {
            if suite_info.name != target {
                continue;
            }
        }

        for service in &suite_info.services {
            out.insert(
                service.name.clone(),
                service.name.replace('-', "_"),
            );
        }

        if suite_info.has_bff {
            out.insert("bff".to_string(), "bff".to_string());
        }
    }

    out
}

/// Get suite info for a single service.
pub fn get_service_info(
    project_root: &Path,
    suite: &str,
    service_name: &str,
) -> Option<FileInfo> {
    for suite_info in discover_suites(project_root, &project_root.join("openapi")) {
        if suite_info.name != suite {
            continue;
        }

        for service in &suite_info.services {
            if service.name == service_name {
                return Some(service.clone());
            }
        }
    }

    None
}

/// Check if a service is a BFF service.
pub fn is_bff_service(project_root: &Path, suite: &str, service_name: &str) -> bool {
    if let Some(suite_info) = discover_suites(project_root, &project_root.join("openapi"))
        .into_iter()
        .find(|s| s.name == suite)
    {
        if suite_info.has_bff && service_name == "bff" {
            return true;
        }
    }
    false
}

/// Return the spec path for a service (BFF or regular).
pub fn service_spec_path(project_root: &Path, suite: &str, service_name: &str) -> PathBuf {
    let openapi_dir = project_root.join("openapi");

    if is_bff_service(project_root, suite, service_name) {
        openapi_dir.join("openapi_bff.yaml")
    } else {
        openapi_dir.join(suite).join(service_name).join("openapi.yaml")
    }
}

/// Return the BFF suite config path if it exists.
pub fn bff_suite_config_path(project_root: &Path, suite: &str) -> Option<PathBuf> {
    let config = project_root.join("openapi").join(suite).join("bff-suite-config.yaml");
    if config.exists() {
        Some(config)
    } else {
        let config_yml = project_root.join("openapi").join(suite).join("bff-suite-config.yml");
        if config_yml.exists() {
            Some(config_yml)
        } else {
            None
        }
    }
}
