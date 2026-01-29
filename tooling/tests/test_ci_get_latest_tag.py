"""Tests for brrtrouter_tooling.ci.get_latest_tag (brrtrouter ci get-latest-tag)."""

import json
import os
import sys
from io import StringIO
from unittest.mock import Mock, patch
from urllib.error import HTTPError, URLError

import pytest

from brrtrouter_tooling.ci import get_latest_tag
from brrtrouter_tooling.ci import run_get_latest_tag as run
from brrtrouter_tooling.helpers import fibonacci_backoff_sequence

# Module under test (use sys.modules so we get the .py module, not ci's re-exported function).
get_latest_tag_module = sys.modules["brrtrouter_tooling.ci.get_latest_tag"]


def _fake_urlopen_success(tag_name: str):
    """Create a mock urlopen that returns successful response."""
    mock_response = Mock()
    mock_response.read.return_value = json.dumps({"tag_name": tag_name}).encode()
    mock_response.__enter__ = Mock(return_value=mock_response)
    mock_response.__exit__ = Mock(return_value=None)
    return mock_response


def _fake_urlopen_404():
    """Create a mock urlopen that returns 404."""
    mock_response = Mock()
    error = HTTPError("url", 404, "Not Found", {}, None)
    mock_response.__enter__ = Mock(side_effect=error)
    mock_response.__exit__ = Mock(return_value=None)
    return error


