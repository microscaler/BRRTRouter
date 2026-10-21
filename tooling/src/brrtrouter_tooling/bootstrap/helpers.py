"""Shared helpers for bootstrap (re-exports from brrtrouter_tooling.helpers + bootstrap-specific)."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from brrtrouter_tooling.bootstrap.config import resolve_bootstrap_layout
from brrtrouter_tooling.helpers import to_snake_case


def _get_registry_path(project_root: Path, layout: dict[str, Any] | None = None) -> Path | None:
    """Path to port registry (env BRRTROUTER_PORT_REGISTRY or layout port_registry)."""
    from brrtrouter_tooling.workspace.legacy_env import env

    cfg = resolve_bootstrap_layout(layout, project_root)
    reg = env("BRRTROUTER_PORT_REGISTRY")
    p = Path(reg).resolve() if reg else (project_root / cfg["port_registry"])
    return p if p.exists() else None


def _get_port_from_registry(
    project_root: Path, service_name: str, layout: dict[str, Any] | None = None
) -> int | None:
    """Read assigned port for service_name from port registry JSON."""
    path = _get_registry_path(project_root, layout)
    if not path:
        return None
    with path.open() as f:
        data = json.load(f)
    return data.get("assignments", {}).get(service_name)


def derive_binary_name(openapi_spec: dict[str, Any], service_name: str) -> str:
    """Derive crate binary name from spec title or service_name."""
    title = (openapi_spec.get("info") or {}).get("title", "")
    if title:
        binary_name = to_snake_case(title)
        if not binary_name.endswith("_api"):
            binary_name = (
                binary_name + "_api"
                if binary_name.endswith("_service")
                else binary_name + "_service_api"
            )
        return binary_name
    return f"{service_name.replace('-', '_')}_service_api"
