"""Generate system-level BFF OpenAPI from directory discovery (RERP-style).

openapi_dir/{system}/{service}/openapi.yaml -> openapi_dir/{system}/openapi.yaml.
"""

from __future__ import annotations

from pathlib import Path

import yaml

from brrtrouter_tooling.bff.discovery import discover_sub_services
from brrtrouter_tooling.bff.merge import merge_sub_service_specs
from brrtrouter_tooling.helpers import extract_readme_overview, to_pascal_case

# Default parameters injected into BFF (RERP compatibility).
_DEFAULT_PARAMETERS = {
    "Page": {
        "name": "page",
        "in": "query",
        "schema": {"type": "integer", "minimum": 1, "default": 1},
    },
    "Limit": {
        "name": "limit",
        "in": "query",
        "schema": {
            "type": "integer",
            "minimum": 1,
            "maximum": 100,
            "default": 20,
        },
    },
    "Search": {
        "name": "search",
        "in": "query",
        "schema": {"type": "string"},
    },
}


def generate_system_bff_spec(
    openapi_dir: Path,
    system: str,
    output_path: Path | None = None,
) -> None:
    """Generate system BFF OpenAPI at output_path (default: openapi_dir/system/openapi.yaml).

    Discovers sub-services from openapi_dir/system/{service}/openapi.yaml,
    merges paths and schemas (prefixing, $refs, x-service, x-brrtrouter-downstream-path),
    writes deterministic YAML. Idempotent; clobbers output.
    If no sub-services, does not write.
    """
    sub_services = discover_sub_services(openapi_dir, system)
    if not sub_services:
        return

    out = output_path if output_path is not None else (openapi_dir / system / "openapi.yaml")
    system_title = system.replace("-", " ").title()
    default_desc = f"System-level API gateway for all {system_title} services"
    system_description = extract_readme_overview(openapi_dir / system / "README.md", default_desc)

    service_routes = [
        f"- `{cfg['base_path']}/*` → {to_pascal_case(sname)} Service"
        for sname, cfg in sorted(sub_services.items())
    ]

    services_for_merge = {
        name: {"base_path": cfg["base_path"], "spec_path": cfg["spec_path"]}
        for name, cfg in sub_services.items()
    }

    info = {
        "title": f"{system_title} API Gateway",
        "description": (
            f"{system_description}. "
            f"This aggregates and proxies requests to {system_title} microservices. "
            f"Single entry point for the {system_title} system.\n\n"
            "All requests proxied to sub-services:\n" + "\n".join(service_routes) + "\n\n"
            "**Note**: Auto-generated. Do not edit manually.\n\n"
            f"**Discovery**: openapi/{system}/"
        ),
        "version": "1.0.0",
    }

    bff = merge_sub_service_specs(services_for_merge, info=info)

    bff["servers"] = [{"url": f"/api/v1/{system}", "description": f"{system_title} API Gateway"}]

    for pname, pdef in _DEFAULT_PARAMETERS.items():
        if pname not in bff["components"]["parameters"]:
            bff["components"]["parameters"][pname] = pdef

    out.parent.mkdir(parents=True, exist_ok=True)
    if out.exists():
        out.unlink()
    with out.open("w") as f:
        yaml.dump(
            bff,
            f,
            sort_keys=False,
            default_flow_style=False,
            allow_unicode=True,
            width=120,
        )
