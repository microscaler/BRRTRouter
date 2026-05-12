//! # Build orchestration
//!
//! Mirrors `workspace/build/microservices.py` + `build/host_aware.py`.
//! Host-aware multi-arch Rust builds via cargo/cross/zigbuild.
//!
//! This is the most complex module due to cross-compilation toolchain
//! management, jemalloc opt-in, and workspace vs single-package builds.

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Architecture target triples
pub const ARCH_TARGETS: &[(&str, &str)] = &[
    ("amd64", "x86_64-unknown-linux-musl"),
    ("arm64", "aarch64-unknown-linux-musl"),
    ("arm7", "armv7-unknown-linux-musleabihf"),
];

// armv7 musl does not provide __ffsdi2 (used by tikv-jemalloc-sys)
const ARM7_TARGET: &str = "aarch64-unknown-linux-musl"; // Actually armv7
const ARM7_TRIPLE: &str = "armv7-unknown-linux-musleabihf";

/// Detect host architecture as a canonical name.
pub fn detect_host_architecture() -> &'static str {
    let machine = std::env::var("CARGO_TARGET_ARCH")
        .ok()
        .unwrap_or_else(|| {
            std::env::var("TARGET")
                .ok()
                .unwrap_or_else(|| {
                    // Fallback to uname
                    String::from("x86_64")
                })
        });

    match machine.to_lowercase().as_str() {
        "x86_64" | "amd64" => "amd64",
        "aarch64" | "arm64" => "arm64",
        _ => "amd64",
    }
}

/// Whether to use cross instead of zigbuild/cargo.
pub fn should_use_cross() -> bool {
    env::var("Hauliage_USE_CROSS").ok().as_deref() == Some("1")
}

/// Whether to use zigbuild (macOS or non-x86_64 Linux).
pub fn should_use_zigbuild() -> bool {
    let system = env::var("TARGET_OS").unwrap_or_else(|_| {
        // Simple heuristic
        if cfg!(target_os = "macos") {
            "macos".to_string()
        } else {
            "linux".to_string()
        }
    });

    system == "macos"
}

