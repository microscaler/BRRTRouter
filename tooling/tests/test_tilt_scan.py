"""Tests for tilt.scan binary name mapping."""

from pathlib import Path


def test_scan_maps_kebab_service_to_snake_binary_name(tmp_path: Path) -> None:
    from brrtrouter_tooling.tilt.scan import run

    d = tmp_path / "trader" / "market-data"
    d.mkdir(parents=True)
    (d / "openapi.yaml").write_text(
        "openapi: 3.1.0\ninfo:\n  title: x\n  version: '1'\npaths: {}\n"
    )

    out = run(str(tmp_path / "trader"), 8002)
    assert isinstance(out, dict)
    assert out["binary_names"]["market-data"] == "market_data_service_api"
