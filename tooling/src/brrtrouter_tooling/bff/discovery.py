"""Directory-based discovery of sub-service OpenAPI specs (RERP-style layout).

openapi_dir/{system}/{service}/openapi.yaml -> service name, base_path /api/v1/{system}/{service}.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any


def discover_sub_services(openapi_dir: Path, system: str) -> dict[str, dict[str, Any]]:
    """Discover sub-services under openapi_dir/system/ with openapi.yaml.

    Returns:
        Dict mapping service name to {"spec_path": Path, "base_path": str}.
        base_path = /api/v1/{system}/{service}. Empty if system dir missing or no subs.
    """
    system_dir = openapi_dir / system
    if not system_dir.exists() or not system_dir.is_dir():
        return {}

    discovered: dict[str, dict[str, Any]] = {}
    for service_path in sorted(system_dir.iterdir()):
        if not service_path.is_dir():
            continue
        service_name = service_path.name
        if service_name.startswith(".") or service_name == system:
            continue
        spec_file = service_path / "openapi.yaml"
        if not spec_file.exists():
            continue
        discovered[service_name] = {
            "spec_path": spec_file,
            "base_path": f"/api/v1/{system}/{service_name}",
        }
    return discovered


def list_systems_with_sub_services(openapi_dir: Path) -> list[str]:
    """Return sorted system names that have at least one sub-service with openapi.yaml."""
    if not openapi_dir.exists() or not openapi_dir.is_dir():
        return []
    out: list[str] = []
    for d in sorted(openapi_dir.iterdir()):
        if not d.is_dir() or d.name.startswith("."):
            continue
        if discover_sub_services(openapi_dir, d.name):
            out.append(d.name)
    return out
