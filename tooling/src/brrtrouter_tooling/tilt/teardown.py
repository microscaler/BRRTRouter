"""Tilt teardown: tilt down, stop/rm containers, optional images/volumes/prune."""

from __future__ import annotations

import subprocess
from collections.abc import Callable
from pathlib import Path


def run(
    project_root: Path,
    service_names: list[str],
    *,
    container_name_fn: Callable[[str], str] | None = None,
    image_rmi_list_fn: Callable[[str], list[str]] | None = None,
    static_containers: list[str] | None = None,
    volume_names: list[str] | None = None,
    remove_images: bool = False,
    remove_volumes: bool = False,
    system_prune: bool = False,
) -> int:
    """Stop Tilt, containers, optionally remove images/volumes. Returns 0."""
    if static_containers is None:
        static_containers = []
    if volume_names is None:
        volume_names = []
    subprocess.run(["pkill", "-f", "tilt up"], capture_output=True)
    subprocess.run(["tilt", "down"], capture_output=True)
    for c in static_containers:
        subprocess.run(["docker", "stop", c], capture_output=True)
        subprocess.run(["docker", "rm", c], capture_output=True)
    if container_name_fn is not None:
        for s in service_names:
            name = container_name_fn(s)
            subprocess.run(["docker", "stop", name], capture_output=True)
            subprocess.run(["docker", "rm", name], capture_output=True)
    if remove_images and image_rmi_list_fn is not None:
        for s in service_names:
            for img in image_rmi_list_fn(s):
                subprocess.run(["docker", "rmi", img], capture_output=True)
    if remove_volumes:
        for v in volume_names:
            subprocess.run(["docker", "volume", "rm", v], capture_output=True)
    subprocess.run(["docker", "network", "prune", "-f"], capture_output=True)
    if system_prune:
        subprocess.run(["docker", "system", "prune", "-f"], capture_output=True)
    print("Teardown complete!")
    return 0
