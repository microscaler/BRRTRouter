"""Tests for brrtrouter-gen impl crate package name resolution."""

from pathlib import Path
from unittest.mock import patch

from brrtrouter_tooling.helpers import (
    brrtrouter_impl_package_name,
    resolve_cargo_impl_package_name,
)


class TestBrrtrouterImplPackageName:
    def test_simple_module(self) -> None:
        assert brrtrouter_impl_package_name("amd") == "amd_service_api_impl"

    def test_kebab_module(self) -> None:
        """Kebab-case service dirs (e.g. openapi/trader/market-data) map to snake impl package."""
        assert brrtrouter_impl_package_name("market-data") == "market_data_service_api_impl"


class TestResolveCargoImplPackageName:
    def test_none_uses_module(self) -> None:
        assert resolve_cargo_impl_package_name(None, "amd") == "amd_service_api_impl"

    def test_none_kebab_module(self) -> None:
        assert (
            resolve_cargo_impl_package_name(None, "market-data") == "market_data_service_api_impl"
        )

    def test_shorthand_impl_suffix(self) -> None:
        assert resolve_cargo_impl_package_name("amd_impl", "amd") == "amd_service_api_impl"

    def test_bff_camel_case_impl_passthrough(self) -> None:
        assert resolve_cargo_impl_package_name("traderBFF_impl", "traderBFF") == "traderBFF_impl"

    def test_none_package_bff_module(self) -> None:
        assert resolve_cargo_impl_package_name(None, "traderBFF") == "traderBFF_impl"

    def test_full_name_unchanged(self) -> None:
        assert (
            resolve_cargo_impl_package_name("amd_service_api_impl", "ignored")
            == "amd_service_api_impl"
        )

    def test_rerp_prefix_passthrough(self) -> None:
        assert (
            resolve_cargo_impl_package_name("rerp_trader_amd_impl", "amd") == "rerp_trader_amd_impl"
        )

    def test_arbitrary_package_passthrough(self) -> None:
        assert resolve_cargo_impl_package_name("custom_crate", "amd") == "custom_crate"


class TestHostAwareBuildResolvesPackage:
    def test_trader_amd_impl_passes_amd_service_api_impl(self, tmp_path: Path) -> None:
        """Regression: Tilt passes --package amd_impl; cargo -p must be amd_service_api_impl."""
        (tmp_path / "microservices").mkdir()
        (tmp_path / "microservices" / "Cargo.toml").write_text("[workspace]\n")

        with (
            patch(
                "brrtrouter_tooling.build.host_aware._install_rust_target",
                return_value=True,
            ),
            patch("brrtrouter_tooling.build.host_aware.subprocess.run") as m_run,
        ):
            m_run.return_value = type("R", (), {"returncode": 0})()
            from brrtrouter_tooling.build.host_aware import run

            rc = run(
                "trader_amd",
                arch="amd64",
                project_root=tmp_path,
                package="amd_impl",
            )
        assert rc == 0
        cmd = m_run.call_args[0][0]
        assert "-p" in cmd
        i = cmd.index("-p")
        assert cmd[i + 1] == "amd_service_api_impl"

    def test_trader_amd_no_package_uses_impl_name(self, tmp_path: Path) -> None:
        (tmp_path / "microservices").mkdir()
        (tmp_path / "microservices" / "Cargo.toml").write_text("[workspace]\n")

        with (
            patch(
                "brrtrouter_tooling.build.host_aware._install_rust_target",
                return_value=True,
            ),
            patch("brrtrouter_tooling.build.host_aware.subprocess.run") as m_run,
        ):
            m_run.return_value = type("R", (), {"returncode": 0})()
            from brrtrouter_tooling.build.host_aware import run

            rc = run("trader_amd", arch="amd64", project_root=tmp_path, package=None)
        assert rc == 0
        cmd = m_run.call_args[0][0]
        i = cmd.index("-p")
        assert cmd[i + 1] == "amd_service_api_impl"

    def test_bff_traderbff_resolves_traderbff_impl(self, tmp_path: Path) -> None:
        (tmp_path / "microservices").mkdir()
        (tmp_path / "microservices" / "Cargo.toml").write_text("[workspace]\n")

        with (
            patch(
                "brrtrouter_tooling.build.host_aware._install_rust_target",
                return_value=True,
            ),
            patch("brrtrouter_tooling.build.host_aware.subprocess.run") as m_run,
        ):
            m_run.return_value = type("R", (), {"returncode": 0})()
            from brrtrouter_tooling.build.host_aware import run

            rc = run("bff_traderBFF", arch="amd64", project_root=tmp_path, package=None)
        assert rc == 0
        cmd = m_run.call_args[0][0]
        i = cmd.index("-p")
        assert cmd[i + 1] == "traderBFF_impl"

    def test_trader_market_data_resolves_snake_impl_package(self, tmp_path: Path) -> None:
        """Target trader_market-data → impl crate market_data_service_api_impl (kebab in path)."""
        (tmp_path / "microservices").mkdir()
        (tmp_path / "microservices" / "Cargo.toml").write_text("[workspace]\n")

        with (
            patch(
                "brrtrouter_tooling.build.host_aware._install_rust_target",
                return_value=True,
            ),
            patch("brrtrouter_tooling.build.host_aware.subprocess.run") as m_run,
        ):
            m_run.return_value = type("R", (), {"returncode": 0})()
            from brrtrouter_tooling.build.host_aware import run

            rc = run("trader_market-data", arch="amd64", project_root=tmp_path, package=None)
        assert rc == 0
        cmd = m_run.call_args[0][0]
        i = cmd.index("-p")
        assert cmd[i + 1] == "market_data_service_api_impl"
