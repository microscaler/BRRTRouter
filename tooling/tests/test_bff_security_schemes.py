"""The BFF aggregate defines every security scheme its operations require."""

from __future__ import annotations

from pathlib import Path

import pytest
import yaml

from brrtrouter_tooling.bff.components import (
    require_defined_security_schemes,
    undefined_security_schemes,
)
from brrtrouter_tooling.bff.merge import merge_sub_service_specs

BEARER = {"type": "http", "scheme": "bearer", "bearerFormat": "JWT"}


def _spec(tmp_path: Path, name: str, *, schemes: dict | None, secured: bool) -> Path:
    op: dict = {"operationId": f"{name}_get", "responses": {"200": {"description": "ok"}}}
    if secured:
        op["security"] = [{"BearerAuth": []}]
    spec: dict = {
        "openapi": "3.1.0",
        "info": {"title": name, "version": "1"},
        "paths": {f"/{name}": {"get": op}},
        "components": {"schemas": {}},
    }
    if schemes is not None:
        spec["components"]["securitySchemes"] = schemes
    path = tmp_path / name / "openapi.yaml"
    path.parent.mkdir(parents=True)
    path.write_text(yaml.safe_dump(spec))
    return path


def test_the_aggregate_carries_the_services_schemes(tmp_path: Path) -> None:
    services = {
        "orders": {
            "base_path": "/orders",
            "spec_path": _spec(tmp_path, "orders", schemes={"BearerAuth": BEARER}, secured=True),
        },
        "alerts": {
            "base_path": "/alerts",
            "spec_path": _spec(tmp_path, "alerts", schemes=None, secured=False),
        },
    }
    bff = merge_sub_service_specs(services)
    assert bff["components"]["securitySchemes"] == {"BearerAuth": BEARER}
    assert undefined_security_schemes(bff) == {}
    require_defined_security_schemes(bff, "test")


def test_a_required_but_undefined_scheme_is_refused(tmp_path: Path) -> None:
    services = {
        "orders": {
            "base_path": "/orders",
            "spec_path": _spec(tmp_path, "orders", schemes=None, secured=True),
        },
    }
    bff = merge_sub_service_specs(services)
    assert set(undefined_security_schemes(bff)) == {"BearerAuth"}
    with pytest.raises(ValueError, match="BearerAuth"):
        require_defined_security_schemes(bff, "test")


def test_root_security_counts_too() -> None:
    spec = {"security": [{"ApiKey": []}], "paths": {}, "components": {}}
    assert undefined_security_schemes(spec) == {"ApiKey": "(root security)"}