/// Find the cargo binary.
fn find_cargo() -> String {
    which::which("cargo")
        .map(|p| p.to_string_lossy().to_string())
        .ok()
        .or_else(|| {
            dirs::home_dir().and_then(|home| {
                let fallback = home.join(".cargo").join("bin").join("cargo");
                if fallback.exists() {
                    Some(fallback.to_string_lossy().to_string())
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| "cargo".to_string())
}

/// Get the linker environment variables for a target triple.
fn get_linker_env(rust_target: &str) -> Vec<(&str, &str)> {
    match rust_target {
        "x86_64-unknown-linux-musl" => vec![
            ("CC_x86_64_unknown_linux_musl", "musl-gcc"),
            (
                "CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER",
                "musl-gcc",
            ),
        ],
        "aarch64-unknown-linux-musl" => vec![
            ("CC_aarch64_unknown_linux_musl", "aarch64-linux-musl-gcc"),
            (
                "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER",
                "aarch64-linux-musl-gcc",
            ),
        ],
        "armv7-unknown-linux-musleabihf" => vec![
            (
                "CC_armv7_unknown_linux_musleabihf",
                "arm-linux-musleabihf-gcc",
            ),
            (
                "CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_LINKER",
                "arm-linux-musleabihf-gcc",
            ),
        ],
        _ => vec![],
    }
}

/// Run rustup target add for a target if not already installed.
fn install_rust_target(rust_target: &str) -> bool {
    let cargo = find_cargo();
    let check = Command::new(&cargo)
        .arg("target")
        .arg("list")
        .arg("--installed")
        .output();

    match check {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.lines().any(|l| l.trim() == rust_target) {
                return true;
            }
        }
        _ => {}
    }

    println!("📦 Installing Rust target: {}", rust_target);
    let install = Command::new(&cargo)
        .arg("target")
        .arg("add")
        .arg(rust_target)
        .status();

    match install {
        Ok(code) => code.success(),
        Err(_) => false,
    }
}

/// Build a microservices workspace for a given architecture.
///
/// # Arguments
/// * `project_root` - Project root
/// * `arch` - Architecture: "amd64", "arm64", or "arm7"
/// * `release` - Build in release mode
/// * `jemalloc` - Enable jemalloc feature (not for arm7)
///
/// # Returns
/// 0 on success, 1 on failure.
pub fn build_workspace(
    project_root: &Path,
    arch: &str,
    release: bool,
    jemalloc: bool,
) -> i32 {
    let target = match ARCH_TARGETS.iter().find(|(a, _)| *a == arch) {
        Some((_, triple)) => *triple,
        None => {
            eprintln!("❌ Unknown arch: {}", arch);
            return 1;
        }
    };

    // Install target if needed
    if !install_rust_target(target) {
        eprintln!("❌ Failed to install Rust target: {}", target);
        return 1;
    }

    let manifest = project_root.join("microservices").join("Cargo.toml");
    if !manifest.exists() {
        eprintln!("❌ Cargo.toml not found in {}", project_root.join("microservices").display());
        return 1;
    }

    let cargo = find_cargo();
    let mut cmd = vec![
        cargo.clone(),
        if should_use_cross() {
            "cross".to_string()
        } else if should_use_zigbuild() {
            "zigbuild".to_string()
        } else {
            cargo.clone()
        },
        "build".to_string(),
        "--manifest-path".to_string(),
        manifest.display().to_string(),
        "--target".to_string(),
        target.to_string(),
    ];

    if release {
        cmd.push("--release".to_string());
    }

    // arm7: build packages individually with --no-default-features (no jemalloc)
    if arch == "arm7" {
        // Get workspace packages
        let packages = get_workspace_packages(&manifest, project_root);
        if packages.is_empty() {
            eprintln!("❌ Could not get workspace members for arm7");
            return 1;
        }
        cmd.push("--no-default-features".to_string());
        for pkg in &packages {
            cmd.push("-p".to_string());
            cmd.push(pkg.clone());
        }
    } else if jemalloc {
        cmd.push("--features".to_string());
        cmd.push("jemalloc".to_string());
    }

    cmd.push("--workspace".to_string());

    let mut command = Command::new(&cmd[0]);
    command.args(&cmd[1..]);
    command.current_dir(project_root.join("microservices"));

    // Set linker env vars (only for non-zigbuild)
    if !should_use_zigbuild() || should_use_cross() {
        for (key, val) in get_linker_env(target) {
            command.env(key, val);
        }
    }

    let output = command.output();

    match output {
        Ok(o) => {
            if o.status.success() {
                println!("✅ Built workspace for {} ({})", arch, target);
                0
            } else {
                let stderr = String::from_utf8_lossy(&o.stderr);
                eprintln!("❌ Build failed for {}: {}", arch, stderr);
                1
            }
        }
        Err(e) => {
            eprintln!("❌ Build failed for {}: {}", arch, e);
            1
        }
    }
}

/// Build a single microservice package.
///
/// # Arguments
/// * `project_root` - Project root
/// * `package_name` - Cargo package name (e.g. "hauliage_identity")
/// * `arch` - Architecture
/// * `release` - Build in release mode
/// * `gen_if_missing` - Callback to generate if gen crate missing
///
/// # Returns
/// 0 on success, 1 on failure.
pub fn build_package(
    project_root: &Path,
    package_name: &str,
    arch: &str,
    release: bool,
    gen_if_missing: Option<&dyn Fn()>,
) -> i32 {
    // Check if gen crate exists
    let crate_dir = project_root.join("microservices");
    if let Some(callback) = gen_if_missing {
        // Simple check: look for any gen/Cargo.toml
        let probe = crate_dir.read_dir()
            .into_iter()
            .flatten()
            .find(|e| {
                e.as_ref().map_or(false, |e| {
                    e.file_type().map_or(false, |ft| ft.is_dir())
                        && e.path().join("gen").join("Cargo.toml").exists()
                })
            });
        if probe.is_none() {
            println!(
                "📦 Gen crates missing; running generation for all services..."
            );
            callback();
        }
    }

    let target = match ARCH_TARGETS.iter().find(|(a, _)| *a == arch) {
        Some((_, triple)) => *triple,
        None => {
            eprintln!("❌ Unknown arch: {}", arch);
            return 1;
        }
    };

    if !install_rust_target(target) {
        eprintln!("❌ Failed to install Rust target: {}", target);
        return 1;
    }

    let cargo = find_cargo();
    let manifest = project_root.join("microservices").join("Cargo.toml");

    let mut cmd = vec![
        cargo.clone(),
        if should_use_cross() {
            "cross".to_string()
        } else if should_use_zigbuild() {
            "zigbuild".to_string()
        } else {
            cargo.clone()
        },
        "build".to_string(),
        "--manifest-path".to_string(),
        manifest.display().to_string(),
        "--target".to_string(),
        target.to_string(),
        "-p".to_string(),
        package_name.to_string(),
    ];

    if release {
        cmd.push("--release".to_string());
    }

    let mut command = Command::new(&cmd[0]);
    command.args(&cmd[1..]);
    command.current_dir(project_root.join("microservices"));

    if !should_use_zigbuild() || should_use_cross() {
        for (key, val) in get_linker_env(target) {
            command.env(key, val);
        }
    }

    let output = command.output();

    match output {
        Ok(o) => {
            if o.status.success() {
                println!("✅ Built package {} for {}", package_name, arch);
                0
            } else {
                let stderr = String::from_utf8_lossy(&o.stderr);
                eprintln!("❌ Build failed for {}: {}", package_name, stderr);
                1
            }
        }
        Err(e) => {
            eprintln!("❌ Build failed for {}: {}", package_name, e);
            1
        }
    }
}

/// Get workspace member package names via cargo metadata.
fn get_workspace_packages(manifest: &Path, project_root: &Path) -> Vec<String> {
    let cargo = find_cargo();
    let output = Command::new(&cargo)
        .arg("metadata")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .current_dir(project_root)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let meta: serde_json::Value = serde_json::from_str(&stdout).ok();
            if let Some(workspaces) = meta.and_then(|m| m.get("workspace_members")) {
                workspaces
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str())
                    .filter_map(|pid| {
                        // pid is like "path+file:///.../gen#hauliage_foo_gen@0.1.0"
                        if pid.contains('#') && pid.contains('@') {
                            pid.split('#').nth(1).and_then(|s| s.split('@').next())
                        } else {
                            Some(pid)
                        }
                    })
                    .map(|s| s.to_string())
                    .collect()
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}
