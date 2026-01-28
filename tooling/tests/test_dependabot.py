"""Tests for Dependabot automerge tooling."""

import json
import os
import tempfile
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from brrtrouter_tooling.dependabot.automerge import (
    check_mergeability,
    comment_on_major_update,
    extract_metadata_from_title,
    extract_pr_info,
    get_event_data,
    get_event_name,
    get_github_token,
    get_repository,
    is_dependabot_pr,
    merge_pr,
    process_dependabot_pr,
    run_gh_command,
)


def test_get_event_name() -> None:
    """Test getting event name from environment."""
    with patch.dict(os.environ, {"GITHUB_EVENT_NAME": "pull_request"}):
        assert get_event_name() == "pull_request"


def test_get_repository() -> None:
    """Test getting repository from environment."""
    with patch.dict(os.environ, {"GITHUB_REPOSITORY": "owner/repo"}):
        assert get_repository() == "owner/repo"


def test_get_github_token() -> None:
    """Test getting GitHub token from environment."""
    with patch.dict(os.environ, {"GITHUB_TOKEN": "test-token"}):
        assert get_github_token() == "test-token"

    with patch.dict(os.environ, {"GH_TOKEN": "test-token-gh"}, clear=True):
        assert get_github_token() == "test-token-gh"

    with patch.dict(os.environ, {}, clear=True):
        with pytest.raises(SystemExit):
            get_github_token()


def test_get_event_data() -> None:
    """Test getting event data from file."""
    event_data = {"pull_request": {"number": 123}}
    with tempfile.NamedTemporaryFile(mode="w", delete=False, suffix=".json") as f:
        json.dump(event_data, f)
        event_path = f.name

    try:
        with patch.dict(os.environ, {"GITHUB_EVENT_PATH": event_path}):
            result = get_event_data()
            assert result == event_data
    finally:
        Path(event_path).unlink()


def test_run_gh_command() -> None:
    """Test running GitHub CLI command."""
    with patch("subprocess.run") as mock_run:
        mock_result = MagicMock()
        mock_result.stdout = "test output"
        mock_result.stderr = ""
        mock_run.return_value = mock_result

        result = run_gh_command(["pr", "view", "123"], "test-token")
        assert result == "test output"
        mock_run.assert_called_once()


def test_extract_pr_info_pull_request() -> None:
    """Test extracting PR info from pull_request event."""
    event_data = {
        "pull_request": {
            "number": 123,
            "html_url": "https://github.com/owner/repo/pull/123",
        },
    }
    with tempfile.NamedTemporaryFile(mode="w", delete=False, suffix=".json") as f:
        json.dump(event_data, f)
        event_path = f.name

    try:
        with patch.dict(
            os.environ,
            {
                "GITHUB_EVENT_NAME": "pull_request",
                "GITHUB_EVENT_PATH": event_path,
                "GITHUB_REPOSITORY": "owner/repo",
            },
        ):
            pr_number, pr_url = extract_pr_info()
            assert pr_number == 123
            assert pr_url == "https://github.com/owner/repo/pull/123"
    finally:
        Path(event_path).unlink()


def test_extract_pr_info_check_suite() -> None:
    """Test extracting PR info from check_suite event."""
    event_data = {
        "check_suite": {
            "pull_requests": [{"number": 456}],
        },
    }
    with tempfile.NamedTemporaryFile(mode="w", delete=False, suffix=".json") as f:
        json.dump(event_data, f)
        event_path = f.name

    try:
        with patch.dict(
            os.environ,
            {
                "GITHUB_EVENT_NAME": "check_suite",
                "GITHUB_EVENT_PATH": event_path,
                "GITHUB_REPOSITORY": "owner/repo",
            },
        ):
            pr_number, pr_url = extract_pr_info()
            assert pr_number == 456
            assert pr_url == "https://github.com/owner/repo/pull/456"
    finally:
        Path(event_path).unlink()


def test_is_dependabot_pr() -> None:
    """Test checking if PR is from Dependabot."""
    with patch("brrtrouter_tooling.dependabot.automerge.run_gh_command") as mock_gh:
        mock_gh.return_value = json.dumps({"author": {"login": "dependabot[bot]"}})
        with patch.dict(os.environ, {"GITHUB_TOKEN": "test-token"}):
            assert is_dependabot_pr(123) is True

        mock_gh.return_value = json.dumps({"author": {"login": "other-user"}})
        with patch.dict(os.environ, {"GITHUB_TOKEN": "test-token"}):
            assert is_dependabot_pr(123) is False


