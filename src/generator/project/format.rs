use std::path::Path;

pub fn format_project(dir: &Path) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new("cargo");
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

    /// Check if cargo fmt is available
    fn is_cargo_fmt_available() -> bool {
        std::process::Command::new("cargo")
            .arg("fmt")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn test_format_project_noop() {
        if !is_cargo_fmt_available() {
            println!("Skipping test: cargo fmt not available");
            return;
        }
        let dir = std::env::temp_dir().join(format!("fmt_test_noop_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stub = dir.join("cargo");
        fs::write(&stub, "#!/bin/sh\nif [ \"$1\" = \"fmt\" ]; then\n    exit 0\nfi\nexit 0\n").unwrap();
        let mut perms = fs::metadata(&stub).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&stub, perms).unwrap();
        let old_path = env::var("PATH").unwrap();
        unsafe {
            env::set_var("PATH", format!("{}:{}", dir.display(), old_path));
        }
        let res = format_project(&dir);
        unsafe {
            env::set_var("PATH", old_path);
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
        fs::write(&stub, "#!/bin/sh\nif [ \"$1\" = \"fmt\" ]; then\n    exit 1\nfi\nexit 0\n").unwrap();
        let mut perms = fs::metadata(&stub).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&stub, perms).unwrap();
        let old_path = env::var("PATH").unwrap();
        unsafe {
            env::set_var("PATH", format!("{}:{}", dir.display(), old_path));
        }
        let res = format_project(&dir);
        unsafe {
            env::set_var("PATH", old_path);
        }
        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
        assert!(res.is_err());
    }
}
