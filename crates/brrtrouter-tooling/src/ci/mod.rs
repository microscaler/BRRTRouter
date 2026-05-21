//! # Post-generation fixes
//!
//! Mirrors `workspace/ci/fix_cargo_paths.py`. Rewrites BRRTRouter path
//! dependencies in generated Cargo.toml files to use workspace deps
//! or correct relative paths.

use std::path::Path;

use crate::paths;

/// Regex to match a path dependency line for brrtrouter or brrtrouter_macros.
static BRRTRouter_PATH_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r#"brrtrouter(?:_macros)? = \{ path = "[^"]+" }"#).unwrap()
});

/// Check if a Cargo.toml is under microservices/.../gen (workspace member).
fn is_microservices_gen_crate(cargo_toml_dir: &Path) -> bool {
    match cargo_toml_dir.canonicalize() {
        Ok(resolved) => {
            let parts: Vec<_> = resolved
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect();
            parts.iter().any(|p| p.as_ref() == "microservices")
                && resolved.file_name().map_or(false, |n| n == "gen")
        }
        Err(_) => false,
    }
}

fn replace_backslashes(s: &str) -> String {
    s.replace('\\', "/")
}

/// Fix BRRTRouter deps in a Cargo.toml file.
///
/// If under microservices/.../gen: use `workspace = true` for brrtrouter and brrtrouter_macros.
/// Otherwise: set path to BRRTRouter repo (`project_root.parent / BRRTRouter`).
///
/// # Returns
/// `true` if content was changed.
pub fn fix_cargo_toml(
    cargo_toml_path: &Path,
    project_root: Option<&Path>,
) -> Result<bool, anyhow::Error> {
    if !cargo_toml_path.exists() {
        eprintln!(
            "Warning: {} does not exist, skipping",
            cargo_toml_path.display()
        );
        return Ok(false);
    }

    let content = std::fs::read_to_string(cargo_toml_path)?;
    let cargo_toml_dir = cargo_toml_path.parent().unwrap();

    if is_microservices_gen_crate(cargo_toml_dir) {
        let fixed = BRRTRouter_PATH_RE.replace_all(&content, |caps: &regex::Captures| {
            let original = caps.get(0).unwrap().as_str();
            if original.contains("brrtrouter_macros") {
                "brrtrouter_macros = { workspace = true }"
            } else {
                "brrtrouter = { workspace = true }"
            }
        });
        let fixed = fixed.to_string();

        if fixed != content {
            std::fs::write(cargo_toml_path, &fixed)?;
            eprintln!("Fixed paths in {}", cargo_toml_path.display());
            return Ok(true);
        }
        eprintln!("No changes needed in {}", cargo_toml_path.display());
        return Ok(false);
    }

    // Non-workspace: set path to BRRTRouter repo.
    let root = project_root.map(Path::to_path_buf).unwrap_or_else(|| {
        cargo_toml_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    });

    let brrtrouter_path = paths::discover_brrtrouter_root(&root);
    let rel = std::path::absolute(&brrtrouter_path).ok();

    if let Some(rel) = rel {
        let brrtrouter_rel = match rel.strip_prefix(cargo_toml_dir) {
            Ok(r) => replace_backslashes(&r.to_string_lossy()),
            Err(_) => replace_backslashes(&rel.to_string_lossy()),
        };
        let macros_rel = match brrtrouter_path.canonicalize().and_then(|p| {
            let _macros = p.join("brrtrouter_macros");
            p.canonicalize()
        }) {
            Ok(m) => match m.strip_prefix(cargo_toml_dir) {
                Ok(r) => replace_backslashes(&r.to_string_lossy()),
                Err(_) => replace_backslashes(&m.to_string_lossy()),
            },
            Err(_) => brrtrouter_rel.clone(),
        };

        let fixed = BRRTRouter_PATH_RE.replace_all(&content, |caps: &regex::Captures| {
            let original = caps.get(0).unwrap().as_str();
            if original.contains("brrtrouter_macros") {
                format!("brrtrouter_macros = {{ path = \"{}\" }}", macros_rel)
            } else {
                format!("brrtrouter = {{ path = \"{}\" }}", brrtrouter_rel)
            }
        });
        let fixed = fixed.to_string();

        if fixed != content {
            std::fs::write(cargo_toml_path, &fixed)?;
            eprintln!("Fixed paths in {}", cargo_toml_path.display());
            return Ok(true);
        }
    }

    eprintln!("No changes needed in {}", cargo_toml_path.display());
    Ok(false)
}

#[cfg(test)]
mod tests;
