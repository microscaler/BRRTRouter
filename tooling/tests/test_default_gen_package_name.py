"""Tests for helpers.default_gen_package_name (kebab-case service dirs → valid gen crate names)."""

from brrtrouter_tooling.helpers import default_gen_package_name


class TestDefaultGenPackageName:
    def test_kebab_service_dir(self) -> None:
        assert default_gen_package_name("market-data") == "market_data_service_api"

    def test_plain_snake_unchanged(self) -> None:
        assert default_gen_package_name("amd") == "amd_service_api"

    def test_matches_impl_naming_convention(self) -> None:
        """Gen package is impl package without _impl suffix."""
        from brrtrouter_tooling.helpers import brrtrouter_impl_package_name

        svc = "market-data"
        assert default_gen_package_name(svc) + "_impl" == brrtrouter_impl_package_name(svc)
