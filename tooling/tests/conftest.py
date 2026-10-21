"""Pytest fixtures for BRRTRouter tooling tests."""

from pathlib import Path

import pytest


@pytest.fixture
def tmp_openapi_dir(tmp_path: Path) -> tuple[Path, Path]:
    """Temporary openapi-like tree for unit tests. Returns (root, openapi_path)."""
    openapi = tmp_path / "openapi"
    openapi.mkdir()
    return tmp_path, openapi


@pytest.fixture(autouse=True)
def acme_project(request: pytest.FixtureRequest, monkeypatch: pytest.MonkeyPatch):
    """Every test project is called ``acme`` (tmp directories are named after the test).

    test_project_profile.py checks the real directory-name default, so it is left alone.
    """
    from brrtrouter_tooling.workspace import profile

    profile.clear_cache()
    if Path(str(request.node.fspath)).name != "test_project_profile.py":
        monkeypatch.setattr(profile, "_slug", lambda _name: "acme")
    yield
    profile.clear_cache()
