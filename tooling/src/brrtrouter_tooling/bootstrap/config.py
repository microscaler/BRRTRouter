"""Bootstrap layout configuration (paths and crate naming)."""

from __future__ import annotations

from typing import Any

from brrtrouter_tooling.helpers import resolve_layout_with_defaults

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
    return resolve_layout_with_defaults(layout, DEFAULT_BOOTSTRAP_LAYOUT)
