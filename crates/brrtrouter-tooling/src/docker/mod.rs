//! # Docker artifact copying and validation
//!
//! Mirrors `workspace/docker/copy_artifacts.py` + `docker/copy_artifacts.py`.
//! Copies built Rust binaries from `microservices/target/{triple}/release/`
//! to `build_artifacts/{arch}/`.

use std::collections::HashMap;
use std::path::Path;
use std::fs;

/// Arch -> target triple mapping
pub const ARCH_TARGETS: &[(&str, &str)] = &[
    ("amd64", "x86_64-unknown-linux-musl"),
    ("arm64", "aarch64-unknown-linux-musl"),
    ("arm7", "armv7-unknown-linux-musleabihf"),
];

/// Arch -> artifact directory in build_artifacts
pub const ARCH_TO_ARTIFACT_DIR: &[(&str, &str)] = &[
    ("amd64", "amd64"),
    ("arm64", "arm64"),
    ("arm7", "arm"),
];

/// Copy built binaries from microservices/target/{triple}/release/
/// to build_artifacts/{arch}/.
///
/// # Arguments
/// * `arch` - Architecture: "amd64", "arm64", or "arm7"
/// * `project_root` - Project root directory
/// * `package_names` - service_name -> binary name in target/
/// * `binary_names` - service_name -> artifact name in build_artifacts/
/// * `workspace_dir` - Relative path to workspace (default: "microservices")
///
/// # Returns
/// 0 on success, 1 on failure.
pub fn copy_artifacts(
    arch: &str,
    project_root: &Path,
    package_names: &HashMap<String, String>,
    binary_names: &HashMap<String, String>,
    workspace_dir: &str,
) -> i32 {
    // Validate arch
    let target_triple = match ARCH_TARGETS.iter().find(|(a, _)| *a == arch) {
        Some((_, triple)) => *triple,
        None => {
            eprintln!("❌ Unknown arch: {}. Use amd64, arm64, or arm7.", arch);
            return 1;
        }
    };

    let artifact_dir = match ARCH_TO_ARTIFACT_DIR.iter().find(|(a, _)| *a == arch) {
        Some((_, dir)) => *dir,
        None => "amd64",
    };

    let release_dir = project_root
        .join(workspace_dir)
        .join("target")
        .join(target_triple)
        .join("release");

    let out_dir = project_root.join("build_artifacts").join(artifact_dir);
    fs::create_dir_all(&out_dir).ok();

    let mut errors = 0;

    for (service_name, pkg) in package_names {
        let bin_name = binary_names
            .get(service_name)
            .map(|s| s.as_str())
            .unwrap_or(pkg.as_str());

        let src = release_dir.join(pkg);
        let dst = out_dir.join(bin_name);

        if !src.exists() {
            eprintln!(
                "❌ Binary not found: {} (run build microservices {} --release)",
                src.display(),
                arch
            );
            errors += 1;
            continue;
        }

        // Copy with permissions
        fs::copy(&src, &dst).ok();
        if let Ok(mut permissions) = fs::metadata(&dst).map(|m| m.permissions().clone()) {
            // Set executable
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o755);
            fs::set_permissions(&dst, permissions).ok();
        }

        let rel_dst = dst
            .strip_prefix(project_root)
            .unwrap_or(&dst)
            .display();
        println!("📦 Copying {}: {} -> {}", service_name, pkg, rel_dst);
    }

    if errors > 0 {
        eprintln!("❌ Failed to copy {} artifact(s)", errors);
        return 1;
    }

    println!("✅ Copied to build_artifacts/{}/", artifact_dir);
    0
}

/// Validate that build_artifacts contains expected binaries for all archs.
///
/// # Arguments
/// * `project_root` - Project root directory
/// * `binary_names` - service_name -> artifact name
///
/// # Returns
/// 0 on success, 1 on failure.
pub fn validate_artifacts(
    project_root: &Path,
    binary_names: &HashMap<String, String>,
) -> i32 {
    let required: Vec<&str> = binary_names.values().map(|v| v.as_str()).collect();
    let required_set: std::collections::HashSet<&str> = required.iter().copied().collect();

    let mut errors = 0;

    for arch_dir in &["amd64", "arm64", "arm"] {
        let d = project_root.join("build_artifacts").join(arch_dir);
        if !d.is_dir() {
            eprintln!("❌ Missing: {}", d.strip_prefix(project_root).unwrap_or(&d).display());
            errors += 1;
            continue;
        }

        let found: std::collections::HashSet<String> = d
            .read_dir()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map_or(false, |ft| ft.is_file()))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| required_set.contains(name.as_str()))
            .collect();

        let missing: Vec<String> = required_set
            .iter()
            .copied()
            .filter(|n| !found.contains(*n))
            .map(|s| s.to_string())
            .collect();

        if !missing.is_empty() {
            eprintln!(
                "❌ {}/: missing {}",
                d.strip_prefix(project_root).unwrap_or(&d).display(),
                missing.join(", ")
            );
            errors += 1;
        } else {
            println!("✅ {}: {}/{} binaries", arch_dir, found.len(), required_set.len());
        }
    }

    if errors > 0 {
        1
    } else {
        0
    }
}

/// Copy artifacts for a specific suite only.
///
/// # Arguments
/// * `arch` - Architecture
/// * `project_root` - Project root
/// * `package_names` - Full package map (will be filtered by suite)
/// * `binary_names` - Full binary map
/// * `suite` - Optional suite name filter
/// * `workspace_dir` - Relative path to workspace
///
/// # Returns
/// 0 on success, 1 on failure.
pub fn copy_artifacts_for_suite(
    arch: &str,
    project_root: &Path,
    package_names: &HashMap<String, String>,
    binary_names: &HashMap<String, String>,
    suite: Option<&str>,
    workspace_dir: &str,
) -> i32 {
    let (filtered_pkgs, filtered_bins) = if let Some(_target_suite) = suite {
        // Hauliage convention: package names start with "hauliage_"
        let svc_prefix = format!("hauliage_");
        let pkgs: HashMap<String, String> = package_names
            .iter()
            .filter(|(k, _)| k.starts_with(&svc_prefix))
            .map(|(k, v)| {
                // Extract service name from package name
                let svc = k.strip_prefix(&svc_prefix).unwrap_or(k);
                let svc_normalized = svc.to_string().replace('_', "-");
                (svc_normalized, v.clone())
            })
            .collect();
        let bins: HashMap<String, String> = binary_names
            .iter()
            .filter(|(k, _)| k.starts_with(&svc_prefix) || *k == "bff")
            .map(|(k, v)| {
                let svc = k.strip_prefix(&svc_prefix).unwrap_or(k);
                let svc_normalized = svc.to_string().replace('_', "-");
                (svc_normalized, v.clone())
            })
            .collect();
        (pkgs, bins)
    } else {
        (package_names.clone(), binary_names.clone())
    };

    copy_artifacts(arch, project_root, &filtered_pkgs, &filtered_bins, workspace_dir)
}

#[cfg(test)]
mod tests;
