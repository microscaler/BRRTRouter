//! # Service code generation
//!
//! Mirrors `workspace/gen/regenerate.py` + `gen/brrtrouter.py` from the
//! Python tooling. Calls `brrtrouter-gen` via cargo run to generate the
//! gen crate from an OpenAPI spec.

use std::path::{Path, PathBuf};

use crate::ci;
use crate::discovery;
use crate::paths;
use crate::ToolingResult;

/// Result of a code generation operation.
#[derive(Debug)]
pub struct GenResult {
    pub success: bool,
    pub service_name: String,
    pub output_dir: PathBuf,
}

/// Generate the gen crate for a single service.
///
/// Calls `brrtrouter-gen generate --spec <path> --output <dir>` via cargo run
/// in the BRRTRouter checkout. Post-generates: fixes Cargo.toml path deps.
///
/// # Arguments
/// * `project_root` - Project root (e.g. `microscaler/hauliage`)
/// * `suite` - Suite name (e.g. "hauliage")
/// * `service_name` - Service name (e.g. "identity")
/// * `brrtrouter_path` - Optional override for BRRTRouter checkout path
pub fn regenerate_service(
    project_root: &Path,
    suite: &str,
    service_name: &str,
    brrtrouter_path: Option<&Path>,
) -> GenResult {
    let brrtrouter_path_buf = paths::discover_brrtrouter_root(project_root);
    let brrtrouter_root = brrtrouter_path.unwrap_or(&brrtrouter_path_buf);

    let spec_path = discovery::service_spec_path(project_root, suite, service_name);

    if !spec_path.exists() {
        eprintln!("❌ OpenAPI spec not found: {}", spec_path.display());
        return GenResult {
            success: false,
            service_name: service_name.to_string(),
            output_dir: PathBuf::new(),
        };
    }

    let gen_dir = project_root
        .join("microservices")
        .join(suite)
        .join(service_name)
        .join("gen");
    std::fs::create_dir_all(&gen_dir).ok();

    // Determine package name
    let package_name = discovery::get_package_names(project_root, Some(suite))
        .get(service_name)
        .map(|p| format!("{}_gen", p));

    // Determine deps config
    let deps_config_path = if discovery::is_bff_service(project_root, suite, service_name) {
        project_root.join("openapi").join("brrtrouter-dependencies.toml")
    } else {
        spec_path.parent().unwrap().join("brrtrouter-dependencies.toml")
    };

    let cargo_bin = find_cargo();
    let manifest = brrtrouter_root.join("Cargo.toml");
    if !manifest.exists() {
        eprintln!("❌ BRRTRouter not found at {}", brrtrouter_root.display());
        return GenResult {
            success: false,
            service_name: service_name.to_string(),
            output_dir: gen_dir,
        };
    }

    let mut cmd = std::process::Command::new(&cargo_bin);
    cmd.arg("run")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--bin")
        .arg("brrtrouter-gen")
        .arg("--")
        .arg("generate")
        .arg("--spec")
        .arg(&spec_path)
        .arg("--output")
        .arg(&gen_dir)
        .arg("--force");

    if deps_config_path.exists() {
        cmd.arg("--dependencies-config").arg(&deps_config_path);
    }
    if let Some(ref pkg_name) = package_name {
        cmd.arg("--package-name").arg(pkg_name);
    }

    println!("🔨 Generating {} for suite '{}'...", service_name, suite);

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("❌ Failed to spawn brrtrouter-gen: {}", e);
            return GenResult {
                success: false,
                service_name: service_name.to_string(),
                output_dir: gen_dir,
            };
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        eprintln!("❌ Failed to regenerate {}: {}", service_name, stderr);
        if !stdout.is_empty() {
            eprintln!("STDOUT: {}", stdout);
        }
        return GenResult {
            success: false,
            service_name: service_name.to_string(),
            output_dir: gen_dir,
        };
    }

    println!("✅ Regenerated {}", service_name);

    // Post-gen: fix Cargo.toml paths
    let gen_cargo = gen_dir.join("Cargo.toml");
    if gen_cargo.exists() {
        if let Err(e) = ci::fix_cargo_toml(&gen_cargo, Some(project_root)) {
            eprintln!("⚠ Warning: Failed to fix Cargo.toml paths: {}", e);
        }
    }

    GenResult {
        success: true,
        service_name: service_name.to_string(),
        output_dir: gen_dir,
    }
}

