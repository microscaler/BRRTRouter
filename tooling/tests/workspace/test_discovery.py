from pathlib import Path

from brrtrouter_tooling.workspace.discovery.suites import (
    bff_service_to_suite,
    bff_suite_config_path,
    get_bff_service_name_from_config,
    iter_bffs,
    iter_suite_services,
    load_suite_services,
    openapi_bff_path,
    service_to_suite,
    suite_sub_service_names,
    suites_with_bff,
    tilt_service_names,
)


class TestSuites:
    def test_suites_with_bff_empty(self, tmp_path: Path):
        assert suites_with_bff(tmp_path) == []

    def test_suites_with_bff_returns_suites_with_config(self, tmp_path: Path):
        (tmp_path / "openapi").mkdir(exist_ok=True, parents=True)
        (tmp_path / "openapi" / "bff-suite-config.yaml").write_text("")
        assert suites_with_bff(tmp_path) == ["hauliage"]

    def test_bff_suite_config_path(self, tmp_path: Path):
        assert (
            bff_suite_config_path(tmp_path, "hauliage")
            == tmp_path / "openapi" / "bff-suite-config.yaml"
        )

    def test_openapi_bff_path(self, tmp_path: Path):
        assert openapi_bff_path(tmp_path, "hauliage") == tmp_path / "openapi" / "openapi_bff.yaml"

    def test_service_to_suite_finds_spec(self, tmp_path: Path):
        d = tmp_path / "openapi" / "identity"
        d.mkdir(parents=True)
        (d / "openapi.yaml").write_text("")
        assert service_to_suite(tmp_path, "identity") == "hauliage"

    def test_get_bff_service_name_from_config(self):
        assert get_bff_service_name_from_config({"bff_service_name": "x"}) == "x"
        assert get_bff_service_name_from_config({"metadata": {"bff_service_name": "y"}}) == "y"

    def test_iter_bffs_yields_bff_name_and_suite(self, tmp_path: Path):
        (tmp_path / "openapi").mkdir(exist_ok=True, parents=True)
        (tmp_path / "openapi" / "bff-suite-config.yaml").write_text("bff_service_name: bff\n")
        assert list(iter_bffs(tmp_path)) == [("bff", "hauliage")]

    def test_bff_service_to_suite(self, tmp_path: Path):
        (tmp_path / "openapi").mkdir(exist_ok=True, parents=True)
        (tmp_path / "openapi" / "bff-suite-config.yaml").write_text("bff_service_name: bff\n")
        assert bff_service_to_suite(tmp_path, "bff") == "hauliage"

    def test_load_suite_services_returns_keys_and_bff_name(self, tmp_path: Path):
        (tmp_path / "openapi").mkdir(exist_ok=True, parents=True)
        (tmp_path / "openapi" / "bff-suite-config.yaml").write_text(
            "bff_service_name: bff\nservices:\n  a: {}\n  b: {}\n"
        )
        assert load_suite_services(tmp_path) == {"bff", "a", "b"}

    def test_suite_sub_service_names_returns_dirs_with_openapi_yaml(self, tmp_path: Path):
        (tmp_path / "openapi" / "svc-a").mkdir(parents=True)
        (tmp_path / "openapi" / "svc-b").mkdir(parents=True)
        (tmp_path / "openapi" / "svc-a" / "openapi.yaml").write_text("")
        (tmp_path / "openapi" / "svc-b" / "openapi.yaml").write_text("")
        assert suite_sub_service_names(tmp_path, "hauliage") == ["svc-a", "svc-b"]

    def test_iter_suite_services_yields_all(self, tmp_path: Path):
        (tmp_path / "openapi" / "svc").mkdir(parents=True)
        (tmp_path / "openapi" / "svc" / "openapi.yaml").write_text("")
        assert list(iter_suite_services(tmp_path)) == [("hauliage", "svc")]

    def test_tilt_service_names_includes_bff_and_services(self, tmp_path: Path):
        (tmp_path / "openapi").mkdir(exist_ok=True, parents=True)
        (tmp_path / "openapi" / "bff-suite-config.yaml").write_text(
            "bff_service_name: bff\nservices:\n  a: {}\n  b: {}\n"
        )
        assert tilt_service_names(tmp_path) == ["a", "b", "bff"]
