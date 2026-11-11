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

    def test_uses_cargo_run(self, tmp_path: Path) -> None:
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

    def test_passes_package_name_when_provided(self, tmp_path: Path) -> None:
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
                package_name="acme_accounting_financial_reports_gen",
                capture_output=True,
            )
            cmd = run.call_args[0][0]
            assert "--package-name" in cmd
            assert "acme_accounting_financial_reports_gen" in cmd


PIN = "0123456789abcdef0123456789abcdef01234567"


def _consumer(base: Path, rev: str | None = PIN) -> Path:
    """A consumer project whose microservices/Cargo.toml pins BRRTRouter at `rev`."""
    root = base / "consumer"
    (root / "microservices").mkdir(parents=True)
    if rev:
        (root / "microservices" / "Cargo.toml").write_text(
            "[workspace.dependencies]\n"
            f'brrtrouter = {{ git = "https://github.com/microscaler/BRRTRouter.git", rev = "{rev}" }}\n'
        )
    return root


def _checkout(base: Path) -> Path:
    """A sibling BRRTRouter git checkout."""
    co = base / "BRRTRouter"
    (co / ".git").mkdir(parents=True)
    (co / "Cargo.toml").write_text("[package]\n")
    return co


class TestGeneratorCommand:
    """generator_command: the generator at the consumer's pinned BRRTRouter rev."""

    @pytest.fixture(autouse=True)
    def _env(self, monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
        for k in ("BRRTROUTER_GEN", "BRRTROUTER_GEN_LOCAL"):
            monkeypatch.delenv(k, raising=False)
        monkeypatch.setenv("BRRTROUTER_GEN_CACHE", str(tmp_path / "cache"))

    def _installed(self, tmp_path: Path) -> Path:
        return tmp_path / "cache" / PIN / "bin" / "brrtrouter-gen"

    def test_reads_the_pinned_rev(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.gen.brrtrouter import pinned_rev

        assert pinned_rev(_consumer(tmp_path)) == PIN
        assert pinned_rev(_consumer(tmp_path / "unpinned", rev=None)) is None

    def test_uses_the_generator_installed_at_the_pin(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.gen.brrtrouter import generator_command

        installed = self._installed(tmp_path)
        installed.parent.mkdir(parents=True)
        installed.touch()
        with patch("brrtrouter_tooling.gen.brrtrouter.subprocess.run") as run:
            assert generator_command(_consumer(tmp_path)) == [str(installed)]
            run.assert_not_called()

    def test_uses_the_checkout_when_it_is_at_the_pin(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.gen.brrtrouter import generator_command

        co = _checkout(tmp_path)
        with patch("brrtrouter_tooling.gen.brrtrouter.subprocess.run") as run:
            run.return_value = MagicMock(returncode=0, stdout=PIN + "\n")
            cmd = generator_command(_consumer(tmp_path))
        assert cmd[:2] == ["cargo", "run"]
        assert str(co / "Cargo.toml") in cmd

    def test_installs_at_the_pin_when_the_checkout_is_at_another_rev(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.gen.brrtrouter import generator_command

        _checkout(tmp_path)
        installed = self._installed(tmp_path)

        def fake(cmd: list[str], **_kw: object) -> MagicMock:
            if cmd[0] == "git":
                return MagicMock(returncode=0, stdout="f" * 40)
            assert cmd[:2] == ["cargo", "install"]
            assert cmd[cmd.index("--rev") + 1] == PIN
            installed.parent.mkdir(parents=True)
            installed.touch()
            return MagicMock(returncode=0)

        with patch("brrtrouter_tooling.gen.brrtrouter.subprocess.run", side_effect=fake):
            assert generator_command(_consumer(tmp_path)) == [str(installed)]

    def test_refuses_another_rev_when_it_cannot_install(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.gen.brrtrouter import generator_command

        _checkout(tmp_path)

        def fake(cmd: list[str], **_kw: object) -> MagicMock:
            if cmd[0] == "git":
                return MagicMock(returncode=0, stdout="f" * 40)
            return MagicMock(returncode=101)

        with (
            patch("brrtrouter_tooling.gen.brrtrouter.subprocess.run", side_effect=fake),
            pytest.raises(RuntimeError, match="BRRTROUTER_GEN_LOCAL"),
        ):
            generator_command(_consumer(tmp_path))

    def test_the_local_override_uses_the_checkout_at_any_rev(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        from brrtrouter_tooling.gen.brrtrouter import generator_command

        co = _checkout(tmp_path)
        monkeypatch.setenv("BRRTROUTER_GEN_LOCAL", "1")
        with patch("brrtrouter_tooling.gen.brrtrouter.subprocess.run") as run:
            cmd = generator_command(_consumer(tmp_path))
            run.assert_not_called()
        assert cmd[:2] == ["cargo", "run"]
        assert str(co / "Cargo.toml") in cmd

    def test_generate_runs_the_pinned_generator(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.gen.brrtrouter import call_brrtrouter_generate

        installed = self._installed(tmp_path)
        installed.parent.mkdir(parents=True)
        installed.touch()
        with patch("brrtrouter_tooling.gen.brrtrouter.subprocess.run") as run:
            run.return_value = MagicMock(returncode=0)
            call_brrtrouter_generate(
                spec_path=tmp_path / "openapi.yaml",
                output_dir=tmp_path / "gen",
                project_root=_consumer(tmp_path),
            )
            cmd = run.call_args[0][0]
        assert cmd[:2] == [str(installed), "generate"]