/// Generate gen crates for all services in a suite.
///
/// # Arguments
/// * `project_root` - Project root
/// * `suite` - Suite name
/// * `service_names` - Service names (empty = all in suite)
/// * `brrtrouter_path` - Optional BRRTRouter override
///
/// # Returns
/// Number of services that failed (0 = all succeeded).
pub fn regenerate_suite_services(
    project_root: &Path,
    suite: &str,
    service_names: &[String],
    brrtrouter_path: Option<&Path>,
) -> usize {
    let services: Vec<String> = if service_names.is_empty() {
        let suite_infos =
            discovery::discover_suites(project_root, &project_root.join("openapi"));
        if let Some(suite_info) = suite_infos.iter().find(|s| s.name == suite) {
            suite_info
                .services
                .iter()
                .map(|s| s.name.clone())
                .collect()
        } else {
            eprintln!("❌ Suite '{}' not found", suite);
            return 1;
        }
    } else {
        service_names.to_vec()
    };

    let mut failed = Vec::new();
    for name in &services {
        let result = regenerate_service(project_root, suite, name, brrtrouter_path);
        if !result.success {
            failed.push(name.clone());
        }
    }

    if !failed.is_empty() {
        println!(
            "\n❌ Failed to regenerate {} service(s): {}",
            failed.len(),
            failed.join(", ")
        );
    } else if !services.is_empty() {
        println!(
            "\n✅ Successfully regenerated {} service(s) in suite '{}'",
            services.len(),
            suite
        );
    }

    failed.len()
}

/// Find cargo binary, with fallback for non-standard paths.
fn find_cargo() -> String {
    if let Ok(path) = which::which("cargo") {
        return path.to_string_lossy().to_string();
    }
    if let Some(home) = dirs::home_dir() {
        let fallback = home.join(".cargo").join("bin").join("cargo");
        if fallback.exists() {
            return fallback.to_string_lossy().to_string();
        }
    }
    "cargo".to_string()
}

/// Generate stubs for the impl crate.
///
/// Calls `brrtrouter-gen generate-stubs`.
pub fn generate_stubs(
    project_root: &Path,
    suite: &str,
    service_name: &str,
    impl_dir: &Path,
    component_name: &str,
    force: bool,
    sync: bool,
    brrtrouter_path: Option<&Path>,
) -> ToolingResult<()> {
    let brrtrouter_path_buf = paths::discover_brrtrouter_root(project_root);
    let brrtrouter_root = brrtrouter_path.unwrap_or(&brrtrouter_path_buf);

    let spec_path = discovery::service_spec_path(project_root, suite, service_name);
    let manifest = brrtrouter_root.join("Cargo.toml");
    if !manifest.exists() {
        anyhow::bail!("BRRTRouter not found at {}", brrtrouter_root.display());
    }

    let cargo_bin = find_cargo();
    let mut cmd = std::process::Command::new(&cargo_bin);
    cmd.arg("run")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--bin")
        .arg("brrtrouter-gen")
        .arg("--")
        .arg("generate-stubs")
        .arg("--spec")
        .arg(&spec_path)
        .arg("--output")
        .arg(impl_dir)
        .arg("--component-name")
        .arg(component_name);

    if force {
        cmd.arg("--force");
    }
    if sync {
        cmd.arg("--sync");
    }

    let output = cmd.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("generate-stubs failed: {}", stderr);
    }

    Ok(())
}
