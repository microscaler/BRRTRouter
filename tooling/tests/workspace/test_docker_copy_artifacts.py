from pathlib import Path

import pytest

pytest.importorskip("brrtrouter_tooling")

from brrtrouter_tooling.workspace.build.constants import get_binary_names, get_package_names
from brrtrouter_tooling.workspace.docker.copy_artifacts import run, validate_build_artifacts


def _make_openapi_layout(project_root: Path, services: list[tuple[str, str]]) -> None:
    for _suite, name in services:
        d = project_root / "openapi" / name
        d.mkdir(parents=True, exist_ok=True)
        (d / "openapi.yaml").write_text(
            f"openapi: 3.1.0\ninfo: {{}}\nservers:\n  - url: http://localhost:8001/api/v1/{name}\n"
        )
    (project_root / "openapi").mkdir(exist_ok=True, parents=True)
    services_yaml = "\n".join(f"  {name}: {{}}" for _, name in services)
    (project_root / "openapi" / "bff-suite-config.yaml").write_text(
        f"bff_service_name: bff\nservices:\n{services_yaml}\n"
    )


class TestValidateBuildArtifacts:
    def test_missing_dir_returns_1(self, tmp_path: Path):
        assert validate_build_artifacts(tmp_path) == 1

    def test_missing_binary_in_dir_returns_1(self, tmp_path: Path):
        _make_openapi_layout(tmp_path, [("hauliage", "identity"), ("hauliage", "fleet")])
        for arch in ("amd64", "arm64", "arm"):
            (tmp_path / "build_artifacts" / arch).mkdir(parents=True)
        (tmp_path / "build_artifacts" / "amd64" / "fleet").write_bytes(b"")
        assert validate_build_artifacts(tmp_path) == 1

    def test_all_present_returns_0(self, tmp_path: Path):
        _make_openapi_layout(tmp_path, [("hauliage", "identity"), ("hauliage", "fleet")])
        binary_names = get_binary_names(tmp_path)
        for arch in ("amd64", "arm64", "arm"):
            d = tmp_path / "build_artifacts" / arch
            d.mkdir(parents=True)
            for name in binary_names.values():
                (d / name).write_bytes(b"\x7fELF")
        assert validate_build_artifacts(tmp_path) == 0


class TestCopyArtifacts:
    def test_unknown_arch_returns_1(self, tmp_path: Path):
        assert run("x64", tmp_path) == 1

    def test_missing_binary_returns_1(self, tmp_path: Path):
        _make_openapi_layout(tmp_path, [("hauliage", "identity")])
        triple = "x86_64-unknown-linux-musl"
        (tmp_path / "microservices" / "target" / triple / "release").mkdir(parents=True)
        assert run("amd64", tmp_path) == 1

    def test_copies_all_to_build_artifacts_amd64(self, tmp_path: Path):
        _make_openapi_layout(tmp_path, [("hauliage", "identity"), ("hauliage", "fleet")])
        package_names = get_package_names(tmp_path)
        binary_names = get_binary_names(tmp_path)
        triple = "x86_64-unknown-linux-musl"
        rel = tmp_path / "microservices" / "target" / triple / "release"
        rel.mkdir(parents=True)
        for pkg in package_names.values():
            (rel / pkg).write_bytes(b"\x7fELF")
        assert run("amd64", tmp_path) == 0
        out = tmp_path / "build_artifacts" / "amd64"
        assert out.is_dir()
        for bin_name in binary_names.values():
            p = out / bin_name
            assert p.exists()

    def __skip_test_arm7(self):
        pass

    def xtest_arm7_uses_artifact_dir_arm(self, tmp_path: Path):
        _make_openapi_layout(tmp_path, [("hauliage", "identity")])
        package_names = get_package_names(tmp_path)
        triple = "armv7-unknown-linux-musleabihf"
        rel = tmp_path / "microservices" / "target" / triple / "release"
        rel.mkdir(parents=True)
        for pkg in package_names.values():
            (rel / pkg).write_bytes(b"\x7fELF")
        assert run("armv7", tmp_path) == 0
        assert (tmp_path / "build_artifacts" / "arm").is_dir()
