"""OCTOPILOT_CARGO_CONFIG: local cargo --config overrides for any BRRTRouter application."""

from brrtrouter_tooling.build.host_aware import _local_dep_config_args


def test_unset_adds_nothing(monkeypatch):
    monkeypatch.delenv("OCTOPILOT_CARGO_CONFIG", raising=False)
    assert _local_dep_config_args() == []


def test_missing_file_adds_nothing(monkeypatch, tmp_path):
    monkeypatch.setenv("OCTOPILOT_CARGO_CONFIG", str(tmp_path / "absent.config"))
    assert _local_dep_config_args() == []


def test_each_line_is_one_config(monkeypatch, tmp_path):
    f = tmp_path / "local-deps.config"
    f.write_text(
        "# generated\n"
        "\n"
        "patch.'https://github.com/microscaler/BRRTRouter'.brrtrouter.path='../BRRTRouter'\n"
        "  net.git-fetch-with-cli=true  \n"
    )
    monkeypatch.setenv("OCTOPILOT_CARGO_CONFIG", str(f))
    assert _local_dep_config_args() == [
        "--config",
        "patch.'https://github.com/microscaler/BRRTRouter'.brrtrouter.path='../BRRTRouter'",
        "--config",
        "net.git-fetch-with-cli=true",
    ]


def test_the_old_pricewhisperer_name_is_ignored(monkeypatch, tmp_path):
    f = tmp_path / "c"
    f.write_text("a.b=1\n")
    monkeypatch.delenv("OCTOPILOT_CARGO_CONFIG", raising=False)
    monkeypatch.setenv("PW_CARGO_CONFIG", str(f))
    assert _local_dep_config_args() == []
