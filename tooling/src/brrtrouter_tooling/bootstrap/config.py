"""Bootstrap layout configuration (paths and crate naming)."""

from __future__ import annotations

from typing import Any

# RERP-style default; override for other consumers.
DEFAULT_BOOTSTRAP_LAYOUT: dict[str, str] = {
    "openapi_dir": "openapi",
    "suite": "accounting",
    "workspace_dir": "microservices",
    "docker_dir": "docker/microservices",
    "tiltfile": "Tiltfile",
    "port_registry": "port-registry.json",
    "crate_name_prefix": "rerp_accounting",
}


def resolve_bootstrap_layout(layout: dict[str, Any] | None) -> dict[str, str]:
    """Return layout dict with defaults filled. Paths relative to project_root."""
    if layout is None:
        return dict(DEFAULT_BOOTSTRAP_LAYOUT)
    out = dict(DEFAULT_BOOTSTRAP_LAYOUT)
    out.update({k: str(v) for k, v in layout.items() if k in out})
    return out
