"""Tail Tilt logs for a component."""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path


def _item_name(item: object) -> str | None:
    """Extract name from a Tilt uiresource item (metadata.name or name)."""
    if not isinstance(item, dict):
        return None
    return (item.get("metadata") or {}).get("name") or item.get("name")


def run(component: str, project_root: Path) -> int:
    """Run tilt logs <component> --follow. Returns exit code from tilt logs."""
    if not project_root.is_dir():
        print(f"[ERROR] Project root is not a directory: {project_root}", file=sys.stderr)
        return 1
    if not shutil.which("tilt"):
        print("[ERROR] Tilt is not installed.", file=sys.stderr)
        return 1
    r = subprocess.run(
        ["tilt", "get", "uiresources", "--format", "json"],
        capture_output=True,
        text=True,
        cwd=project_root,
    )
    if r.returncode != 0:
        print("[ERROR] Tilt is not running or not connected.", file=sys.stderr)
        return 1
    try:
        payload = json.loads(r.stdout or "{}")
    except json.JSONDecodeError:
        print("[ERROR] Tilt returned invalid JSON.", file=sys.stderr)
        return 1
    items = (
        payload.get("items", [])
        if isinstance(payload, dict)
        else (payload if isinstance(payload, list) else [])
    )
    if not any(_item_name(i) == component for i in items):
        print(
            f"[WARN] Component '{component}' not found. Run: tilt get uiresources",
            file=sys.stderr,
        )
        return 1
    return subprocess.run(
        ["tilt", "logs", component, "--follow"],
        cwd=project_root,
    ).returncode
