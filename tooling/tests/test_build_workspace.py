"""Tests for brrtrouter_tooling.build.workspace_build."""

from pathlib import Path
from unittest.mock import patch


class TestBuildWorkspaceWithOptions:
    def test_returns_1_when_manifest_missing(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.build import build_workspace_with_options

        rc = build_workspace_with_options(tmp_path, workspace_dir="microservices", arch="amd64")
        assert rc == 1

    def test_calls_gen_callback_before_build(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.build import build_workspace_with_options

        (tmp_path / "microservices").mkdir()
        (tmp_path / "microservices" / "Cargo.toml").write_text("[workspace]\n")
        called = []

        def gen_cb(root: Path) -> None:
            called.append(root)

        with patch("brrtrouter_tooling.build.workspace_build.subprocess.run") as m_run:
            m_run.return_value = type("R", (), {"returncode": 0})()
            rc = build_workspace_with_options(
                tmp_path,
                workspace_dir="microservices",
                arch="amd64",
                gen_if_missing_callback=gen_cb,
            )
        assert rc == 0
        assert called == [tmp_path]
        assert m_run.called

    def test_arm7_adds_no_default_features(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.build import build_workspace_with_options

        (tmp_path / "microservices").mkdir()
        (tmp_path / "microservices" / "Cargo.toml").write_text("[workspace]\n")
        with patch("brrtrouter_tooling.build.workspace_build.subprocess.run") as m_run:
            m_run.return_value = type("R", (), {"returncode": 0})()
            build_workspace_with_options(tmp_path, workspace_dir="microservices", arch="arm7")
        (cmd,) = m_run.call_args[0]
        assert "--no-default-features" in cmd
        assert "armv7-unknown-linux-musleabihf" in cmd


class TestBuildPackageWithOptions:
    def test_returns_1_when_manifest_missing(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.build import build_package_with_options

        rc = build_package_with_options(
            tmp_path, workspace_dir="microservices", package_name="my_crate"
        )
        assert rc == 1

    def test_returns_1_when_package_name_empty(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.build import build_package_with_options

        (tmp_path / "microservices").mkdir()
        (tmp_path / "microservices" / "Cargo.toml").write_text("[workspace]\n")
        rc = build_package_with_options(tmp_path, workspace_dir="microservices", package_name="")
        assert rc == 1

    def test_calls_gen_callback_and_builds_package(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.build import build_package_with_options

        (tmp_path / "microservices").mkdir()
        (tmp_path / "microservices" / "Cargo.toml").write_text("[workspace]\n")
        called = []

        def gen_cb(root: Path) -> None:
            called.append(root)

        with patch("brrtrouter_tooling.build.workspace_build.subprocess.run") as m_run:
            m_run.return_value = type("R", (), {"returncode": 0})()
            rc = build_package_with_options(
                tmp_path,
                workspace_dir="microservices",
                package_name="my_crate",
                gen_if_missing_callback=gen_cb,
            )
        assert rc == 0
        assert called == [tmp_path]
        (cmd,) = m_run.call_args[0]
        assert "-p" in cmd
        assert "my_crate" in cmd
