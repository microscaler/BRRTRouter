use std::path::Path;
use std::process::Command;

/// Format a generated project using `cargo fmt`
///
/// # Arguments
///
/// * `dir` - Directory containing the Rust project to format
///
/// # Returns
///
/// `Ok(())` on success
///
/// # Errors
///
/// Returns an error if `cargo fmt` fails or is not installed.
pub fn format_project(dir: &Path) -> anyhow::Result<()> {
    // Allow tests to override the cargo binary path without mutating PATH
    let cargo_bin = std::env::var("BRRTR_CARGO_BIN").unwrap_or_else(|_| "cargo".to_string());

    let mut cmd = Command::new(cargo_bin);
    cmd.arg("fmt").current_dir(dir);
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("cargo fmt failed");
    }
    Ok(())
}

/// Format one generated file with `rustfmt --edition <edition>`. Unlike [`format_project`] this needs no Cargo
/// workspace around the file, so the output is the same wherever the crate sits (outside its workspace `cargo fmt`
/// failed and the file was left unformatted).
///
/// # Errors
///
/// Returns an error if `rustfmt` fails or is not installed.
pub fn format_file(file: &Path, edition: &str) -> anyhow::Result<()> {
    let rustfmt = std::env::var("BRRTR_RUSTFMT_BIN").unwrap_or_else(|_| "rustfmt".to_string());
    let status = Command::new(rustfmt)
        .arg("--edition")
        .arg(edition)
        .arg(file)
        .status()?;
    if !status.success() {
        anyhow::bail!("rustfmt failed on {}", file.display());
    }
    Ok(())
}

/// The Rust edition of the crate in `crate_dir`, as Cargo reads it: its `package.edition`; with
/// `edition.workspace = true`, the nearest `workspace.package.edition` from the crate's own manifest (a workspace
/// root can be a package too) upwards; with no edition at all, Cargo's default, 2015.
#[must_use]
pub fn crate_edition(crate_dir: &Path) -> String {
    fn manifest(dir: &Path) -> Option<toml::Value> {
        toml::from_str(&std::fs::read_to_string(dir.join("Cargo.toml")).ok()?).ok()
    }
    let dir = crate_dir
        .canonicalize()
        .unwrap_or_else(|_| crate_dir.to_path_buf());
    let own = manifest(&dir);
    match own.as_ref().and_then(|m| m.get("package")?.get("edition")) {
        Some(toml::Value::String(edition)) => edition.clone(),
        Some(inherit) if inherit.get("workspace").and_then(toml::Value::as_bool) == Some(true) => {
            dir.ancestors()
                .find_map(|d| {
                    manifest(d).and_then(|m| {
                        m.get("workspace")?
                            .get("package")?
                            .get("edition")?
                            .as_str()
                            .map(str::to_string)
                    })
                })
                .unwrap_or_else(|| "2015".to_string())
        }
        _ => "2015".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Mutex, OnceLock};

    /// Check if cargo fmt is available
    fn is_cargo_fmt_available() -> bool {
        std::process::Command::new("cargo")
            .arg("fmt")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    // Serialize environment mutations to avoid test races
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn test_format_project_noop() {
        if !is_cargo_fmt_available() {
            println!("Skipping test: cargo fmt not available");
            return;
        }
        let dir = std::env::temp_dir().join(format!("fmt_test_noop_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stub = dir.join("cargo");
        fs::write(
            &stub,
            "#!/bin/sh\nif [ \"$1\" = \"fmt\" ]; then\n    exit 0\nfi\nexit 0\n",
        )
        .unwrap();
        let mut perms = fs::metadata(&stub).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&stub, perms).unwrap();
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let old_bin = env::var("BRRTR_CARGO_BIN").ok();
        env::set_var("BRRTR_CARGO_BIN", &stub);
        let res = format_project(&dir);
        match old_bin {
            Some(v) => env::set_var("BRRTR_CARGO_BIN", v),
            None => env::remove_var("BRRTR_CARGO_BIN"),
        }
        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
        assert!(res.is_ok());
    }

    #[test]
    fn test_format_project_error() {
        if !is_cargo_fmt_available() {
            println!("Skipping test: cargo fmt not available");
            return;
        }
        let dir = std::env::temp_dir().join(format!("fmt_test_err_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stub = dir.join("cargo");
        fs::write(
            &stub,
            "#!/bin/sh\nif [ \"$1\" = \"fmt\" ]; then\n    exit 1\nfi\nexit 0\n",
        )
        .unwrap();
        let mut perms = fs::metadata(&stub).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&stub, perms).unwrap();
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let old_bin = env::var("BRRTR_CARGO_BIN").ok();
        env::set_var("BRRTR_CARGO_BIN", &stub);
        let res = format_project(&dir);
        match old_bin {
            Some(v) => env::set_var("BRRTR_CARGO_BIN", v),
            None => env::remove_var("BRRTR_CARGO_BIN"),
        }
        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
        assert!(res.is_err());
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "brrtr_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn crate_edition_reads_the_package_then_the_workspace() {
        let base = temp("edition");
        let member = base.join("member");
        fs::create_dir_all(&member).unwrap();
        fs::write(
            base.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n[workspace.package]\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            member.join("Cargo.toml"),
            "[package]\nname = \"m\"\nedition.workspace = true\n",
        )
        .unwrap();
        assert_eq!(crate_edition(&member), "2024");
        fs::write(
            member.join("Cargo.toml"),
            "[package]\nname = \"m\"\nedition = \"2018\"\n",
        )
        .unwrap();
        assert_eq!(crate_edition(&member), "2018");
        // no edition: Cargo's default, even under a workspace that names one
        fs::write(member.join("Cargo.toml"), "[package]\nname = \"m\"\n").unwrap();
        assert_eq!(crate_edition(&member), "2015");
        // a workspace root that is also the package inherits from its own manifest
        fs::write(
            base.join("Cargo.toml"),
            "[package]\nname = \"root\"\nedition.workspace = true\n[workspace]\n[workspace.package]\nedition = \"2024\"\n",
        )
        .unwrap();
        assert_eq!(crate_edition(&base), "2024");
        fs::remove_file(base.join("Cargo.toml")).unwrap();
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn format_file_formats_one_file_outside_any_workspace() {
        let has_rustfmt = Command::new("rustfmt")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !has_rustfmt {
            return;
        }
        let dir = temp("format_file");
        let file = dir.join("impl_registry.rs");
        fs::write(&file, "fn  main( ){let x=1;}").unwrap();
        format_file(&file, "2021").unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "fn main() {\n    let x = 1;\n}\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
