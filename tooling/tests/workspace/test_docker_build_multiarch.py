"""Tests for hauliage docker build-multiarch (delegates to brrtrouter_tooling.docker.build_multiarch)."""

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

pytest.importorskip("brrtrouter_tooling")

from brrtrouter_tooling.workspace.env_paths import venv_bin


class TestBuildMultiarch:
    def test_returns_1_when_hauliage_build_fails(self, tmp_path: Path):
        from brrtrouter_tooling.docker.build_multiarch import run

        (tmp_path / "docker" / "microservices").mkdir(parents=True)
        (tmp_path / "docker" / "microservices" / "Dockerfile.template").write_text(
            "ARG BASE_IMAGE=ghcr.io/x/y:z\nFROM ${BASE_IMAGE}\n"
        )
        build_cmd = [venv_bin("hauliage"), "build", "auth_idam", "all"]
        with patch("subprocess.run") as m:
            m.return_value = MagicMock(returncode=1)
            rc = run(
                "auth",
                "idam",
                "img",
                "latest",
                False,
                tmp_path,
                build_cmd=build_cmd,
            )
        assert rc == 1
        assert m.call_count >= 1
        assert m.call_args[0][0][:3] == [venv_bin("hauliage"), "build", "auth_idam"]
