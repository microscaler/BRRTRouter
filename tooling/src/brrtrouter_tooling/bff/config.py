"""Suite config loading for BFF generator.

Suite config YAML format (aligned with RERP bff-suite-config.yaml):
- openapi_base_dir: base directory for spec_path (relative to base_dir)
- output_path: path for merged BFF spec (relative to base_dir)
- services: map service name -> { base_path, spec_path, port? }
- metadata (optional): title, version, description, servers, etc. for merged spec
"""

from __future__ import annotations

from pathlib import Path
from typing import Any


def load_suite_config(config_path: Path, base_dir: Path | None = None) -> dict[str, Any]:
    """Load and normalize suite config from YAML.

    Paths in config (openapi_base_dir, output_path, services[].spec_path) are
    resolved relative to base_dir (default: config file parent).

    Returns:
        Config dict with resolved paths under "_resolved" for generator use.
    """
    import yaml

    with config_path.open() as f:
        data = yaml.safe_load(f) or {}

    # Paths in config (openapi_base_dir, output_path, spec_path) are relative to base_dir.
    # base_dir defaults to cwd so that running from project root works (e.g. RERP).
    base = (Path(base_dir) if base_dir else Path.cwd()).resolve()
    openapi_base = data.get("openapi_base_dir", "")
    output_path = data.get("output_path", "openapi_bff.yaml")
    openapi_base_dir = (base / openapi_base).resolve() if openapi_base else base

    resolved: dict[str, Any] = {
        "base_dir": base,
        "openapi_base_dir": openapi_base_dir,
        "output_path": (base / output_path).resolve()
        if not Path(output_path).is_absolute()
        else Path(output_path),
        "services": {},
    }

    for name, svc in (data.get("services") or {}).items():
        if not isinstance(svc, dict):
            continue
        spec_path = svc.get("spec_path") or f"{name}/openapi.yaml"
        full_spec = (openapi_base_dir / spec_path).resolve()
        resolved["services"][name] = {
            "base_path": svc.get("base_path", f"/api/{name}"),
            "spec_path": full_spec,
            "port": svc.get("port"),
        }

    data["_resolved"] = resolved
    return data