def test_extract_metadata_from_title() -> None:
    """Test extracting metadata from PR title."""
    with patch("brrtrouter_tooling.dependabot.automerge.run_gh_command") as mock_gh:
        # Major version update
        mock_gh.return_value = json.dumps(
            {"title": "Bump jsonwebtoken from 1.0.0 to 2.0.0"},
        )
        with patch.dict(os.environ, {"GITHUB_TOKEN": "test-token"}):
            deps, update_type = extract_metadata_from_title(123)
            assert deps == "jsonwebtoken"
            assert update_type == "version-update:semver-major"

        # Minor version update
        mock_gh.return_value = json.dumps(
            {"title": "Bump jsonwebtoken from 1.0.0 to 1.1.0"},
        )
        with patch.dict(os.environ, {"GITHUB_TOKEN": "test-token"}):
            deps, update_type = extract_metadata_from_title(123)
            assert update_type == "version-update:semver-minor"

        # Patch version update
        mock_gh.return_value = json.dumps(
            {"title": "Bump jsonwebtoken from 1.0.0 to 1.0.1"},
        )
        with patch.dict(os.environ, {"GITHUB_TOKEN": "test-token"}):
            deps, update_type = extract_metadata_from_title(123)
            assert update_type == "version-update:semver-patch"


def test_check_mergeability() -> None:
    """Test checking PR mergeability."""
    with patch("brrtrouter_tooling.dependabot.automerge.run_gh_command") as mock_gh:
        mock_gh.return_value = json.dumps({"mergeable": True, "mergeableState": "clean"})
        with patch.dict(os.environ, {"GITHUB_TOKEN": "test-token"}):
            mergeable, state, ready = check_mergeability(123)
            assert mergeable is True
            assert state == "clean"
            assert ready is True

        mock_gh.return_value = json.dumps({"mergeable": False, "mergeableState": "dirty"})
        with patch.dict(os.environ, {"GITHUB_TOKEN": "test-token"}):
            mergeable, state, ready = check_mergeability(123)
            assert mergeable is False
            assert ready is False


def test_merge_pr() -> None:
    """Test merging a PR."""
    with patch("brrtrouter_tooling.dependabot.automerge.run_gh_command") as mock_gh:
        with patch.dict(os.environ, {"GITHUB_TOKEN": "test-token"}):
            merge_pr("https://github.com/owner/repo/pull/123", "jsonwebtoken", "version-update:semver-patch")
            mock_gh.assert_called_once_with(
                ["pr", "merge", "https://github.com/owner/repo/pull/123", "--squash", "--auto"],
                "test-token",
            )


def test_comment_on_major_update() -> None:
    """Test commenting on major update."""
    with patch("brrtrouter_tooling.dependabot.automerge.run_gh_command") as mock_gh:
        with patch.dict(os.environ, {"GITHUB_TOKEN": "test-token"}):
            comment_on_major_update("https://github.com/owner/repo/pull/123", "jsonwebtoken")
            assert mock_gh.called
            call_args = mock_gh.call_args[0][0]
            assert call_args[0] == "pr"
            assert call_args[1] == "comment"
            assert "jsonwebtoken" in call_args[4]


def test_process_dependabot_pr_non_dependabot() -> None:
    """Test processing non-Dependabot PR."""
    event_data = {
        "pull_request": {
            "number": 123,
            "html_url": "https://github.com/owner/repo/pull/123",
        },
    }
    with tempfile.NamedTemporaryFile(mode="w", delete=False, suffix=".json") as f:
        json.dump(event_data, f)
        event_path = f.name

    try:
        with patch.dict(
            os.environ,
            {
                "GITHUB_EVENT_NAME": "pull_request",
                "GITHUB_EVENT_PATH": event_path,
                "GITHUB_REPOSITORY": "owner/repo",
                "GITHUB_TOKEN": "test-token",
            },
        ):
            with patch("brrtrouter_tooling.dependabot.automerge.is_dependabot_pr") as mock_check:
                mock_check.return_value = False
                with pytest.raises(SystemExit) as exc_info:
                    process_dependabot_pr()
                assert exc_info.value.code == 0
    finally:
        Path(event_path).unlink()
