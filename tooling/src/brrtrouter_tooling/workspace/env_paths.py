"""Shared BRRTRouter Python tooling layout (see BRRTRouter tooling README + MCP tilt-setup guide)."""

from __future__ import annotations

import os
from pathlib import Path


def brrtrouter_venv_root() -> Path:
    """Return the directory that contains bin/hauliage, bin/brrtrouter, etc.

    Override with env ``BRRTROUTER_VENV``; default ``~/.local/share/brrtrouter/venv``.
    """
    override = os.environ.get("BRRTROUTER_VENV", "").strip()
    if override:
        return Path(override).expanduser().resolve()
    return Path.home() / ".local" / "share" / "brrtrouter" / "venv"


def venv_bin(*parts: str) -> str:
    """Absolute path under the shared venv's ``bin/`` (e.g. ``venv_bin('hauliage')``)."""
    return str(brrtrouter_venv_root().joinpath("bin", *parts))


def discover_brrtrouter_root(project_root: Path) -> Path:
    """Resolve the BRRTRouter checkout for codegen and Cargo path deps.

    Resolution order:

    1. ``BRRTROUTER_ROOT`` (absolute path, or relative to ``project_root``).
    2. ``project_root/../BRRTRouter`` (e.g. ``microscaler/hauliage`` → ``microscaler/BRRTRouter``).
    3. ``project_root/../../BRRTRouter`` (e.g. ``microscaler/hauliage/microservices`` or legacy
       ``microscaler/ai/hauliage`` → ``microscaler/BRRTRouter``).
    4. ``project_root/../../../BRRTRouter`` (e.g. legacy ``microscaler/ai/hauliage/microservices`` →
       ``microscaler/BRRTRouter``).

    If no candidate directory exists, returns the last candidate (three levels up) so callers
    surface a consistent "not found" path for the deepest layout.
    """
    override = os.environ.get("BRRTROUTER_ROOT", "").strip()
    if override:
        p = Path(override).expanduser()
        return (project_root / p).resolve() if not p.is_absolute() else p.resolve()

    candidates = [
        (project_root / ".." / "BRRTRouter").resolve(),
        (project_root / ".." / ".." / "BRRTRouter").resolve(),
        (project_root / ".." / ".." / ".." / "BRRTRouter").resolve(),
    ]
    for c in candidates:
        if c.is_dir():
            return c
    return candidates[-1]
