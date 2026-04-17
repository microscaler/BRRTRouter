"""Tilt-only setup: create dirs, docker volumes, check deps, print instructions. Replaces setup-tilt.sh."""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

from brrtrouter_tooling.discovery.suites import _openapi_dir
from brrtrouter_tooling.ports.layout import resolve_layout


def run(project_root: Path) -> int:
    """Create dirs, docker volumes; check docker/tilt; print help. Returns 0 or 1."""
    layout = resolve_layout(None, project_root=project_root)
    openapi_dir = _openapi_dir(project_root, layout)

    # Generic paths that most projects might need
    for p in [
        openapi_dir,
        project_root / "microservices",
        project_root / "k8s/microservices",
        project_root / "k8s/data",
    ]:
        p.mkdir(parents=True, exist_ok=True)

    for v in ["postgres_data", "redis_data", "prometheus_data", "grafana_data"]:
        if shutil.which("docker"):
            vr = subprocess.run(["docker", "volume", "create", v], capture_output=True)
            if vr.returncode != 0:
                err = (vr.stderr or vr.stdout or b"").decode("utf-8", errors="replace").strip()
                print(
                    f"[ERROR] docker volume create {v} failed (exit {vr.returncode}): {err}",
                    file=sys.stderr,
                )
                return 1

    for cmd in ["docker", "tilt"]:
        if not shutil.which(cmd):
            print(
                f"[ERROR] {cmd} is not installed. Please install it first.",
                file=sys.stderr,
            )
            return 1

    print("Setup complete! 🎉")
    print("To start: just up  or  just up-k8s  or  tilt up")
    return 0
