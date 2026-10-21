"""Default path layout for ports discovery. All paths relative to project_root."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from brrtrouter_tooling.helpers import resolve_layout_with_defaults

# Default layout; override via PortsLayout for other projects. Empty paths are filled from the project profile
# (layout.openapi_dir, layout.helm_values) before explicit overrides are applied.
DEFAULT_LAYOUT: dict[str, str] = {
    "openapi_dir": "",
    "helm_values_dir": "",
    "kind_config": "kind-config.yaml",
    "tiltfile": "Tiltfile",
    "port_registry": "port-registry.json",
    "bff_suite_config_name": "bff-suite-config.yaml",
    "openapi_bff_name": "openapi_bff.yaml",
}


def resolve_layout(
    layout: dict[str, Any] | None, project_root: Path | None = None
) -> dict[str, str]:
    """Return layout dict with defaults filled. Paths are relative to project_root.

    When ``project_root`` is omitted, :func:`pathlib.Path.cwd` is used so ``helm_values_dir``
    still resolves to a concrete absolute path (CodeRabbit: deterministic paths).
    """
    from brrtrouter_tooling.workspace.profile import load_profile

    root = project_root if project_root is not None else Path.cwd()
    profile = load_profile(root)
    defaults = dict(DEFAULT_LAYOUT)
    defaults["openapi_dir"] = profile.openapi_dir
    defaults["helm_values_dir"] = profile.helm_values
    if not (root / profile.helm_values).exists():
        # back-compat: a project whose chart is helm/<other>-microservice without a profile saying so
        helm_dir = root / "helm"
        if helm_dir.is_dir():
            for d in sorted(helm_dir.iterdir()):
                if d.is_dir() and d.name.endswith("-microservice") and (d / "values").is_dir():
                    defaults["helm_values_dir"] = f"helm/{d.name}/values"
                    break
    resolved = resolve_layout_with_defaults(layout, defaults)
    if "{system}" in resolved["helm_values_dir"]:
        resolved["helm_values_dir"] = resolved["helm_values_dir"].replace("{system}", profile.name)
    hd = resolved["helm_values_dir"]
    if "{" not in hd and not Path(hd).is_absolute():
        resolved["helm_values_dir"] = str((root / hd).resolve())
    return resolved
