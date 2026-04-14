"""Tests for hauliage docker generate-dockerfile (delegates to brrtrouter_tooling.docker.generate_dockerfile)."""

from pathlib import Path

import pytest

pytest.importorskip("brrtrouter_tooling")


class TestGenerateDockerfile:
    def test_generates_file_with_substitutions(self, tmp_path: Path):
        from brrtrouter_tooling.docker.generate_dockerfile import generate_dockerfile

        (tmp_path / "docker" / "microservices").mkdir(parents=True)
        tpl = tmp_path / "docker" / "microservices" / "Dockerfile.template"
        tpl.write_text(
            "FROM x\nCOPY {{binary_name}}\nEXPOSE {{port}}\n# {{service_name}} {{system}} {{module}}\n"
        )
        out = generate_dockerfile(
            "auth",
            "idam",
            port=8000,
            project_root=tmp_path,
            binary_name_pattern="hauliage_{system}_{module}_impl",
        )
        assert out == tmp_path / "docker" / "microservices" / "Dockerfile.auth_idam"
        text = out.read_text()
        assert "hauliage_auth_idam_impl" in text
        assert "8000" in text
        assert "auth-idam" in text
        assert "auth" in text
        assert "idam" in text

    def test_uses_bundled_template_when_project_template_missing(self, tmp_path: Path):
        from brrtrouter_tooling.docker.generate_dockerfile import run

        (tmp_path / "docker" / "microservices").mkdir(parents=True)
        # No Dockerfile.template under project: falls back to bundled template
        assert run("x", "y", project_root=tmp_path) == 0
        out = tmp_path / "docker" / "microservices" / "Dockerfile.x_y"
        assert out.exists()

    def test_returns_1_when_no_template_in_project_or_bundle(self, tmp_path: Path, monkeypatch):
        from importlib import import_module

        gd = import_module("brrtrouter_tooling.docker.generate_dockerfile")

        (tmp_path / "docker" / "microservices").mkdir(parents=True)
        missing = tmp_path / "nowhere" / "Dockerfile.template"
        monkeypatch.setattr(gd, "bundled_microservices_template", lambda: missing)
        assert gd.run("x", "y", project_root=tmp_path) == 1


class TestRun:
    def test_run_returns_zero(self, tmp_path: Path):
        from brrtrouter_tooling.docker.generate_dockerfile import run

        (tmp_path / "docker" / "microservices").mkdir(parents=True)
        (tmp_path / "docker" / "microservices" / "Dockerfile.template").write_text("FROM x\n")
        assert run("a", "b", port=9000, project_root=tmp_path) == 0
        out = tmp_path / "docker" / "microservices" / "Dockerfile.a_b"
        assert out.exists()
