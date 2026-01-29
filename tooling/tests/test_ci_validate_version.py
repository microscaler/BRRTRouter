"""Tests for brrtrouter_tooling.ci.validate_version (brrtrouter ci validate-version)."""

from io import StringIO
from unittest.mock import patch

import pytest

from brrtrouter_tooling.ci import (
    compare_versions,
    run_validate_version,
    run_validate_version_cli,
    validate_version,
)


class TestCompareVersions:
    def test_returns_positive_for_upgrade(self) -> None:
        assert compare_versions("0.40.0", "0.39.0") > 0
        assert compare_versions("0.39.1", "0.39.0") > 0
        assert compare_versions("1.0.0", "0.39.0") > 0

    def test_returns_negative_for_downgrade(self) -> None:
        assert compare_versions("0.39.0", "0.40.0") < 0
        assert compare_versions("0.39.0", "0.39.1") < 0
        assert compare_versions("0.1.0", "0.39.0") < 0

    def test_returns_zero_for_same_version(self) -> None:
        assert compare_versions("0.39.0", "0.39.0") == 0

    def test_handles_rc_versions(self) -> None:
        assert compare_versions("0.39.0-rc.2", "0.39.0") < 0
        assert compare_versions("0.39.0", "0.39.0-rc.2") > 0
        assert compare_versions("0.39.0-rc.3", "0.39.0-rc.2") > 0
        assert compare_versions("0.39.0-rc.2", "0.39.0-rc.3") < 0

    def test_handles_v_prefix(self) -> None:
        assert compare_versions("v0.40.0", "0.39.0") > 0
        assert compare_versions("0.40.0", "v0.39.0") > 0
        assert compare_versions("v0.40.0", "v0.39.0") > 0


class TestValidateVersion:
    def test_allows_upgrade(self) -> None:
        assert validate_version("0.40.0", "0.39.0", allow_same=False) == 0

    def test_rejects_downgrade(self) -> None:
        with patch("sys.stderr", new=StringIO()) as mock_stderr:
            with pytest.raises(SystemExit) as exc_info:
                validate_version("0.39.0", "0.40.0", allow_same=False)
            assert exc_info.value.code == 1
            assert "Version downgrade detected" in mock_stderr.getvalue()

    def test_rejects_same_version_by_default(self) -> None:
        with patch("sys.stderr", new=StringIO()) as mock_stderr:
            with pytest.raises(SystemExit) as exc_info:
                validate_version("0.39.0", "0.39.0", allow_same=False)
            assert exc_info.value.code == 1
            assert "is not greater than" in mock_stderr.getvalue()

    def test_allows_same_version_when_flag_set(self) -> None:
        assert validate_version("0.39.0", "0.39.0", allow_same=True) == 0

    def test_allows_first_release(self) -> None:
        assert validate_version("0.1.0", None, allow_same=False) == 0

    def test_handles_rc_versions(self) -> None:
        assert validate_version("0.39.0-rc.1", "0.38.0", allow_same=False) == 0
        with patch("sys.stderr", new=StringIO()) as mock_stderr:
            with pytest.raises(SystemExit) as exc_info:
                validate_version("0.38.0-rc.1", "0.39.0", allow_same=False)
            assert exc_info.value.code == 1
            assert "Version downgrade detected" in mock_stderr.getvalue()


class TestRun:
    def test_run_validates_successfully(self) -> None:
        with patch("sys.argv", ["validate-version", "--current", "0.40.0", "--latest", "0.39.0"]):
            result = run_validate_version()
            assert result == 0

    def test_run_fails_on_downgrade(self) -> None:
        with (
            patch(
                "sys.argv",
                ["validate-version", "--current", "0.39.0", "--latest", "0.40.0"],
            ),
            patch("sys.stderr"),
        ):
            result = run_validate_version()
            assert result == 1

    def test_run_allows_same_with_flag(self) -> None:
        with patch(
            "sys.argv",
            [
                "validate-version",
                "--current",
                "0.39.0",
                "--latest",
                "0.39.0",
                "--allow-same",
            ],
        ):
            result = run_validate_version()
            assert result == 0


class TestRunValidateVersionCli:
    """Test run_validate_version_cli function (used by CLI integration)."""

    def test_validates_successfully(self) -> None:
        result = run_validate_version_cli(current="0.40.0", latest="0.39.0", allow_same=False)
        assert result == 0

    def test_fails_on_downgrade(self) -> None:
        with patch("sys.stderr", new=StringIO()):
            result = run_validate_version_cli(current="0.39.0", latest="0.40.0", allow_same=False)
        assert result == 1

    def test_allows_same_with_flag(self) -> None:
        result = run_validate_version_cli(current="0.39.0", latest="0.39.0", allow_same=True)
        assert result == 0

    def test_requires_current(self) -> None:
        with patch("sys.stderr", new=StringIO()):
            result = run_validate_version_cli(current=None, latest="0.39.0", allow_same=False)
        assert result == 1

    def test_fetches_latest_from_github(self) -> None:
        with (
            patch.dict(
                "os.environ",
                {"GITHUB_REPOSITORY": "owner/repo", "GITHUB_TOKEN": "token"},
            ),
            patch(
                "brrtrouter_tooling.ci.get_latest_tag.get_latest_tag",
                return_value="0.39.0",
            ),
        ):
            result = run_validate_version_cli(current="0.40.0", latest=None, allow_same=False)
            assert result == 0

    def test_fails_when_no_latest_and_no_github_env(self) -> None:
        with (
            patch.dict("os.environ", {}, clear=True),
            patch("sys.stderr", new=StringIO()),
        ):
            result = run_validate_version_cli(current="0.40.0", latest=None, allow_same=False)
            assert result == 1
