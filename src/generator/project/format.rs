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

/// Format one generated file with `rustfmt --edition <edition>`, through rustfmt's standard input.
///
/// Given a path, rustfmt follows the file's `mod` declarations and reformats those files too, so a regenerated
/// `controllers/mod.rs` would rewrite every user-owned controller (`skip_children` is not stable); from standard input
/// it formats just this text. Unlike [`format_project`] this needs no Cargo workspace around the file, so the output
/// is the same wherever the crate sits.
///
/// # Errors
///
/// Returns an error if the file cannot be read or written, or `rustfmt` is not installed or fails.
pub fn format_file(file: &Path, edition: &str) -> anyhow::Result<()> {
    use std::io::Write as _;
    let source = std::fs::read(file)?;
    let rustfmt = std::env::var("BRRTR_RUSTFMT_BIN").unwrap_or_else(|_| "rustfmt".to_string());
    let mut child = Command::new(rustfmt)
        .arg("--edition")
        .arg(edition)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    // rustfmt reads all of its input before it writes; the handle is dropped (stdin closed) after this statement
    child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("rustfmt: no stdin"))?
        .write_all(&source)?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        anyhow::bail!("rustfmt failed on {}", file.display());
    }
    std::fs::write(file, out.stdout)?;
    Ok(())
}

/// The Rust edition of the crate in `crate_dir`, as Cargo reads it: its `package.edition`; with
/// `edition.workspace = true`, the nearest `workspace.package.edition` from the crate's own manifest (a workspace
/// root can be a package too) upwards, or from the root `package.workspace` names; with no edition at all,
/// Cargo's default, 2015.
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
            let workspace_edition = |d: &Path| {
                manifest(d).and_then(|m| {
                    m.get("workspace")?
                        .get("package")?
                        .get("edition")?
                        .as_str()
                        .map(str::to_string)
                })
            };
            // `package.workspace = "<path>"` names the workspace root; otherwise Cargo searches upwards
            if let Some(root) = own
                .as_ref()
                .and_then(|m| m.get("package")?.get("workspace")?.as_str())
            {
                return workspace_edition(&dir.join(root)).unwrap_or_else(|| "2015".to_string());
            }
            dir.ancestors()
                .find_map(workspace_edition)
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

    fn has_rustfmt() -> bool {
        Command::new("rustfmt")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn format_file_leaves_the_modules_it_declares_alone() {
        if !has_rustfmt() {
            return;
        }
        let dir = temp("format_children");
        let child = "fn  untouched( ){}\n";
        fs::write(dir.join("child.rs"), child).unwrap();
        fs::write(dir.join("mod.rs"), "pub   mod child;").unwrap();
        format_file(&dir.join("mod.rs"), "2021").unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("mod.rs")).unwrap(),
            "pub mod child;\n"
        );
        assert_eq!(fs::read_to_string(dir.join("child.rs")).unwrap(), child);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn crate_edition_follows_an_explicit_workspace_path() {
        let base = temp("edition_path");
        let (ws, member) = (base.join("ws"), base.join("member"));
        fs::create_dir_all(&ws).unwrap();
        fs::create_dir_all(&member).unwrap();
        fs::write(
            ws.join("Cargo.toml"),
            "[workspace]\nmembers = [\"../member\"]\n[workspace.package]\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            member.join("Cargo.toml"),
            "[package]\nname = \"m\"\nworkspace = \"../ws\"\nedition.workspace = true\n",
        )
        .unwrap();
        assert_eq!(crate_edition(&member), "2024");
        let _ = fs::remove_dir_all(&base);
    }
}
