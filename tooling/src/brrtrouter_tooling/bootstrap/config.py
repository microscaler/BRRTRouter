"""Bootstrap layout configuration (paths and crate naming)."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from brrtrouter_tooling.helpers import resolve_layout_with_defaults

# Empty entries come from the project profile: openapi_dir / workspace_dir (layout.*), suite (layout.default_suite) and
# crate_name_prefix (the same package_prefix discovery uses). Explicit layout keys override them.
DEFAULT_BOOTSTRAP_LAYOUT: dict[str, str] = {
    "openapi_dir": "",
    "workspace_dir": "",
    "docker_dir": "docker/microservices",
    "tiltfile": "Tiltfile",
    "port_registry": "port-registry.json",
    "suite": "",  # default: the profile's layout.default_suite
    "crate_name_prefix": "",  # default: workspace.discovery.services.package_prefix(root, suite)
}


def resolve_bootstrap_layout(
    layout: dict[str, Any] | None, project_root: Path | None = None
) -> dict[str, str]:
    """Return layout dict with defaults filled. Paths relative to project_root.

    Profile first (layout.openapi_dir, layout.workspace_dir, layout.default_suite), explicit ``layout`` keys win.
    ``crate_name_prefix`` defaults to the same prefix discovery uses for the suite
    (``workspace.discovery.services.package_prefix``), so a scaffolded crate and a discovered one share a name.
    """
    from brrtrouter_tooling.workspace.discovery.services import package_prefix
    from brrtrouter_tooling.workspace.profile import load_profile

    root = project_root or Path.cwd()
    profile = load_profile(root)
    defaults = dict(DEFAULT_BOOTSTRAP_LAYOUT)
    defaults["openapi_dir"] = profile.openapi_dir
    defaults["workspace_dir"] = profile.workspace_dir
    resolved = resolve_layout_with_defaults(layout, defaults)
    if not resolved.get("suite"):
        resolved["suite"] = profile.default_suite
    if not resolved.get("crate_name_prefix"):
        resolved["crate_name_prefix"] = package_prefix(Path(root), resolved["suite"])
    return resolved
