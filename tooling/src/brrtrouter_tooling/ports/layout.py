"""Default path layout for ports discovery (RERP-style). All paths relative to project_root."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from brrtrouter_tooling.helpers import resolve_layout_with_defaults

# Default layout; override via PortsLayout for other projects.
DEFAULT_LAYOUT: dict[str, str] = {
    "openapi_dir": "openapi",
    "helm_values_dir": "helm/{system}-microservice/values",
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
    resolved = resolve_layout_with_defaults(layout, DEFAULT_LAYOUT)
    root = project_root if project_root is not None else Path.cwd()
    if "{system}" in resolved["helm_values_dir"]:
        helm_dir = root / "helm"
        system_name = "rerp"
        if helm_dir.exists():
            for d in helm_dir.iterdir():
                if d.is_dir() and d.name.endswith("-microservice"):
                    system_name = d.name.replace("-microservice", "")
                    break
        resolved["helm_values_dir"] = resolved["helm_values_dir"].replace("{system}", system_name)
    hd = resolved["helm_values_dir"]
    if "{" not in hd and not Path(hd).is_absolute():
        resolved["helm_values_dir"] = str((root / hd).resolve())
    return resolved
