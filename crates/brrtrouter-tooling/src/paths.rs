//! # Path resolution for BRRTRouter checkout
//!
//! Mirrors the Python `workspace/env_paths.py` surface.

use std::env;
use std::path::{Path, PathBuf};

/// Return the shared venv root directory.
///
/// Override with `BRRTROUTER_VENV`; default `~/.local/share/brrtrouter/venv`.
pub fn brrtrouter_venv_root() -> PathBuf {
    env::var("BRRTROUTER_VENV")
        .ok()
        .and_then(|v| {
            let p = PathBuf::from(&v);
            if p.exists() {
                Some(p)
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".local")
                .join("share")
                .join("brrtrouter")
                .join("venv")
        })
}

/// Absolute path under the shared venv's `bin/`.
pub fn venv_bin(parts: &[&str]) -> PathBuf {
    let mut p = brrtrouter_venv_root();
    for part in parts {
        p = p.join(part);
    }
    p
}

/// Resolve the BRRTRouter checkout for codegen and Cargo path deps.
///
/// Resolution order:
/// 1. `BRRTROUTER_ROOT` env var (absolute or relative to project_root)
/// 2. `project_root/../BRRTRouter`
/// 3. `project_root/../../BRRTRouter`
/// 4. `project_root/../../../BRRTRouter`
pub fn discover_brrtrouter_root(project_root: &Path) -> PathBuf {
    if let Ok(override_path) = env::var("BRRTROUTER_ROOT") {
        let p = PathBuf::from(&override_path);
        let resolved = if p.is_absolute() {
            p
        } else {
            project_root.join(p)
        };
        if resolved.exists() {
            return resolved;
        }
    }

    let candidates = [
        project_root.join("../BRRTRouter"),
        project_root.join("../../BRRTRouter"),
        project_root.join("../../../BRRTRouter"),
    ];

    for c in &candidates {
        if c.is_dir() {
            return c.canonicalize().unwrap_or_else(|_| c.to_path_buf());
        }
    }

    // Return deepest candidate for consistent error messaging
    candidates.last().unwrap().to_path_buf()
}

/// Check if a path looks like a valid BRRTRouter checkout.
pub fn is_brrtrouter_root(path: &Path) -> bool {
    path.join("Cargo.toml").exists()
}
