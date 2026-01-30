"""TDD: tests for brrtrouter_tooling.ci.fix_cargo_paths (brrtrouter ci fix-cargo-paths)."""

from pathlib import Path


class TestFixCargoToml:
    def test_missing_file_returns_false(self, tmp_path: Path, capsys) -> None:
        from brrtrouter_tooling.ci import fix_cargo_toml

        p = tmp_path / "nonexistent" / "Cargo.toml"
        assert fix_cargo_toml(p) is False
        out, _ = capsys.readouterr()
        assert "Warning" in out and "nonexistent" in out

    def test_no_brrtrouter_dep_no_change(self, tmp_path: Path, capsys) -> None:
        from brrtrouter_tooling.ci import fix_cargo_toml

        (tmp_path / "microservices" / "accounting" / "svc").mkdir(parents=True)
        cargo = tmp_path / "microservices" / "accounting" / "svc" / "Cargo.toml"
        cargo.write_text('[package]\nname = "x"\nversion = "0.1.0"\n')
        assert fix_cargo_toml(cargo, project_root=tmp_path) is False
        assert "[package]" in cargo.read_text()
        out, _ = capsys.readouterr()
        assert "No changes" in out

    def test_replaces_brrtrouter_paths(self, tmp_path: Path, capsys) -> None:
        from brrtrouter_tooling.ci import fix_cargo_toml

        root = tmp_path / "root"
        (root / "microservices" / "accounting" / "svc").mkdir(parents=True)
        cargo = root / "microservices" / "accounting" / "svc" / "Cargo.toml"
        cargo.write_text(
            '[dependencies]\nbrrtrouter = { path = "old/brrtrouter" }\nbrrtrouter_macros = { path = "old/macros" }\n'
        )
        (tmp_path / "BRRTRouter").mkdir(parents=True, exist_ok=True)
        (tmp_path / "BRRTRouter" / "brrtrouter_macros").mkdir(parents=True, exist_ok=True)
        assert fix_cargo_toml(cargo, project_root=root) is True
        text = cargo.read_text()
        assert "old/brrtrouter" not in text
        assert "old/macros" not in text
        assert "BRRTRouter" in text
        assert "brrtrouter_macros" in text
        out, _ = capsys.readouterr()
        assert "Fixed" in out

    def test_gen_crate_config_sets_name_version_lib(self, tmp_path: Path, capsys) -> None:
        from brrtrouter_tooling.ci import fix_cargo_toml

        (tmp_path / "microservices" / "accounting" / "general-ledger" / "gen").mkdir(parents=True)
        cargo = tmp_path / "microservices" / "accounting" / "general-ledger" / "gen" / "Cargo.toml"
        cargo.write_text(
            '[package]\nname = "brrtrouter_gen"\nversion = "0.0.1"\n\n[dependencies]\n'
        )
        (tmp_path / "BRRTRouter").mkdir(parents=True, exist_ok=True)
        (tmp_path / "BRRTRouter" / "brrtrouter_macros").mkdir(parents=True, exist_ok=True)
        assert (
            fix_cargo_toml(
                cargo,
                project_root=tmp_path,
                gen_crate_config=("rerp_{suite}_{service}_gen", "0.1.3"),
            )
            is True
        )
        text = cargo.read_text()
        assert 'name = "rerp_accounting_general_ledger_gen"' in text
        assert 'version = "0.1.3"' in text
        assert "[lib]" in text
        assert 'path = "src/lib.rs"' in text


class TestRun:
    def test_run_returns_zero(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.ci import run_fix_cargo_paths as run

        (tmp_path / "microservices" / "accounting" / "svc").mkdir(parents=True)
        cargo = tmp_path / "microservices" / "accounting" / "svc" / "Cargo.toml"
        cargo.write_text('[package]\nname = "x"\n')
        assert run(cargo, project_root=tmp_path) == 0