class TestGetLatestTag:
    def test_returns_latest_tag_from_github_api(self) -> None:
        with patch.object(
            get_latest_tag_module,
            "urlopen",
            return_value=_fake_urlopen_success("v0.39.0"),
        ):
            result = get_latest_tag("owner/repo", "token")
            assert result == "0.39.0"

    def test_strips_v_prefix(self) -> None:
        with patch.object(
            get_latest_tag_module,
            "urlopen",
            return_value=_fake_urlopen_success("v1.2.3"),
        ):
            result = get_latest_tag("owner/repo", "token")
            assert result == "1.2.3"

    def test_handles_tag_without_v_prefix(self) -> None:
        with patch.object(
            get_latest_tag_module,
            "urlopen",
            return_value=_fake_urlopen_success("0.39.0"),
        ):
            result = get_latest_tag("owner/repo", "token")
            assert result == "0.39.0"

    def test_handles_rc_tags(self) -> None:
        with patch.object(
            get_latest_tag_module,
            "urlopen",
            return_value=_fake_urlopen_success("v0.39.0-rc.2"),
        ):
            result = get_latest_tag("owner/repo", "token")
            assert result == "0.39.0-rc.2"

    def test_returns_none_when_no_releases(self) -> None:
        with patch.object(get_latest_tag_module, "urlopen", side_effect=_fake_urlopen_404()):
            result = get_latest_tag("owner/repo", "token")
            assert result is None

    def test_run_prints_version_to_stdout(self) -> None:
        with (
            patch.object(
                get_latest_tag_module,
                "urlopen",
                return_value=_fake_urlopen_success("v0.39.0"),
            ),
            patch.dict(
                os.environ,
                {"GITHUB_REPOSITORY": "owner/repo", "GITHUB_TOKEN": "token"},
                clear=False,
            ),
            patch("sys.stdout", new=StringIO()) as fake_out,
        ):
            result = run()
            assert result == 0
            assert fake_out.getvalue().strip() == "0.39.0"

    def test_run_handles_no_releases(self) -> None:
        with (
            patch.object(
                get_latest_tag_module,
                "urlopen",
                side_effect=_fake_urlopen_404(),
            ),
            patch.dict(
                os.environ,
                {"GITHUB_REPOSITORY": "owner/repo", "GITHUB_TOKEN": "token"},
                clear=False,
            ),
            patch("sys.stdout", new=StringIO()) as fake_out,
        ):
            result = run()
            assert result == 0
            assert fake_out.getvalue().strip() == ""

    def test_retries_on_http_error_then_succeeds(self) -> None:
        """Test retry logic: fails twice with 503, then succeeds."""
        http_error = HTTPError("url", 503, "Service Unavailable", {}, None)
        with (
            patch.object(get_latest_tag_module, "urlopen") as mock_urlopen,
            patch("time.sleep") as mock_sleep,
            patch("sys.stderr", new=StringIO()) as fake_err,
        ):
            mock_urlopen.side_effect = [
                http_error,
                http_error,
                _fake_urlopen_success("v0.39.0"),
            ]
            result = get_latest_tag("owner/repo", "token", max_retries=5)
            assert result == "0.39.0"
            assert mock_sleep.call_count == 2
            assert mock_sleep.call_args_list[0][0][0] == 1
            assert mock_sleep.call_args_list[1][0][0] == 1
            err_output = fake_err.getvalue()
            assert "Retry 1/5" in err_output
            assert "Retry 2/5" in err_output
            assert "HTTP 503" in err_output

    def test_retries_on_urlerror_then_succeeds(self) -> None:
        """Test retry logic: fails with URLError, then succeeds."""
        url_error = URLError("Connection refused")
        with (
            patch.object(get_latest_tag_module, "urlopen") as mock_urlopen,
            patch("time.sleep") as mock_sleep,
            patch("sys.stderr", new=StringIO()) as fake_err,
        ):
            mock_urlopen.side_effect = [url_error, _fake_urlopen_success("v0.39.0")]
            result = get_latest_tag("owner/repo", "token", max_retries=5)
            assert result == "0.39.0"
            assert mock_sleep.call_count == 1
            assert mock_sleep.call_args_list[0][0][0] == 1
            err_output = fake_err.getvalue()
            assert "Retry 1/5" in err_output
            assert "Network error" in err_output

    def test_exhausts_retries_and_raises(self) -> None:
        """Test that SystemExit is raised when all retries are exhausted."""
        http_error = HTTPError("url", 503, "Service Unavailable", {}, None)
        with (
            patch.object(
                get_latest_tag_module,
                "urlopen",
                side_effect=http_error,
            ),
            patch("time.sleep"),
            patch("sys.stderr", new=StringIO()) as fake_err,
        ):
            with pytest.raises(SystemExit) as exc_info:
                get_latest_tag("owner/repo", "token", max_retries=3)
            assert "Failed to fetch" in str(exc_info.value)
            assert "after 3 retries" in str(exc_info.value)
            err_output = fake_err.getvalue()
            assert "Retry 1/3" in err_output
            assert "Retry 2/3" in err_output
            assert "Retry 3/3" in err_output

    def test_fibonacci_backoff_sequence(self) -> None:
        """Test Fibonacci backoff sequence generation."""
        seq = fibonacci_backoff_sequence(max_total_seconds=300)
        assert seq[0] == 1
        assert seq[1] == 1
        assert seq[2] == 2
        assert seq[3] == 3
        assert seq[4] == 5
        assert seq[5] == 8
        assert seq[6] == 13
        assert sum(seq) <= 300

        seq_small = fibonacci_backoff_sequence(max_total_seconds=10)
        assert sum(seq_small) <= 10
        assert seq_small == [1, 1, 2, 3]

    def test_run_fails_after_retries_exhausted(self) -> None:
        """Test that run() raises SystemExit with detailed message when retries are exhausted."""
        http_error = HTTPError("url", 503, "Service Unavailable", {}, None)
        with (
            patch.object(
                get_latest_tag_module,
                "urlopen",
                side_effect=http_error,
            ),
            patch("time.sleep"),
            patch.dict(
                os.environ,
                {"GITHUB_REPOSITORY": "owner/repo", "GITHUB_TOKEN": "token"},
                clear=False,
            ),
            patch("sys.stdout", new=StringIO()),
            patch("sys.stderr", new=StringIO()),
            pytest.raises(SystemExit) as exc_info,
        ):
            run()
        assert "Failed to fetch latest release" in str(exc_info.value)
