"""Pytest fixtures for workspace tooling tests (consumer-repo helpers)."""

from pathlib import Path

import pytest


@pytest.fixture
def repo_root() -> Path:
    """BRRTRouter repo root (parent of ``tooling/``). Tests under ``tests/workspace/`` use parents[2] = ``tooling``."""
    tooling = Path(__file__).resolve().parents[2]
    assert (tooling / "pyproject.toml").exists(), f"expected BRRTRouter tooling/ at {tooling}"
    root = tooling.parent
    assert (root / "tooling" / "pyproject.toml").exists(), (
        f"expected BRRTRouter repo root at {root}"
    )
    return root


@pytest.fixture
def tmp_openapi_dir(tmp_path: Path):
    """Temporary openapi-like tree for unit tests. Returns (root, openapi_path)."""
    openapi = tmp_path / "openapi"
    openapi.mkdir()
    return tmp_path, openapi
