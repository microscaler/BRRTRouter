"""Regression: `bff generate-system` keeps member specs' `components.responses` (and headers,
requestBodies, examples, securitySchemes) that their operations `$ref`.

Two generic member services each define an error response `NotFound` (plus `BadRequest` and
`RateLimited`) in `components.responses`, which `$ref` a header and their own `Error` schema. The
merged spec must define everything it references; the committed merged fixture is what the Rust
test `tests/bff_shared_responses_tests.rs` loads and generates a project from.
"""

import shutil
from pathlib import Path
from typing import Any

import pytest
import yaml

from brrtrouter_tooling.bff.generate_system import generate_system_bff_spec
from brrtrouter_tooling.bff.merge import merge_sub_service_specs

REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIR = REPO_ROOT / "tests" / "fixtures" / "bff_shared_responses"
SYSTEM = "catalog"


def _resolve_pointer(spec: dict[str, Any], ref: str) -> bool:
    node: Any = spec
    for part in ref[2:].split("/"):
        part = part.replace("~1", "/").replace("~0", "~")
        if not isinstance(node, dict) or part not in node:
            return False
        node = node[part]
    return True


def _unresolved_refs(spec: dict[str, Any]) -> list[str]:
    missing: list[str] = []
    stack: list[Any] = [spec]
    while stack:
        cur = stack.pop()
        if isinstance(cur, dict):
            ref = cur.get("$ref")
            if isinstance(ref, str) and ref.startswith("#/") and not _resolve_pointer(spec, ref):
                missing.append(ref)
            stack.extend(cur.values())
        elif isinstance(cur, list):
            stack.extend(cur)
    return sorted(set(missing))


def _openapi_dir(tmp_path: Path) -> Path:
    openapi_dir = tmp_path / "openapi"
    shutil.copytree(FIXTURE_DIR / SYSTEM, openapi_dir / SYSTEM)
    return openapi_dir


def _system_bff(tmp_path: Path) -> dict[str, Any]:
    out = tmp_path / "openapi_bff.yaml"
    generate_system_bff_spec(_openapi_dir(tmp_path), SYSTEM, output_path=out)
    return yaml.safe_load(out.read_text())


def _op(spec: dict[str, Any], path_suffix: str, method: str) -> dict[str, Any]:
    matches = [p for p in spec["paths"] if p.endswith(path_suffix)]
    assert len(matches) == 1, (path_suffix, list(spec["paths"]))
    return spec["paths"][matches[0]][method]


def test_system_bff_keeps_member_component_responses(tmp_path: Path) -> None:
    bff = _system_bff(tmp_path)
    assert _unresolved_refs(bff) == []
    comps = bff["components"]
    for svc in ("Widgets", "Gadgets"):
        assert {f"{svc}NotFound", f"{svc}BadRequest", f"{svc}RateLimited"} <= set(
            comps["responses"]
        )
        assert f"{svc}RequestId" in comps["headers"]
    assert "WidgetsWidgetInput" in comps["requestBodies"]
    assert "GadgetsSampleGadget" in comps["examples"]
    assert "bearerAuth" in comps["securitySchemes"]

    get_widget = _op(bff, "/widgets/{id}", "get")
    assert get_widget["responses"]["404"] == {"$ref": "#/components/responses/WidgetsNotFound"}
    not_found = comps["responses"]["WidgetsNotFound"]
    assert not_found["content"]["application/json"]["schema"] == {
        "$ref": "#/components/schemas/WidgetsError"
    }
    assert not_found["headers"]["X-Request-Id"] == {"$ref": "#/components/headers/WidgetsRequestId"}
    create = _op(bff, "/gadgets", "post")
    assert create["requestBody"] == {"$ref": "#/components/requestBodies/GadgetsGadgetInput"}
    body = comps["requestBodies"]["GadgetsGadgetInput"]["content"]["application/json"]
    assert body["schema"] == {"$ref": "#/components/schemas/GadgetsGadget"}
    assert body["examples"]["sample"] == {"$ref": "#/components/examples/GadgetsSampleGadget"}


def test_system_bff_rewrites_schema_refs_in_parameters(tmp_path: Path) -> None:
    bff = _system_bff(tmp_path)
    path = next(p for p in bff["paths"] if p.endswith("/widgets/{id}"))
    (param,) = bff["paths"][path]["parameters"]
    assert param["schema"] == {"$ref": "#/components/schemas/WidgetsWidgetId"}


def test_system_bff_matches_the_committed_fixture(tmp_path: Path) -> None:
    """The Rust generator test loads this file; regenerate it with `bff generate-system` when the
    merge changes (see the module docstring)."""
    committed = yaml.safe_load((FIXTURE_DIR / "openapi_bff.yaml").read_text())
    assert _system_bff(tmp_path) == committed


def test_workspace_generate_system_keeps_member_component_responses(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.bff.generate_system import (
        generate_system_bff_spec as workspace_generate_system,
    )

    out = tmp_path / "openapi_bff.yaml"
    workspace_generate_system(_openapi_dir(tmp_path), SYSTEM, output_path=out)
    bff = yaml.safe_load(out.read_text())
    assert _unresolved_refs(bff) == []
    comps = bff["components"]
    assert {"WidgetsNotFound", "GadgetsNotFound"} <= set(comps["responses"])
    assert "bearerAuth" in comps["securitySchemes"]
    assert bff["paths"]["/widgets/{id}"]["get"]["responses"]["404"] == {
        "$ref": "#/components/responses/WidgetsNotFound"
    }


def test_filtered_service_keeps_only_the_components_its_operations_reach(tmp_path: Path) -> None:
    openapi_dir = _openapi_dir(tmp_path)
    bff = merge_sub_service_specs(
        {
            "widgets": {
                "base_path": "/api/v1/widgets",
                "spec_path": openapi_dir / SYSTEM / "widgets" / "openapi.yaml",
                "include_operations": {"get_widgets"},
            }
        }
    )
    assert _unresolved_refs(bff) == []
    comps = bff["components"]
    assert set(comps["responses"]) == {"WidgetsNotFound"}
    assert set(comps["headers"]) == {"WidgetsRequestId"}
    assert "requestBodies" not in comps
    assert "WidgetsError" in comps["schemas"]


def test_conflicting_prefixed_component_is_an_error() -> None:
    from brrtrouter_tooling.bff.merge import merge_named_components, prefix_named_components

    def spec(description: str) -> dict[str, Any]:
        return {"components": {"responses": {"NotFound": {"description": description}}}}

    merged: dict[str, dict[str, Any]] = {}
    origin: dict[tuple[str, str], str] = {}
    first, second = spec("one"), spec("two")
    for loaded in (first, second):
        prefix_named_components(loaded, "svc")
    merge_named_components(merged, origin, "svc", first)
    merge_named_components(merged, origin, "svc", first)  # the same definition again is fine
    with pytest.raises(ValueError, match=r"components\.responses conflict for 'SvcNotFound'"):
        merge_named_components(merged, origin, "svc", second)
