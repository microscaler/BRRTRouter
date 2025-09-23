use std::path::Path;
use std::process::Command;

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
}
