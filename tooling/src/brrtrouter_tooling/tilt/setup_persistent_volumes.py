"""Setup PersistentVolumes for RERP (k8s/data, k8s/monitoring). Replaces setup-persistent-volumes.sh."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path


def _skip_persistent_volume_setup() -> bool:
    v = os.environ.get("BRRTROUTER_SKIP_SETUP_PERSISTENT_VOLUMES", "").strip().lower()
    return v in ("1", "true", "yes", "on")


def _first_existing(*candidates: Path) -> Path | None:
    for p in candidates:
        if p.exists():
            return p
    return None


def run(project_root: Path) -> int:
    """Apply data + monitoring PersistentVolume manifests. Returns 0 or 1.

    Looks for k8s paths under the project first; PriceWhisperer moved PV YAML to
    ``shared-kind-cluster/k8s/platform-data/`` (sibling of the app repo).
    """
    if _skip_persistent_volume_setup():
        print("⏭️  Skipping setup-persistent-volumes (BRRTROUTER_SKIP_SETUP_PERSISTENT_VOLUMES=1)")
        return 0
    if not shutil.which("kubectl"):
        print("❌ Error: kubectl is not installed or not on PATH", file=sys.stderr)
        return 1
    if subprocess.run(["kubectl", "cluster-info"], capture_output=True).returncode != 0:
        print("❌ Error: Cannot connect to Kubernetes cluster", file=sys.stderr)
        print(
            "   Please ensure your Kind cluster is running: kind get clusters",
            file=sys.stderr,
        )
        return 1
    sibling_shared_kind = project_root.parent / "shared-kind-cluster" / "k8s" / "platform-data"
    for label, candidates in (
        (
            "data",
            (
                project_root / "k8s" / "data" / "persistent-volumes.yaml",
                sibling_shared_kind / "data" / "persistent-volumes.yaml",
            ),
        ),
        (
            "monitoring",
            (
                project_root / "k8s" / "monitoring" / "persistent-volumes.yaml",
                sibling_shared_kind / "monitoring" / "persistent-volumes.yaml",
            ),
        ),
    ):
        path = _first_existing(*candidates)
        if path is not None:
            print(f"📦 Creating {label} PersistentVolumes...")
            r = subprocess.run(
                ["kubectl", "apply", "-f", str(path)], capture_output=True, text=True
            )
            if r.returncode != 0 and "AlreadyExists" not in (r.stderr or ""):
                print(f"⚠️  Warning: Some {label} PVs may already exist (this is OK)")
        else:
            print(f"Info:  No {label} PersistentVolumes file found (this is OK for initial setup)")
    print("✅ PersistentVolumes setup complete!")
    r = subprocess.run(["kubectl", "get", "pv"], capture_output=True, text=True)
    if r.returncode == 0:
        print(r.stdout or "No PersistentVolumes found")
    return 0
