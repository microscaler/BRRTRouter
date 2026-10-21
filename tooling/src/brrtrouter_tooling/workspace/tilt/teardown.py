"""Tilt teardown: tilt down, stop/rm containers, optional images/volumes/prune. Replaces teardown-tilt.sh."""

from __future__ import annotations

import subprocess
from pathlib import Path

from brrtrouter_tooling.workspace.discovery import tilt_service_names
from brrtrouter_tooling.workspace.discovery.suites import service_to_suite
from brrtrouter_tooling.workspace.legacy_env import env
from brrtrouter_tooling.workspace.profile import load_profile


def _tilt_web_port(project_root: Path) -> str:
    """Must match `just up` / `tilt up --port` (a Tiltfile cannot set the web UI port): env, else the profile."""
    return env("BRRTROUTER_TILT_PORT") or str(load_profile(project_root).tilt_port)


def run(
    project_root: Path,
    remove_images: bool = False,
    remove_volumes: bool = False,
    system_prune: bool = False,
) -> int:
    """Stop Tilt, containers, optionally remove images/volumes, optionally docker system prune. Returns 0."""
    subprocess.run(["pkill", "-f", "tilt up"], capture_output=True)
    profile = load_profile(project_root)
    subprocess.run(["tilt", "down", "--port", _tilt_web_port(project_root)], capture_output=True)
    for c in ["postgres-dev", "redis-dev", "prometheus-dev", "grafana-dev"]:
        subprocess.run(["docker", "stop", c], capture_output=True)
        subprocess.run(["docker", "rm", c], capture_output=True)
    for s in tilt_service_names(project_root):
        subprocess.run(["docker", "stop", f"{profile.name}-{s}-dev"], capture_output=True)
        subprocess.run(["docker", "rm", f"{profile.name}-{s}-dev"], capture_output=True)
    if remove_images:
        for s in tilt_service_names(project_root):
            image = profile.image_name(service_to_suite(project_root, s) or profile.flat_suite, s)
            subprocess.run(["docker", "rmi", f"{image}:latest"], capture_output=True)
            subprocess.run(["docker", "rmi", f"localhost:5001/{image}:tilt"], capture_output=True)
    if remove_volumes:
        for v in ["postgres_data", "redis_data", "prometheus_data", "grafana_data"]:
            subprocess.run(["docker", "volume", "rm", v], capture_output=True)
    subprocess.run(["docker", "network", "prune", "-f"], capture_output=True)
    if system_prune:
        subprocess.run(["docker", "system", "prune", "-f"], capture_output=True)
    print("Teardown complete!")
    return 0
