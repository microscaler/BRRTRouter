"""Regression tests: generated Tiltfiles must call the renamed `brrtrouter-tooling` CLI."""

import pytest

from brrtrouter_tooling.workspace.tilt.tilt_gen import generate_tiltfile

BASE = {
    "project_name": "demo",
    "suite_name": "trader",
    "brrtrouter_root": "../BRRTRouter",
    "docker_image_prefix": "localhost:5001/demo",
    "helm_chart_path": "helm/demo",
    "target_rust_triple": "x86_64-unknown-linux-musl",
    "has_bff": True,
}


@pytest.mark.parametrize("cli_name", [None, "brrtrouter-tooling", "brrtrouter"])
def test_stock_cli_is_unwrapped_and_uses_renamed_binary(cli_name):
    cfg = dict(BASE)
    if cli_name is not None:
        cfg["cli_name"] = cli_name
    out = generate_tiltfile(cfg)
    assert "/bin/brrtrouter-tooling'" in out
    assert "/bin/brrtrouter'" not in out
    assert "brrtrouter-tooling client gen suite bff" in out
    assert "demo client gen suite bff" not in out
    # Unwrapped mode must go through the `client` scope.
    assert "brrtrouter-tooling client docker copy-binary" in out


def test_wrapped_project_cli_unchanged():
    out = generate_tiltfile({**BASE, "cli_name": "hauliage"})
    assert "hauliage_bin = " in out
    assert "brrtrouter_bin = hauliage_bin" in out
