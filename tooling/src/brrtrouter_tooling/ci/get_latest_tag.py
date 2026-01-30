"""Get latest release tag from GitHub API."""

import json
import os
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from brrtrouter_tooling.helpers import fibonacci_backoff_sequence


def get_latest_tag(repo: str, token: str, max_retries: int = 20) -> str | None:
    """Get latest release tag from GitHub API with retry logic.

    Returns version string without 'v' prefix or None if no releases exist.
    """
    url = f"https://api.github.com/repos/{repo}/releases/latest"
    headers = {
        "Accept": "application/vnd.github.v3+json",
        "Authorization": f"token {token}",
        "User-Agent": "brrtrouter-tooling",
    }

    backoff_sequence = fibonacci_backoff_sequence(max_total_seconds=300)
    last_error: Exception | None = None

    for attempt in range(max_retries):
        req = Request(url, headers=headers)

        try:
            with urlopen(req, timeout=10) as response:
                data = json.loads(response.read().decode())
                tag_name = data.get("tag_name", "")
                return tag_name.lstrip("v") if tag_name else None
        except json.JSONDecodeError as e:
            last_error = e
            wait_time = (
                backoff_sequence[attempt]
                if attempt < len(backoff_sequence)
                else (backoff_sequence[-1] if backoff_sequence else 1)
            )
            print(
                f"Retry {attempt + 1}/{max_retries}: Invalid JSON ({e}), waiting {wait_time}s...",
                file=sys.stderr,
            )
            time.sleep(wait_time)
        except HTTPError as e:
            if e.code == 404:
                return None
            last_error = e
            wait_time = (
                backoff_sequence[attempt]
                if attempt < len(backoff_sequence)
                else (backoff_sequence[-1] if backoff_sequence else 1)
            )
            print(
                f"Retry {attempt + 1}/{max_retries}: HTTP {e.code} error, waiting {wait_time}s...",
                file=sys.stderr,
            )
            time.sleep(wait_time)
        except URLError as e:
            last_error = e
            wait_time = (
                backoff_sequence[attempt]
                if attempt < len(backoff_sequence)
                else (backoff_sequence[-1] if backoff_sequence else 1)
            )
            print(
                f"Retry {attempt + 1}/{max_retries}: Network error ({e}), waiting {wait_time}s...",
                file=sys.stderr,
            )
            time.sleep(wait_time)

    if isinstance(last_error, HTTPError):
        msg = f"Failed to fetch latest release after {max_retries} retries: HTTP {last_error.code} {last_error.reason}"
    else:
        msg = f"Failed to fetch latest release after {max_retries} retries: {last_error}"
    raise SystemExit(msg) from last_error


def run() -> int:
    """Get latest tag and print to stdout. Uses GITHUB_REPOSITORY and GITHUB_TOKEN env."""
    repo = os.environ.get("GITHUB_REPOSITORY", "")
    token = os.environ.get("GITHUB_TOKEN", "")

    if not repo:
        print("Error: GITHUB_REPOSITORY environment variable is required", file=sys.stderr)
        return 1

    if not token:
        print("Error: GITHUB_TOKEN environment variable is required", file=sys.stderr)
        return 1

    latest = get_latest_tag(repo, token)
    if latest:
        print(latest)
    return 0
