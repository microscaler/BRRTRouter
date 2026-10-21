"""Bootstrap layout configuration (paths and crate naming)."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from brrtrouter_tooling.helpers import resolve_layout_with_defaults

# Paths; the suite and crate prefix come from the project profile (layout.default_suite, naming.package_prefix).
DEFAULT_BOOTSTRAP_LAYOUT: dict[str, str] = {
    "openapi_dir": "openapi",
    "workspace_dir": "microservices",
    "docker_dir": "docker/microservices",
    "tiltfile": "Tiltfile",
    "port_registry": "port-registry.json",
    "suite": "",  # default: the profile's layout.default_suite
    "crate_name_prefix": "",  # default: <naming.package_prefix>_<suite>
}


def resolve_bootstrap_layout(
    layout: dict[str, Any] | None, project_root: Path | None = None
) -> dict[str, str]:
    """Return layout dict with defaults filled. Paths relative to project_root.

    ``suite`` defaults to the profile's layout.default_suite and ``crate_name_prefix`` to
    ``<naming.package_prefix>_<suite>``.
    """
    from brrtrouter_tooling.workspace.profile import load_profile

    profile = load_profile(project_root or Path.cwd())
    resolved = resolve_layout_with_defaults(layout, DEFAULT_BOOTSTRAP_LAYOUT)
    if not resolved.get("suite"):
        resolved["suite"] = profile.default_suite
    if not resolved.get("crate_name_prefix"):
        resolved["crate_name_prefix"] = f"{profile.package_prefix}_{resolved['suite']}"
    return resolved
