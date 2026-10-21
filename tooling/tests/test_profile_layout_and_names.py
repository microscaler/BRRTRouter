"""BRRTRouter#488 review: profile layout paths are honoured, one image contract, one crate prefix, gen membership by
pattern, gen_if_missing stops on failure."""

from pathlib import Path
from unittest.mock import patch

import yaml

from brrtrouter_tooling.workspace.gen import regenerate as rg
from brrtrouter_tooling.workspace.profile import load_profile


def _spec(root: Path, rel: str) -> None:
    p = root / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text("openapi: 3.1.0\ninfo: {title: t, version: '1'}\npaths: {}\n")


def _toml(root: Path, text: str) -> None:
    (root / "brrtrouter.toml").write_text(text)


# --- A. layout paths -------------------------------------------------------------------------------------------------


def test_discovery_uses_profile_openapi_and_workspace_dirs(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.discovery.suites import (
        resolve_service_microservice_dir,
        suite_sub_service_names,
    )

    _toml(tmp_path, '[layout]\nopenapi_dir = "specs"\nworkspace_dir = "crates"\n')
    _spec(tmp_path, "specs/trader/market/openapi.yaml")
    assert suite_sub_service_names(tmp_path, "trader") == ["market"]
    assert resolve_service_microservice_dir(tmp_path, "trader", "market") == (
        tmp_path / "crates" / "trader" / "market"
    )


def test_bootstrap_layout_profile_first_explicit_wins(tmp_path: Path) -> None:
    from brrtrouter_tooling.bootstrap.config import resolve_bootstrap_layout

    _toml(
        tmp_path,
        '[layout]\nopenapi_dir = "specs"\nworkspace_dir = "crates"\ndefault_suite = "trader"\n',
    )
    lay = resolve_bootstrap_layout(None, tmp_path)
    assert (lay["openapi_dir"], lay["workspace_dir"], lay["suite"]) == ("specs", "crates", "trader")
    lay = resolve_bootstrap_layout({"workspace_dir": "other"}, tmp_path)
    assert lay["workspace_dir"] == "other"
    assert lay["openapi_dir"] == "specs"


def test_ports_layout_uses_profile_helm_values_and_openapi(tmp_path: Path) -> None:
    from brrtrouter_tooling.ports.layout import resolve_layout

    _toml(tmp_path, '[layout]\nopenapi_dir = "specs"\nhelm_values = "deploy/values"\n')
    lay = resolve_layout(None, tmp_path)
    assert lay["openapi_dir"] == "specs"
    assert Path(lay["helm_values_dir"]) == (tmp_path / "deploy" / "values").resolve()
    lay = resolve_layout({"helm_values_dir": "x/values"}, tmp_path)
    assert Path(lay["helm_values_dir"]) == (tmp_path / "x" / "values").resolve()


# --- B. one image-name contract --------------------------------------------------------------------------------------


def test_image_name_default_and_suite_qualified_pattern(tmp_path: Path) -> None:
    assert load_profile(tmp_path).image_name("trader", "market") == "acme-market"
    other = tmp_path / "other"
    other.mkdir()
    _toml(other, '[naming]\nimage_pattern = "{prefix}-{suite}-{service}"\n')
    assert load_profile(other).image_name("trader", "market") == "acme-trader-market"


def _values(root: Path, service: str) -> Path:
    v = root / "helm" / "acme-microservice" / "values" / f"{service}.yaml"
    v.parent.mkdir(parents=True, exist_ok=True)
    v.write_text("image: {}\n")
    return v


def test_workspace_ports_writes_profile_image_name(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.ports import PortRegistry

    _toml(tmp_path, '[naming]\nimage_pattern = "{prefix}-{suite}-{service}"\n')
    _spec(tmp_path, "openapi/trader/market/openapi.yaml")
    v = _values(tmp_path, "market")
    PortRegistry(tmp_path / "port-registry.json", tmp_path).update_config_files("market", 8010)
    assert yaml.safe_load(v.read_text())["image"]["name"] == "acme-trader-market"


def test_ports_registry_writes_profile_image_name(tmp_path: Path) -> None:
    from brrtrouter_tooling.ports.registry import PortRegistry

    _spec(tmp_path, "openapi/trader/market/openapi.yaml")
    v = _values(tmp_path, "market")
    PortRegistry(tmp_path / "port-registry.json", tmp_path).update_config_files("market", 8010)
    assert yaml.safe_load(v.read_text())["image"]["name"] == "acme-market"


# --- C. gen membership by pattern ------------------------------------------------------------------------------------


def _members(root: Path, members: list[str]) -> None:
    ws = root / "microservices"
    ws.mkdir(parents=True, exist_ok=True)
    body = ", ".join(f'"{m}"' for m in members)
    (ws / "Cargo.toml").write_text(f"[workspace]\nmembers = [{body}]\n")


def test_glob_members_match_a_service_with_its_impl_before_gen_exists(tmp_path: Path) -> None:
    _members(tmp_path, ["trader/*/gen", "trader/*/impl"])
    (tmp_path / "microservices" / "trader" / "market" / "impl").mkdir(parents=True)
    assert rg.service_gen_dir(tmp_path, "trader", "market") == (
        tmp_path / "microservices" / "trader" / "market" / "gen"
    )
    # a proxied spec under the same suite matches the glob but has no crate of its own
    assert rg.service_gen_dir(tmp_path, "trader", "identity") is None
    _members(tmp_path, ["*/gen"])
    (tmp_path / "microservices" / "identity" / "gen").mkdir(parents=True)
    assert rg.is_service_gen_crate(tmp_path, "acme", "identity")


def test_explicit_member_matches_before_the_directory_exists(tmp_path: Path) -> None:
    _members(tmp_path, ["trader/market/gen"])
    assert rg.service_gen_dir(tmp_path, "trader", "market") == (
        tmp_path / "microservices" / "trader" / "market" / "gen"
    )


def test_flat_member_matches_flat_candidate(tmp_path: Path) -> None:
    _members(tmp_path, ["identity/gen", "identity/impl"])
    assert rg.service_gen_dir(tmp_path, "acme", "identity") == (
        tmp_path / "microservices" / "identity" / "gen"
    )
    assert not rg.is_service_gen_crate(tmp_path, "acme", "billing")


def test_empty_or_absent_manifest_falls_back_to_dir_exists(tmp_path: Path) -> None:
    assert rg.workspace_members(tmp_path) is None
    _members(tmp_path, [])
    assert rg.workspace_members(tmp_path) is None
    (tmp_path / "microservices" / "trader" / "market" / "gen").mkdir(parents=True)
    assert rg.is_service_gen_crate(tmp_path, "trader", "market")


# --- D. one crate prefix ---------------------------------------------------------------------------------------------


def test_bootstrap_and_discovery_share_the_crate_prefix(tmp_path: Path) -> None:
    from brrtrouter_tooling.bootstrap.config import resolve_bootstrap_layout
    from brrtrouter_tooling.workspace.discovery.services import get_package_names, package_prefix

    _spec(tmp_path, "openapi/acme/x/openapi.yaml")
    lay = resolve_bootstrap_layout(None, tmp_path)
    assert lay["crate_name_prefix"] == package_prefix(tmp_path, "acme") == "acme"
    assert get_package_names(tmp_path, suite="acme")["x"] == "acme_x"

    other = tmp_path / "erp"
    other.mkdir()
    _toml(other, '[naming]\ncrate_pattern = "{prefix}_{suite}_{service}"\n')
    _spec(other, "openapi/accounting/ledger/openapi.yaml")
    lay = resolve_bootstrap_layout({"suite": "accounting"}, other)
    assert lay["crate_name_prefix"] == package_prefix(other, "accounting") == "acme_accounting"
    assert get_package_names(other, suite="accounting")["ledger"] == "acme_accounting_ledger"


def test_crate_pattern_must_end_with_service(tmp_path: Path) -> None:
    import pytest

    _toml(tmp_path, '[naming]\ncrate_pattern = "{service}_{prefix}"\n')
    with pytest.raises(ValueError, match="crate_pattern"):
        load_profile(tmp_path)


# --- E. gen_if_missing -----------------------------------------------------------------------------------------------


def test_gen_if_missing_covers_suites_without_bff_and_stops_on_failure(tmp_path: Path) -> None:
    from brrtrouter_tooling.workspace.build import microservices as ms

    _spec(tmp_path, "openapi/trader/market/openapi.yaml")
    _spec(tmp_path, "openapi/trader/proxied/openapi.yaml")
    _members(tmp_path, ["trader/market/gen"])
    seen = []

    def fake(root, suite, name, allow_new=False, output_dir=None):
        seen.append((suite, name, output_dir))
        return 1

    with patch.object(ms, "regenerate_service", fake), patch.object(ms, "run_host_aware") as build:
        assert ms.build_microservices_workspace(tmp_path, "amd64", False) == 1
        build.assert_not_called()
    assert seen == [("trader", "market", tmp_path / "microservices" / "trader" / "market" / "gen")]
