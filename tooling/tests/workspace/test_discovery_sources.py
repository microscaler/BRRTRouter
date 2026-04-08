from pathlib import Path


def test_discover_helm_missing_dir_returns_empty(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.discovery.sources import discover_helm

    assert discover_helm(tmp_path) == {}


def test_discover_helm_finds_ports_with_names(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.discovery.sources import discover_helm

    d = tmp_path / "helm" / "hauliage-microservice" / "values"
    d.mkdir(parents=True)
    (d / "identity.yaml").write_text("service:\n  port: 8001\n")
    (d / "fleet.yaml").write_text("service:\n  name: custom\n  port: 8002\n")
    got = discover_helm(tmp_path)
    assert got["identity"] == 8001
    assert got["custom"] == 8002


def test_discover_kind_host_ports_missing_returns_empty(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.discovery.sources import discover_kind_host_ports

    assert discover_kind_host_ports(tmp_path) == []


def test_discover_kind_host_ports_finds_mappings(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.discovery.sources import discover_kind_host_ports

    (tmp_path / "kind-config.yaml").write_text(
        "nodes:\n  - extraPortMappings:\n      - hostPort: 8001\n        containerPort: 80\n"
    )
    assert discover_kind_host_ports(tmp_path) == [(8001, "80")]


def test_discover_tiltfile_missing_returns_empty(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.discovery.sources import discover_tiltfile

    assert discover_tiltfile(tmp_path) == {}


def test_discover_tiltfile_finds_ports_dict(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.discovery.sources import discover_tiltfile

    (tmp_path / "Tiltfile").write_text("ports = {\n  'identity': '8001',\n  'fleet': '8002',\n}")
    got = discover_tiltfile(tmp_path)
    assert got["identity"] == 8001
    assert got["fleet"] == 8002


def test_discover_bff_suite_config_one_suite(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.discovery.sources import discover_bff_suite_config

    (tmp_path / "openapi").mkdir(exist_ok=True, parents=True)
    (tmp_path / "openapi" / "bff-suite-config.yaml").write_text(
        "services:\n  bff: {port: 8010}\n  identity: {port: 8001}\n"
    )
    got = discover_bff_suite_config(tmp_path)
    assert got["bff"] == 8010
    assert got["identity"] == 8001


def test_discover_openapi_bff_localhost(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.discovery.sources import discover_openapi_bff_localhost

    (tmp_path / "openapi").mkdir(exist_ok=True, parents=True)
    (tmp_path / "openapi" / "bff-suite-config.yaml").write_text("bff_service_name: bff\n")
    (tmp_path / "openapi" / "openapi_bff.yaml").write_text(
        "servers:\n  - url: http://localhost:8010/api/v1\n"
    )
    got = discover_openapi_bff_localhost(tmp_path)
    assert got["bff"] == (8010, "hauliage")


def test_discover_openapi_suite_microservice_localhost(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.discovery.sources import (
        discover_openapi_suite_microservice_localhost,
    )

    d = tmp_path / "openapi" / "identity"
    d.mkdir(parents=True)
    (d / "openapi.yaml").write_text("servers:\n  - url: http://localhost:8001/api/v1/identity\n")
    got = discover_openapi_suite_microservice_localhost(tmp_path)
    assert got["identity"] == ("hauliage", 8001)
