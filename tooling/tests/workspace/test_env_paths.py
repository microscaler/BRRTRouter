"""Shared BRRTRouter venv path helpers."""

from __future__ import annotations

from pathlib import Path

from brrtrouter_tooling.workspace.env_paths import (
    brrtrouter_venv_root,
    discover_brrtrouter_root,
    venv_bin,
)


def test_brrtrouter_venv_root_respects_env(monkeypatch, tmp_path: Path) -> None:
    custom = tmp_path / "myvenv"
    monkeypatch.setenv("BRRTROUTER_VENV", str(custom))
    assert brrtrouter_venv_root() == custom.resolve()


def test_venv_bin_joins_under_bin(monkeypatch, tmp_path: Path) -> None:
    custom = tmp_path / "v"
    monkeypatch.setenv("BRRTROUTER_VENV", str(custom))
    assert venv_bin("acme") == str(custom.resolve() / "bin" / "acme")


def test_default_uses_home_when_env_unset(monkeypatch) -> None:
    monkeypatch.delenv("BRRTROUTER_VENV", raising=False)
    fake_home = Path("/tmp/fakehome")
    monkeypatch.setenv("HOME", str(fake_home))
    assert brrtrouter_venv_root() == fake_home / ".local" / "share" / "brrtrouter" / "venv"


def test_discover_brrtrouter_one_level_up(monkeypatch, tmp_path: Path) -> None:
    """microscaler/acme-style: project_root/../BRRTRouter."""
    monkeypatch.delenv("BRRTROUTER_ROOT", raising=False)
    root = tmp_path / "microscaler" / "acme"
    root.mkdir(parents=True)
    brr = tmp_path / "microscaler" / "BRRTRouter"
    brr.mkdir()
    assert discover_brrtrouter_root(root) == brr.resolve()


def test_discover_brrtrouter_acme_microservices_two_levels(monkeypatch, tmp_path: Path) -> None:
    """microscaler/acme/microservices: ../../BRRTRouter (canonical layout)."""
    monkeypatch.delenv("BRRTROUTER_ROOT", raising=False)
    root = tmp_path / "microscaler" / "acme" / "microservices"
    root.mkdir(parents=True)
    brr = tmp_path / "microscaler" / "BRRTRouter"
    brr.mkdir()
    assert discover_brrtrouter_root(root) == brr.resolve()


def test_discover_brrtrouter_ai_acme_two_levels(monkeypatch, tmp_path: Path) -> None:
    """Legacy microscaler/ai/acme: ../../BRRTRouter."""
    monkeypatch.delenv("BRRTROUTER_ROOT", raising=False)
    root = tmp_path / "microscaler" / "ai" / "acme"
    root.mkdir(parents=True)
    brr = tmp_path / "microscaler" / "BRRTRouter"
    brr.mkdir()
    assert discover_brrtrouter_root(root) == brr.resolve()


def test_discover_brrtrouter_ai_acme_microservices_three_levels(
    monkeypatch, tmp_path: Path
) -> None:
    """microscaler/ai/acme/microservices: ../../../BRRTRouter."""
    monkeypatch.delenv("BRRTROUTER_ROOT", raising=False)
    root = tmp_path / "microscaler" / "ai" / "acme" / "microservices"
    root.mkdir(parents=True)
    brr = tmp_path / "microscaler" / "BRRTRouter"
    brr.mkdir()
    assert discover_brrtrouter_root(root) == brr.resolve()


def test_discover_brrtrouter_respects_env(monkeypatch, tmp_path: Path) -> None:
    custom = tmp_path / "custom" / "BRRTRouter"
    custom.mkdir(parents=True)
    monkeypatch.setenv("BRRTROUTER_ROOT", str(custom))
    assert discover_brrtrouter_root(tmp_path / "acme") == custom.resolve()
