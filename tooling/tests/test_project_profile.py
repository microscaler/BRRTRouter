"""ProjectProfile: every field derived from the project name unless the project says otherwise."""

import warnings
from pathlib import Path

import pytest

from brrtrouter_tooling.workspace import legacy_env, profile


@pytest.fixture(autouse=True)
def _fresh() -> None:
    profile.clear_cache()
    yield
    profile.clear_cache()


def test_defaults_from_directory_name(tmp_path: Path) -> None:
    root = tmp_path / "Acme ERP"
    root.mkdir()
    p = profile.load_profile(root)
    assert p.name == "acme-erp"
    assert p.cli == "acme-erp"
    assert p.package_prefix == "acme_erp"
    assert p.binary_name("trader", "market-data") == "acme_erp_trader_market_data_impl"
    assert p.image_name("trader", "market") == "acme-erp-trader-market"
    assert p.base_image == "acme-erp-base"
    assert p.zip_prefix == "acme-erp-binaries-"
    assert p.helm_values == "helm/acme-erp-microservice/values"
    assert p.flat_suite == "acme-erp"
    assert p.default_suite == "acme-erp"
    assert p.kind_cluster == "acme-erp"
    assert p.tilt_port == 10350
    assert p.source == "defaults"


def test_brrtrouter_toml_overrides(tmp_path: Path) -> None:
    (tmp_path / "brrtrouter.toml").write_text(
        '[project]\nname = "acme"\ncli = "acme-dev"\n'
        '[naming]\npackage_prefix = "ac"\nbase_image = "ac/base"\n'
        '[layout]\nflat_suite = "core"\ndefault_suite = "accounting"\n'
        "[dev]\ntilt_port = 10352\n"
    )
    p = profile.load_profile(tmp_path)
    assert (p.name, p.cli, p.package_prefix, p.base_image) == ("acme", "acme-dev", "ac", "ac/base")
    assert (p.flat_suite, p.default_suite, p.tilt_port) == ("core", "accounting", 10352)
    assert p.binary_name("accounting", "gl") == "ac_accounting_gl_impl"
    assert p.source.endswith("brrtrouter.toml")


def test_pyproject_tool_table(tmp_path: Path) -> None:
    (tmp_path / "tooling").mkdir()
    (tmp_path / "tooling" / "pyproject.toml").write_text(
        '[project]\nname = "x"\n[tool.brrtrouter.project]\nname = "acme"\n'
    )
    assert profile.load_profile(tmp_path).name == "acme"


def test_cli_name_follows_the_active_profile(tmp_path: Path) -> None:
    (tmp_path / "brrtrouter.toml").write_text('[project]\nname = "acme"\n')
    profile.activate(profile.load_profile(tmp_path))
    assert profile.cli_name() == "acme"


def test_legacy_env_names_warn(monkeypatch: pytest.MonkeyPatch) -> None:
    for old in legacy_env.LEGACY_ENV["BRRTROUTER_PORT_REGISTRY"]:
        monkeypatch.delenv(old, raising=False)
    monkeypatch.delenv("BRRTROUTER_PORT_REGISTRY", raising=False)
    old = legacy_env.LEGACY_ENV["BRRTROUTER_PORT_REGISTRY"][0]
    monkeypatch.setenv(old, "/tmp/reg.json")
    with pytest.warns(DeprecationWarning, match="BRRTROUTER_PORT_REGISTRY"):
        assert legacy_env.env("BRRTROUTER_PORT_REGISTRY") == "/tmp/reg.json"
    monkeypatch.setenv("BRRTROUTER_PORT_REGISTRY", "/tmp/new.json")
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        assert legacy_env.env("BRRTROUTER_PORT_REGISTRY") == "/tmp/new.json"
