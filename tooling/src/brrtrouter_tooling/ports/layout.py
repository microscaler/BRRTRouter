"""Default path layout for ports discovery (RERP-style). All paths relative to project_root."""

from __future__ import annotations

from typing import Any

from brrtrouter_tooling.helpers import resolve_layout_with_defaults

# RERP-style default layout; override via PortsLayout for other projects.
DEFAULT_LAYOUT: dict[str, str] = {
    "openapi_dir": "openapi",
    "helm_values_dir": "helm/rerp-microservice/values",
    "kind_config": "kind-config.yaml",
    "tiltfile": "Tiltfile",
    "port_registry": "port-registry.json",
    "bff_suite_config_name": "bff-suite-config.yaml",
    "openapi_bff_name": "openapi_bff.yaml",
}


def resolve_layout(layout: dict[str, Any] | None) -> dict[str, str]:
    """Return layout dict with defaults filled. Paths are relative to project_root."""
    return resolve_layout_with_defaults(layout, DEFAULT_LAYOUT)
