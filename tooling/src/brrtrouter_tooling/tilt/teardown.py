"""Tilt teardown: tilt down, stop/rm containers, optional images/volumes/prune. Replaces teardown-tilt.sh."""

from __future__ import annotations

import subprocess
from pathlib import Path

from brrtrouter_tooling.discovery.suites import tilt_service_names


def run(
    project_root: Path,
    remove_images: bool = False,
    remove_volumes: bool = False,
    system_prune: bool = False,
) -> int:
    """Stop Tilt, containers, optionally remove images/volumes, optionally docker system prune. Returns 0."""
    subprocess.run(["pkill", "-f", "tilt up"], capture_output=True)
    subprocess.run(["tilt", "down"], capture_output=True)
    for c in ["postgres-dev", "redis-dev", "prometheus-dev", "grafana-dev"]:
        subprocess.run(["docker", "stop", c], capture_output=True)
        subprocess.run(["docker", "rm", c], capture_output=True)

    for s in tilt_service_names(project_root):
        # We try to stop common patterns just in case
        for prefix in ["rerp", "pricewhisperer"]:
            subprocess.run(["docker", "stop", f"{prefix}-{s}-dev"], capture_output=True)
            subprocess.run(["docker", "rm", f"{prefix}-{s}-dev"], capture_output=True)

    if remove_images:
        for s in tilt_service_names(project_root):
            # Attempt generic cleanup
            subprocess.run(["docker", "rmi", f"localhost:5001/{s}:latest"], capture_output=True)
            subprocess.run(["docker", "rmi", f"localhost:5001/{s}:tilt"], capture_output=True)
            for prefix in ["rerp-accounting", "pricewhisperer-trader", "pricewhisperer"]:
                subprocess.run(["docker", "rmi", f"{prefix}-{s}:latest"], capture_output=True)
                subprocess.run(
                    ["docker", "rmi", f"localhost:5001/{prefix}-{s}:tilt"], capture_output=True
                )

    if remove_volumes:
        for v in ["postgres_data", "redis_data", "prometheus_data", "grafana_data"]:
            subprocess.run(["docker", "volume", "rm", v], capture_output=True)

    subprocess.run(["docker", "network", "prune", "-f"], capture_output=True)

    if system_prune:
        subprocess.run(["docker", "system", "prune", "-f"], capture_output=True)

    print("Teardown complete!")
    return 0
