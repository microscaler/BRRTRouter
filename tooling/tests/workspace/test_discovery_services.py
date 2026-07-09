from pathlib import Path

from brrtrouter_tooling.workspace.discovery.services import (
    get_binary_names,
    get_package_names,
    get_service_ports,
)
from brrtrouter_tooling.workspace.discovery.suites import (
    project_uses_flat_microservice_layout,
    project_uses_flat_openapi_layout,
    resolve_service_microservice_dir,
    resolve_service_openapi_spec_path,
    service_openapi_spec_path,
)


def _make_openapi_spec_flat(project_root: Path, service: str, port: int = 8001) -> Path:
    d = project_root / "openapi" / service
    d.mkdir(parents=True, exist_ok=True)
    spec = d / "openapi.yaml"
    spec.write_text(
        f"openapi: 3.1.0\ninfo: {{}}\nservers:\n  - url: http://localhost:{port}/api/v1/{service}\n"
    )
    return spec


def _make_openapi_spec_nested(
    project_root: Path, suite: str, service: str, port: int = 8001
) -> Path:
    d = project_root / "openapi" / suite / service
    d.mkdir(parents=True, exist_ok=True)
    spec = d / "openapi.yaml"
    spec.write_text(
        f"openapi: 3.1.0\ninfo: {{}}\nservers:\n  - url: http://localhost:{port}/api/v1/{service}\n"
    )
    return spec


def _make_openapi_spec(project_root: Path, suite: str, service: str, port: int = 8001) -> None:
    _make_openapi_spec_flat(project_root, service, port)


def test_resolve_service_openapi_spec_path_flat_layout(tmp_path: Path) -> None:
    spec = _make_openapi_spec_flat(tmp_path, "identity")
    got = resolve_service_openapi_spec_path(tmp_path, "hauliage", "identity")
    assert got == spec


def test_resolve_service_openapi_spec_path_nested_layout(tmp_path: Path) -> None:
    spec = _make_openapi_spec_nested(tmp_path, "trader", "orders")
    got = resolve_service_openapi_spec_path(tmp_path, "trader", "orders")
    assert got == spec


def test_resolve_service_microservice_dir_flat_layout(tmp_path: Path) -> None:
    flat = tmp_path / "microservices" / "identity" / "gen"
    flat.mkdir(parents=True)
    got = resolve_service_microservice_dir(tmp_path, "hauliage", "identity")
    assert got == tmp_path / "microservices" / "identity"


def test_resolve_service_microservice_dir_nested_layout(tmp_path: Path) -> None:
    nested = tmp_path / "microservices" / "trader" / "orders" / "gen"
    nested.mkdir(parents=True)
    got = resolve_service_microservice_dir(tmp_path, "trader", "orders")
    assert got == tmp_path / "microservices" / "trader" / "orders"


def test_project_uses_flat_openapi_layout(tmp_path: Path) -> None:
    assert not project_uses_flat_openapi_layout(tmp_path)
    _make_openapi_spec_flat(tmp_path, "identity")
    assert project_uses_flat_openapi_layout(tmp_path)


def test_service_openapi_spec_path_flat_layout(tmp_path: Path) -> None:
    spec = _make_openapi_spec_flat(tmp_path, "identity")
    got = service_openapi_spec_path(tmp_path, "hauliage", "identity")
    assert got == spec


def test_get_package_names_empty(tmp_path: Path) -> None:
    assert get_package_names(tmp_path) == {}


def test_get_package_names_derived(tmp_path: Path) -> None:
    _make_openapi_spec(tmp_path, "hauliage", "identity")
    _make_openapi_spec(tmp_path, "hauliage", "fleet")
    got = get_package_names(tmp_path)
    assert got == {
        "identity": "hauliage_identity",
        "fleet": "hauliage_fleet",
    }


def test_get_binary_names_empty(tmp_path: Path) -> None:
    assert get_binary_names(tmp_path) == {}


def test_get_binary_names_derived(tmp_path: Path) -> None:
    _make_openapi_spec(tmp_path, "hauliage", "identity")
    got = get_binary_names(tmp_path)
    assert got == {"identity": "identity"}


def test_get_package_names_includes_bff(tmp_path: Path) -> None:
    _make_openapi_spec(tmp_path, "hauliage", "identity")
    (tmp_path / "openapi").mkdir(parents=True, exist_ok=True)
    (tmp_path / "openapi" / "bff-suite-config.yaml").write_text(
        "suite: hauliage\nbff_service_name: bff\noutput_path: openapi/openapi_bff.yaml\n"
    )
    got = get_package_names(tmp_path)
    assert got["identity"] == "hauliage_identity"
    assert got["bff"] == "hauliage_bff"


def test_get_binary_names_includes_bff(tmp_path: Path) -> None:
    (tmp_path / "openapi").mkdir(parents=True, exist_ok=True)
    (tmp_path / "openapi" / "bff-suite-config.yaml").write_text(
        "suite: hauliage\nbff_service_name: bff\n"
    )
    got = get_binary_names(tmp_path)
    assert got["bff"] == "bff"


def test_get_service_ports_from_openapi(tmp_path: Path) -> None:
    _make_openapi_spec(tmp_path, "hauliage", "identity", port=8001)
    _make_openapi_spec(tmp_path, "hauliage", "fleet", port=8002)
    got = get_service_ports(tmp_path)
    assert got["identity"] == "8001"
    assert got["fleet"] == "8002"


def test_get_service_ports_empty(tmp_path: Path) -> None:
    assert get_service_ports(tmp_path) == {}
