"""Merge components.parameters, components.securitySchemes, root security (Story 1.3)."""

from __future__ import annotations

import logging
from typing import Any

logger = logging.getLogger(__name__)


def merge_components_and_security(
    bff_spec: dict[str, Any],
    security_schemes: dict[str, Any] | None = None,
    security: list[Any] | None = None,
) -> None:
    """Merge security schemes and set root security on the BFF spec.

    - components.securitySchemes: merged from security_schemes (e.g. bearer JWT).
    - security: root-level security (e.g. [{"bearerAuth": []}]).
    Mutates bff_spec in place.
    """
    if "components" not in bff_spec:
        bff_spec["components"] = {}
    comp = bff_spec["components"]
    if "securitySchemes" not in comp:
        comp["securitySchemes"] = {}
    if security_schemes:
        for name, scheme in security_schemes.items():
            if name not in comp["securitySchemes"]:
                comp["securitySchemes"][name] = scheme
    if security is not None:
        bff_spec["security"] = security


def collect_security_schemes(sub_services: dict[str, dict[str, Any]]) -> dict[str, Any]:
    """components.securitySchemes of every sub-service spec, merged by name.

    Each operation copied into the BFF keeps its ``security``; without the schemes it names, BRRTRouter cannot
    evaluate the requirement and answers 401 to every request. The first service (by name) to define a scheme
    wins; a different definition under the same name elsewhere is logged.
    """
    from pathlib import Path

    from brrtrouter_tooling.helpers import load_yaml_spec

    merged: dict[str, Any] = {}
    origin: dict[str, str] = {}
    for sname, cfg in sorted(sub_services.items()):
        spec_path = cfg.get("spec_path")
        if spec_path is None:
            continue
        spec_path = Path(spec_path)
        if not spec_path.exists():
            continue
        schemes = ((load_yaml_spec(spec_path) or {}).get("components") or {}).get(
            "securitySchemes"
        ) or {}
        for name, scheme in schemes.items():
            if name not in merged:
                merged[name] = scheme
                origin[name] = sname
            elif merged[name] != scheme:
                logger.warning(
                    "Security scheme %r differs between services %r and %r; keeping %r's",
                    name,
                    origin[name],
                    sname,
                    origin[name],
                )
    return merged


def undefined_security_schemes(spec: dict[str, Any]) -> dict[str, str]:
    """Security scheme names the spec's operations (or root ``security``) require but it does not define,
    each with one ``METHOD /path`` that requires it."""
    defined = set(((spec.get("components") or {}).get("securitySchemes") or {}).keys())
    undefined: dict[str, str] = {}
    for requirement in spec.get("security") or []:
        for name in requirement or {}:
            if name not in defined:
                undefined.setdefault(name, "(root security)")
    for path, path_def in (spec.get("paths") or {}).items():
        if not isinstance(path_def, dict):
            continue
        for method, op in path_def.items():
            if not isinstance(op, dict):
                continue
            for requirement in op.get("security") or []:
                for name in requirement or {}:
                    if name not in defined:
                        undefined.setdefault(name, f"{method.upper()} {path}")
    return undefined


def require_defined_security_schemes(spec: dict[str, Any], label: str) -> None:
    """Raise ValueError when the spec requires security schemes it does not define."""
    undefined = undefined_security_schemes(spec)
    if undefined:
        listed = ", ".join(f"{name} (e.g. {where})" for name, where in sorted(undefined.items()))
        msg = (
            f"{label}: operations require security schemes the spec does not define: {listed}. "
            "Define them in a service spec's components.securitySchemes or the suite config's "
            "metadata.security_schemes; BRRTRouter answers 401 to every request on such a route."
        )
        raise ValueError(msg)
