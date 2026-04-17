"""Docker disk hygiene helpers for local dev / Tilt / Kind workflows.

Repeated ``docker build`` (especially with ``--no-cache``) leaves **dangling** ``<none>`` images.
BuildKit/buildx also grows the build cache. These helpers mirror the strategy in
``tests/curl_harness.rs`` (signal cleanup + image prune) in a form tooling can call on demand.

Safe defaults: ``prune_dangling_images`` and ``prune_stopped_containers`` only remove unused data.
``prune_buildx_cache`` is more aggressive — use periodically, not after every build.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def prune_dangling_images() -> int:
    """Run ``docker image prune -f``. Removes dangling images only (not tagged in-use images)."""
    r = subprocess.run(
        ["docker", "image", "prune", "-f"],
        cwd=str(Path.cwd()),
    )
    return 0 if r.returncode == 0 else 1


def prune_stopped_containers() -> int:
    """Run ``docker container prune -f``. Removes stopped containers."""
    r = subprocess.run(
        ["docker", "container", "prune", "-f"],
        cwd=str(Path.cwd()),
    )
    return 0 if r.returncode == 0 else 1


def prune_buildx_cache() -> int:
    """Run ``docker buildx prune -f`` to trim BuildKit cache (can free many GB)."""
    r = subprocess.run(
        ["docker", "buildx", "prune", "-f"],
        cwd=str(Path.cwd()),
    )
    return 0 if r.returncode == 0 else 1


def prune_dev_sweep() -> int:
    """Ordered dev cleanup: dangling images, stopped containers, then buildx cache. Returns 0 if all ok."""
    steps = [
        ("docker image prune -f", prune_dangling_images),
        ("docker container prune -f", prune_stopped_containers),
        ("docker buildx prune -f", prune_buildx_cache),
    ]
    failed = []
    for label, fn in steps:
        if fn() != 0:
            failed.append(label)
    if failed:
        print(f"⚠️  Some prune steps failed: {', '.join(failed)}", file=sys.stderr)
        return 1
    print("✅ Dev prune sweep complete (dangling images, stopped containers, buildx cache).")
    return 0


def env_prune_after_build() -> bool:
    """True if ``BRRTR_DOCKER_PRUNE_DANGLING_AFTER_BUILD`` is 1/true/yes."""
    v = os.environ.get("BRRTR_DOCKER_PRUNE_DANGLING_AFTER_BUILD", "").strip().lower()
    return v in ("1", "true", "yes")
