"""Unit tests for docker cleanup helpers."""

import os
from unittest.mock import MagicMock, patch

import pytest

pytest.importorskip("brrtrouter_tooling")


def test_env_prune_after_build():
    from brrtrouter_tooling.docker import cleanup

    with patch.dict(os.environ, {"BRRTR_DOCKER_PRUNE_DANGLING_AFTER_BUILD": "1"}, clear=True):
        assert cleanup.env_prune_after_build() is True
    with patch.dict(os.environ, {"BRRTR_DOCKER_PRUNE_DANGLING_AFTER_BUILD": "no"}, clear=True):
        assert cleanup.env_prune_after_build() is False


def test_prune_dangling_images_runs_docker():
    from brrtrouter_tooling.docker import cleanup

    with patch("brrtrouter_tooling.docker.cleanup.subprocess.run") as m:
        m.return_value = MagicMock(returncode=0)
        assert cleanup.prune_dangling_images() == 0
        m.assert_called_once()
        assert m.call_args[0][0][:3] == ["docker", "image", "prune"]


def test_prune_dev_sweep_returns_1_if_step_fails():
    from brrtrouter_tooling.docker import cleanup

    with patch("brrtrouter_tooling.docker.cleanup.subprocess.run") as m:
        m.return_value = MagicMock(returncode=1)
        assert cleanup.prune_dev_sweep() == 1
