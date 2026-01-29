"""Merge components.parameters, components.securitySchemes, root security (Story 1.3)."""

from __future__ import annotations

from typing import Any


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
