"""Tests for brrtrouter_tooling.bootstrap (config + microservice)."""

from pathlib import Path
from unittest.mock import patch


class TestResolveBootstrapLayout:
    def test_none_returns_defaults(self) -> None:
        from brrtrouter_tooling.bootstrap.config import resolve_bootstrap_layout

        got = resolve_bootstrap_layout(None)
        assert got["openapi_dir"] == "openapi"
        assert got["workspace_dir"] == "microservices"
        assert got["tiltfile"] == "Tiltfile"
        assert "suite" in got
        assert "crate_name_prefix" in got

    def test_override_partial(self) -> None:
        from brrtrouter_tooling.bootstrap.config import resolve_bootstrap_layout

        got = resolve_bootstrap_layout({"workspace_dir": "apps", "suite": "billing"})
        assert got["workspace_dir"] == "apps"
        assert got["suite"] == "billing"
        assert got["openapi_dir"] == "openapi"

    def test_unknown_key_ignored(self) -> None:
        from brrtrouter_tooling.bootstrap.config import resolve_bootstrap_layout

        got = resolve_bootstrap_layout({"unknown_key": "x", "workspace_dir": "y"})
        assert "unknown_key" not in got
        assert got["workspace_dir"] == "y"


class TestToSnakeCase:
    def test_dash_to_underscore(self) -> None:
        from brrtrouter_tooling.bootstrap import to_snake_case

        assert to_snake_case("my-service") == "my_service"

    def test_camel_to_snake(self) -> None:
        from brrtrouter_tooling.bootstrap import to_snake_case

        assert to_snake_case("MyService") == "my_service"

    def test_spaces_to_underscore(self) -> None:
        from brrtrouter_tooling.bootstrap import to_snake_case

        assert to_snake_case("my service") == "my_service"


class TestToPascalCase:
    def test_dash_separated(self) -> None:
        from brrtrouter_tooling.bootstrap import to_pascal_case

        assert to_pascal_case("my-service") == "MyService"


class TestDeriveBinaryName:
    def test_from_title_snake_with_api(self) -> None:
        from brrtrouter_tooling.bootstrap import derive_binary_name

        spec = {"info": {"title": "Accounting Service"}}
        assert derive_binary_name(spec, "accounting") == "accounting_service_api"

    def test_from_title_ends_with_service(self) -> None:
        from brrtrouter_tooling.bootstrap import derive_binary_name

        spec = {"info": {"title": "Foo"}}
        assert derive_binary_name(spec, "foo") == "foo_service_api"

    def test_fallback_from_service_name(self) -> None:
        from brrtrouter_tooling.bootstrap import derive_binary_name

        spec = {}
        assert derive_binary_name(spec, "my-svc") == "my_svc_service_api"


