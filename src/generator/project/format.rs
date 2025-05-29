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

    #[test]
    fn test_format_project_noop() {
        let dir = std::env::temp_dir().join("fmt_test");
        std::fs::create_dir_all(&dir).unwrap();
        let stub = dir.join("cargo");
        fs::write(&stub, "#!/bin/sh\nexit 0\n").unwrap();
        let mut perms = fs::metadata(&stub).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&stub, perms).unwrap();
        let old_path = env::var("PATH").unwrap();
        env::set_var("PATH", format!("{}:{}", dir.display(), old_path));
        let res = format_project(&dir);
        env::set_var("PATH", old_path);
        assert!(res.is_ok());
    }
}
