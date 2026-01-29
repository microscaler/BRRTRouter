"""Pytest fixtures for BRRTRouter tooling tests."""

from pathlib import Path

import pytest


@pytest.fixture
def tmp_openapi_dir(tmp_path: Path) -> tuple[Path, Path]:
    """Temporary openapi-like tree for unit tests. Returns (root, openapi_path)."""
    openapi = tmp_path / "openapi"
    openapi.mkdir()
    return tmp_path, openapi
