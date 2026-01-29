"""Orchestrate BFF spec generation: config, merge, extensions, components, write."""

from __future__ import annotations

from pathlib import Path

from brrtrouter_tooling.bff.components import merge_components_and_security
from brrtrouter_tooling.bff.config import load_suite_config
from brrtrouter_tooling.bff.merge import merge_sub_service_specs


def generate_bff_spec(
    suite_config_path: Path,
    output_path: Path | None = None,
    base_dir: Path | None = None,
    validate: bool = False,
) -> Path:
    """Generate BFF OpenAPI spec from suite config.

    - Loads suite config (YAML) with openapi_base_dir, output_path, services.
    - Merges sub-service specs with path prefixing, schema prefixing, $ref updates.
    - Sets x-brrtrouter-downstream-path and x-service on each operation (Story 1.2).
    - Merges components.parameters, components.securitySchemes, root security (Story 1.3).
    - Writes merged spec to output_path (or config's output_path).

    Returns:
        Path to the written BFF spec.
    """
    import yaml

    config = load_suite_config(suite_config_path, base_dir=base_dir)
    resolved = config.get("_resolved") or {}
    services = resolved.get("services") or {}
    if not services:
        msg = f"No services in suite config: {suite_config_path}"
        raise ValueError(msg)

    metadata = config.get("metadata") or {}
    info = {
        "title": metadata.get("title", "BFF API"),
        "version": metadata.get("version", "1.0.0"),
        "description": metadata.get("description", "Backend for Frontend API (generated)."),
    }
    if metadata.get("contact"):
        info["contact"] = metadata["contact"]

    bff = merge_sub_service_specs(services, info=info)

    security_schemes = (config.get("metadata") or {}).get("security_schemes")
    security = (config.get("metadata") or {}).get("security")
    if security_schemes or security is not None:
        merge_components_and_security(bff, security_schemes=security_schemes, security=security)

    if "metadata" in config and "servers" in config["metadata"]:
        bff["servers"] = config["metadata"]["servers"]

    out = (
        output_path
        or resolved.get("output_path")
        or (suite_config_path.parent / "openapi_bff.yaml")
    )
    out = Path(out)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w") as f:
        yaml.dump(
            bff,
            f,
            sort_keys=False,
            default_flow_style=False,
            allow_unicode=True,
            width=120,
        )

    if validate:
        _validate_spec(out)

    return out


def _validate_spec(path: Path) -> None:
    """Basic validation: spec loads and has openapi, paths, info."""
    import yaml

    with path.open() as f:
        spec = yaml.safe_load(f)
    if not spec or spec.get("openapi") != "3.1.0":
        msg = f"Invalid or non-OpenAPI 3.1.0 spec: {path}"
        raise ValueError(msg)
    if "paths" not in spec or "info" not in spec:
        msg = f"Spec missing paths or info: {path}"
        raise ValueError(msg)