class TestLoadOpenApiSpec:
    def test_load_yaml(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bootstrap import load_openapi_spec

        spec_file = tmp_path / "openapi.yaml"
        spec_file.write_text("openapi: '3.0'\ninfo:\n  title: Test API\n")
        got = load_openapi_spec(spec_file)
        assert got["info"]["title"] == "Test API"


class TestCreateConfigYaml:
    def test_writes_file(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bootstrap.microservice import create_config_yaml

        out = tmp_path / "config" / "config.yaml"
        create_config_yaml(out)
        assert out.exists()
        assert "security:" in out.read_text()
        assert "api_keys:" in out.read_text()


class TestCreateDockerfile:
    def test_writes_dockerfile_with_port_and_paths(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bootstrap.microservice import create_dockerfile

        out = tmp_path / "Dockerfile.svc"
        create_dockerfile(
            service_name="my-svc",
            binary_name="my_svc_service_api",
            port=8080,
            output_path=out,
            workspace_dir="microservices",
            suite="accounting",
        )
        text = out.read_text()
        assert "8080" in text
        assert "my_svc_service_api" in text
        assert "microservices/accounting/my-svc" in text


class TestCreateDependenciesConfigToml:
    def test_writes_file(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bootstrap.microservice import create_dependencies_config_toml

        out = tmp_path / "brrtrouter-dependencies.toml"
        create_dependencies_config_toml(out)
        assert out.exists()
        assert "[dependencies]" in out.read_text()


class TestUpdateWorkspaceCargoToml:
    def test_adds_members(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bootstrap.microservice import update_workspace_cargo_toml

        cargo = tmp_path / "Cargo.toml"
        cargo.write_text('[workspace]\nmembers = [\n    "a",\n]\n')
        update_workspace_cargo_toml("new-svc", cargo, "accounting")
        text = cargo.read_text()
        assert "accounting/new-svc/gen" in text
        assert "accounting/new-svc/impl" in text

    def test_idempotent_if_already_present(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bootstrap.microservice import update_workspace_cargo_toml

        cargo = tmp_path / "Cargo.toml"
        cargo.write_text(
            '[workspace]\nmembers = [\n    "accounting/new-svc/gen",\n    "accounting/new-svc/impl",\n]\n'
        )
        update_workspace_cargo_toml("new-svc", cargo, "accounting")
        text = cargo.read_text()
        assert text.count("accounting/new-svc/gen") == 1
        assert text.count("accounting/new-svc/impl") == 1


class TestRunBootstrapMicroservice:
    def test_missing_spec_returns_1(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bootstrap import run_bootstrap_microservice

        # No openapi dir/spec
        code = run_bootstrap_microservice(
            service_name="nospec",
            port=9000,
            project_root=tmp_path,
            layout={
                "openapi_dir": "openapi",
                "suite": "accounting",
                "workspace_dir": "ms",
                "docker_dir": "docker/ms",
                "tiltfile": "Tiltfile",
                "port_registry": "ports.json",
                "crate_name_prefix": "test_",
            },
        )
        assert code == 1

    def test_missing_port_and_no_registry_returns_1(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bootstrap import run_bootstrap_microservice

        (tmp_path / "openapi" / "accounting" / "mysvc").mkdir(parents=True)
        (tmp_path / "openapi" / "accounting" / "mysvc" / "openapi.yaml").write_text(
            "openapi: '3.0'\ninfo:\n  title: My Svc\n"
        )
        code = run_bootstrap_microservice(
            service_name="mysvc",
            port=None,
            project_root=tmp_path,
            layout={
                "openapi_dir": "openapi",
                "suite": "accounting",
                "workspace_dir": "ms",
                "docker_dir": "docker/ms",
                "tiltfile": "Tiltfile",
                "port_registry": "nonexistent.json",
                "crate_name_prefix": "test_",
            },
        )
        assert code == 1

    def test_success_with_mocked_gen(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bootstrap import run_bootstrap_microservice

        (tmp_path / "openapi" / "accounting" / "mysvc").mkdir(parents=True)
        (tmp_path / "openapi" / "accounting" / "mysvc" / "openapi.yaml").write_text(
            "openapi: '3.0'\ninfo:\n  title: My Svc\npaths: {}\n"
        )
        (tmp_path / "ms").mkdir()
        (tmp_path / "ms" / "Cargo.toml").write_text("[workspace]\nmembers = []\n")
        (tmp_path / "Tiltfile").write_text(
            "BINARY_NAMES = {}\ndeps=[]\nresource_deps=[] labels=['microservices-build']\nports = {}\ncreate_microservice_lint('x', 'y')\ncreate_microservice_gen('x', 'y', 'z')\ncreate_microservice_deployment('x')\n"
        )
        layout = {
            "openapi_dir": "openapi",
            "suite": "accounting",
            "workspace_dir": "ms",
            "docker_dir": "docker/ms",
            "tiltfile": "Tiltfile",
            "port_registry": "ports.json",
            "crate_name_prefix": "test_",
        }

        with (
            patch("brrtrouter_tooling.bootstrap.microservice.generate_code_with_brrtrouter"),
            patch("brrtrouter_tooling.bootstrap.microservice._generate_impl_with_brrtrouter"),
            patch("brrtrouter_tooling.ci.run_fix_cargo_paths"),
        ):
            code = run_bootstrap_microservice(
                service_name="mysvc",
                port=9000,
                project_root=tmp_path,
                layout=layout,
            )
        assert code == 0
        assert (tmp_path / "docker" / "ms" / "Dockerfile.mysvc").exists()
        assert (
            tmp_path / "ms" / "accounting" / "mysvc" / "impl" / "config" / "config.yaml"
        ).exists()
        assert "accounting/mysvc/gen" in (tmp_path / "ms" / "Cargo.toml").read_text()


class TestGetPortFromRegistry:
    def test_returns_port_when_registry_has_assignment(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bootstrap.helpers import _get_port_from_registry

        (tmp_path / "port-registry.json").write_text('{"assignments": {"svc1": 9100}}')
        got = _get_port_from_registry(tmp_path, "svc1", {"port_registry": "port-registry.json"})
        assert got == 9100

    def test_returns_none_when_registry_missing(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bootstrap.helpers import _get_port_from_registry

        got = _get_port_from_registry(tmp_path, "svc1", {"port_registry": "nonexistent.json"})
        assert got is None


class TestFixImplMainNaming:
    """Test that _fix_impl_main_naming only replaces the gen-crate use, not std/brrtrouter/etc."""

    def test_replaces_only_gen_crate_use(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bootstrap.microservice import _fix_impl_main_naming

        main_rs = tmp_path / "src" / "main.rs"
        main_rs.parent.mkdir(parents=True, exist_ok=True)
        content = """use pet_store_gen::registry;
use brrtrouter::dispatcher::Dispatcher;
use std::io;
use std::path::PathBuf;
use clap::Parser;
"""
        main_rs.write_text(content)
        _fix_impl_main_naming(main_rs, "my-svc", "prefix")
        result = main_rs.read_text()
        assert "use prefix_my_svc_gen::registry;" in result
        assert "use brrtrouter::dispatcher::Dispatcher;" in result
        assert "use std::io;" in result
        assert "use std::path::PathBuf;" in result
        assert "use clap::Parser;" in result
        assert "use pet_store_gen::" not in result


class TestUpdateGenCargoToml:
    """Test _update_gen_cargo_toml injects rust_decimal when no anchor deps exist."""

    def test_injects_rust_decimal_under_dependencies_when_no_anchor(self, tmp_path: Path) -> None:
        from brrtrouter_tooling.bootstrap.microservice import _update_gen_cargo_toml

        gen_dir = tmp_path / "gen"
        gen_dir.mkdir()
        (gen_dir / "src").mkdir()
        cargo_toml = gen_dir / "Cargo.toml"
        cargo_toml.write_text(
            '[package]\nname = "old"\nversion = "0.1.0"\n\n[dependencies]\nbrrtrouter = { workspace = true }\n'
        )
        (gen_dir / "src" / "lib.rs").write_text(
            "use rust_decimal::Decimal;\nfn f() -> Decimal { Decimal::ZERO }\n"
        )
        _update_gen_cargo_toml(cargo_toml, "my-svc", "prefix")
        content = cargo_toml.read_text()
        assert "rust_decimal = { workspace = true }" in content
        assert "rust_decimal" in content
