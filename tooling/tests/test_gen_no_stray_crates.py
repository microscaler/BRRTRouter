"""`gen suite` only regenerates a suite's own services: a spec the BFF merely proxies gets no gen crate."""

from pathlib import Path

import pytest

from brrtrouter_tooling.workspace.gen import regenerate as rg


@pytest.fixture
def project(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    for svc in ("market", "identity"):
        spec = tmp_path / "openapi" / "trader" / svc / "openapi.yaml"
        spec.parent.mkdir(parents=True)
        spec.write_text("openapi: 3.1.0\n")
    (tmp_path / "microservices" / "trader" / "market" / "gen").mkdir(parents=True)
    monkeypatch.setattr(
        rg,
        "resolve_service_microservice_dir",
        lambda root, suite, svc: root / "microservices" / suite / svc,
    )
    monkeypatch.setattr(
        rg,
        "resolve_service_openapi_spec_path",
        lambda root, suite, svc: root / "openapi" / suite / svc / "openapi.yaml",
    )
    monkeypatch.setattr(rg, "_gen_package_name", lambda *a, **k: None)
    monkeypatch.setattr(rg, "discover_brrtrouter_root", lambda root: root)
    return tmp_path


def test_existing_gen_services_splits_proxied(project: Path) -> None:
    have, proxied = rg.existing_gen_services(project, "trader", ["identity", "market"])
    assert have == ["market"]
    assert proxied == ["identity"]


def test_service_without_gen_crate_is_refused(
    project: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    calls = []
    monkeypatch.setattr(rg, "call_brrtrouter_generate", lambda **kw: calls.append(kw))
    assert rg.regenerate_service(project, "trader", "identity") == 1
    assert calls == []
    assert not (project / "microservices" / "trader" / "identity").exists()


def test_allow_new_creates(project: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    class Ok:
        returncode = 0

    calls = []
    monkeypatch.setattr(rg, "call_brrtrouter_generate", lambda **kw: calls.append(kw) or Ok())
    assert rg.regenerate_service(project, "trader", "identity", allow_new=True) == 0
    assert (
        calls
        and calls[0]["output_dir"] == project / "microservices" / "trader" / "identity" / "gen"
    )
