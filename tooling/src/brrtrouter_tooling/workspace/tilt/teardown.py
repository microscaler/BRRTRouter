"""Tilt teardown: tilt down, stop/rm containers, optional images/volumes/prune. Replaces teardown-tilt.sh."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

from brrtrouter_tooling.workspace.discovery import tilt_service_names

# Must match `just up` / `tilt up --port` (Tiltfile cannot set the web UI port).
_DEFAULT_TILT_WEB_PORT = os.environ.get("HAULIAGE_TILT_PORT", "10352")


def run(
    project_root: Path,
    remove_images: bool = False,
    remove_volumes: bool = False,
    system_prune: bool = False,
) -> int:
    """Stop Tilt, containers, optionally remove images/volumes, optionally docker system prune. Returns 0."""
    subprocess.run(["pkill", "-f", "tilt up"], capture_output=True)
    subprocess.run(["tilt", "down", "--port", _DEFAULT_TILT_WEB_PORT], capture_output=True)
    for c in ["postgres-dev", "redis-dev", "prometheus-dev", "grafana-dev"]:
        subprocess.run(["docker", "stop", c], capture_output=True)
        subprocess.run(["docker", "rm", c], capture_output=True)
    for s in tilt_service_names(project_root):
        subprocess.run(["docker", "stop", f"hauliage-{s}-dev"], capture_output=True)
        subprocess.run(["docker", "rm", f"hauliage-{s}-dev"], capture_output=True)
    if remove_images:
        for s in tilt_service_names(project_root):
            subprocess.run(["docker", "rmi", f"hauliage-hauliage-{s}:latest"], capture_output=True)
            subprocess.run(
                ["docker", "rmi", f"localhost:5001/hauliage-hauliage-{s}:tilt"], capture_output=True
            )
    if remove_volumes:
        for v in ["postgres_data", "redis_data", "prometheus_data", "grafana_data"]:
            subprocess.run(["docker", "volume", "rm", v], capture_output=True)
    subprocess.run(["docker", "network", "prune", "-f"], capture_output=True)
    if system_prune:
        subprocess.run(["docker", "system", "prune", "-f"], capture_output=True)
    print("Teardown complete!")
    return 0
