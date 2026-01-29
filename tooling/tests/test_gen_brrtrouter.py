"""TDD: tests for brrtrouter_tooling.gen.brrtrouter (brrtrouter gen generate | generate-stubs)."""

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest


class TestFindBrrtrouter:
    """Unit tests for find_brrtrouter(project_root, brrtrouter_path=None)."""

    def test_returns_bin_and_manifest_when_manifest_exists(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.gen.brrtrouter import find_brrtrouter

        project_root = tmp_path / "consumer"
        project_root.mkdir()
        brrtrouter_path = tmp_path / "BRRTRouter"
        brrtrouter_path.mkdir()
        (brrtrouter_path / "Cargo.toml").write_text('[package]\nname = "brrtrouter"\n')
        bin_path = brrtrouter_path / "target" / "debug" / "brrtrouter-gen"
        bin_path.parent.mkdir(parents=True, exist_ok=True)
        bin_path.touch()

        b, m = find_brrtrouter(project_root, brrtrouter_path=brrtrouter_path)
        assert b == bin_path
        assert m == brrtrouter_path / "Cargo.toml"

    def test_default_brrtrouter_path_is_parent_brrtrouter(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.gen.brrtrouter import find_brrtrouter

        project_root = tmp_path / "consumer"
        project_root.mkdir()
        brrtrouter_path = tmp_path / "BRRTRouter"
        brrtrouter_path.mkdir()
        (brrtrouter_path / "Cargo.toml").write_text("[package]\n")

        b, m = find_brrtrouter(project_root)
        assert m == brrtrouter_path / "Cargo.toml"
        assert b == brrtrouter_path / "target" / "debug" / "brrtrouter-gen"

    def test_raises_when_manifest_missing(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.gen.brrtrouter import find_brrtrouter

        project_root = tmp_path / "consumer"
        project_root.mkdir()
        brrtrouter_path = tmp_path / "BRRTRouter"
        brrtrouter_path.mkdir()
        # No Cargo.toml

        with pytest.raises(FileNotFoundError, match="BRRTRouter not found"):
            find_brrtrouter(project_root, brrtrouter_path=brrtrouter_path)


class TestCallBrrtrouterGenerate:
    """Unit tests for call_brrtrouter_generate (subprocess.run mocked)."""

    def test_uses_binary_when_exists(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.gen.brrtrouter import call_brrtrouter_generate

        project_root = tmp_path / "consumer"
        project_root.mkdir()
        brrtrouter_path = tmp_path / "BRRTRouter"
        brrtrouter_path.mkdir()
        (brrtrouter_path / "Cargo.toml").write_text("[package]\n")
        (brrtrouter_path / "target" / "debug" / "brrtrouter-gen").parent.mkdir(parents=True)
        (brrtrouter_path / "target" / "debug" / "brrtrouter-gen").touch()
        spec = tmp_path / "openapi.yaml"
        spec.write_text("openapi: 3.0.3\n")
        output_dir = tmp_path / "gen"
        output_dir.mkdir()

        with patch("brrtrouter_tooling.gen.brrtrouter.subprocess.run") as run:
            run.return_value = MagicMock(returncode=0)
            call_brrtrouter_generate(
                spec_path=spec,
                output_dir=output_dir,
                project_root=project_root,
                brrtrouter_path=brrtrouter_path,
                capture_output=True,
            )
            run.assert_called_once()
            cmd = run.call_args[0][0]
            assert cmd[0] == str(brrtrouter_path / "target" / "debug" / "brrtrouter-gen")
            assert "generate" in cmd
            assert "--spec" in cmd
            assert str(spec) in cmd
            assert "--output" in cmd
            assert str(output_dir) in cmd
            assert "--force" in cmd
            assert run.call_args[1]["cwd"] == str(project_root)

    def test_uses_cargo_run_when_binary_missing(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.gen.brrtrouter import call_brrtrouter_generate

        project_root = tmp_path / "consumer"
        project_root.mkdir()
        brrtrouter_path = tmp_path / "BRRTRouter"
        brrtrouter_path.mkdir()
        (brrtrouter_path / "Cargo.toml").write_text("[package]\n")
        # No target/debug/brrtrouter-gen
        spec = tmp_path / "openapi.yaml"
        spec.write_text("openapi: 3.0.3\n")
        output_dir = tmp_path / "gen"
        output_dir.mkdir()

        with patch("brrtrouter_tooling.gen.brrtrouter.subprocess.run") as run:
            run.return_value = MagicMock(returncode=0)
            call_brrtrouter_generate(
                spec_path=spec,
                output_dir=output_dir,
                project_root=project_root,
                brrtrouter_path=brrtrouter_path,
                capture_output=True,
            )
            cmd = run.call_args[0][0]
            assert cmd[0] == "cargo"
            assert "run" in cmd
            assert "--manifest-path" in cmd
            assert "brrtrouter-gen" in cmd

    def test_passes_deps_config_when_provided(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.gen.brrtrouter import call_brrtrouter_generate

        project_root = tmp_path / "consumer"
        project_root.mkdir()
        brrtrouter_path = tmp_path / "BRRTRouter"
        brrtrouter_path.mkdir()
        (brrtrouter_path / "Cargo.toml").write_text("[package]\n")
        (brrtrouter_path / "target" / "debug" / "brrtrouter-gen").parent.mkdir(parents=True)
        (brrtrouter_path / "target" / "debug" / "brrtrouter-gen").touch()
        spec = tmp_path / "openapi.yaml"
        spec.write_text("openapi: 3.0.3\n")
        output_dir = tmp_path / "gen"
        output_dir.mkdir()
        deps_config = tmp_path / "brrtrouter-dependencies.toml"
        deps_config.write_text("[deps]\n")

        with patch("brrtrouter_tooling.gen.brrtrouter.subprocess.run") as run:
            run.return_value = MagicMock(returncode=0)
            call_brrtrouter_generate(
                spec_path=spec,
                output_dir=output_dir,
                project_root=project_root,
                brrtrouter_path=brrtrouter_path,
                deps_config_path=deps_config,
                capture_output=True,
            )
            cmd = run.call_args[0][0]
            assert "--dependencies-config" in cmd
            assert str(deps_config) in cmd
