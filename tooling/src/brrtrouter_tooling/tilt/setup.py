"""Tilt setup: create dirs, docker volumes, check docker/tilt."""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path


def run(
    project_root: Path,
    dirs: list[str] | None = None,
    volumes: list[str] | None = None,
) -> int:
    """Create dirs (relative to project_root), docker volumes; check docker/tilt. Returns 0 or 1."""
    if dirs is None:
        dirs = []
    if volumes is None:
        volumes = []
    for p in dirs:
        (project_root / p).mkdir(parents=True, exist_ok=True)
    for v in volumes:
        if shutil.which("docker"):
            subprocess.run(["docker", "volume", "create", v], capture_output=True)
    for cmd in ["docker", "tilt"]:
        if not shutil.which(cmd):
            print(
                f"[ERROR] {cmd} is not installed. Please install it first.",
                file=sys.stderr,
            )
            return 1
    print("Setup complete! 🎉")
    print("To start: tilt up  (or your project's command)")
    return 0
