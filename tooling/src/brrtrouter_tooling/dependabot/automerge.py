"""Dependabot automerge logic."""

import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, Optional, Tuple


def get_event_data() -> Dict[str, Any]:
    """Get GitHub event data from environment or event file."""
    event_path = os.getenv("GITHUB_EVENT_PATH")
    if event_path and Path(event_path).exists():
        with open(event_path, encoding="utf-8") as f:
            return json.load(f)
    return {}


def get_event_name() -> str:
    """Get the GitHub event name."""
    return os.getenv("GITHUB_EVENT_NAME", "")


def get_repository() -> str:
    """Get the GitHub repository in owner/repo format."""
    return os.getenv("GITHUB_REPOSITORY", "")


def get_github_token() -> str:
    """Get the GitHub token from environment."""
    token = os.getenv("GITHUB_TOKEN") or os.getenv("GH_TOKEN")
    if not token:
        print("Error: GITHUB_TOKEN or GH_TOKEN environment variable is required", file=sys.stderr)
        sys.exit(1)
    return token


def run_gh_command(args: list[str], token: str) -> str:
    """Run a GitHub CLI command and return stdout."""
    env = os.environ.copy()
    env["GH_TOKEN"] = token
    try:
        result = subprocess.run(
            ["gh"] + args,
            capture_output=True,
            text=True,
            check=True,
            env=env,
        )
        return result.stdout.strip()
    except subprocess.CalledProcessError as e:
        print(f"Error running gh command: {' '.join(args)}", file=sys.stderr)
        print(f"Error: {e.stderr}", file=sys.stderr)
        sys.exit(1)


def extract_pr_info() -> Tuple[int, str]:
    """Extract PR number and URL based on event type."""
    event_name = get_event_name()
    event_data = get_event_data()
    repository = get_repository()

    if event_name == "pull_request":
        pr_number = event_data.get("pull_request", {}).get("number")
        pr_url = event_data.get("pull_request", {}).get("html_url")
        if not pr_number or not pr_url:
            print("Error: Could not extract PR number or URL from pull_request event", file=sys.stderr)
            sys.exit(1)
        return pr_number, pr_url

    if event_name == "check_suite":
        pull_requests = event_data.get("check_suite", {}).get("pull_requests", [])
        if not pull_requests:
            print("Error: No PR found in check_suite event", file=sys.stderr)
            sys.exit(1)
        pr_number = pull_requests[0].get("number")
        if not pr_number:
            print("Error: Could not extract PR number from check_suite event", file=sys.stderr)
            sys.exit(1)
        pr_url = f"https://github.com/{repository}/pull/{pr_number}"
        return pr_number, pr_url

    if event_name == "status":
        token = get_github_token()
        commit_sha = event_data.get("commit", {}).get("sha") or os.getenv("GITHUB_SHA")
        if not commit_sha:
            print("Error: Could not determine commit SHA for status event", file=sys.stderr)
            sys.exit(1)

        # Find PRs associated with this commit
        result = run_gh_command(
            ["pr", "list", "--state", "open", "--search", f"head:{commit_sha}", "--json", "number"],
            token,
        )
        prs = json.loads(result)
        if not prs:
            print(f"Error: No open PR found for commit {commit_sha}", file=sys.stderr)
            sys.exit(1)
        pr_number = prs[0]["number"]
        pr_url = f"https://github.com/{repository}/pull/{pr_number}"
        return pr_number, pr_url

    print(f"Error: Unsupported event type: {event_name}", file=sys.stderr)
    sys.exit(1)


def is_dependabot_pr(pr_number: int) -> bool:
    """Check if a PR is from Dependabot."""
    token = get_github_token()
    result = run_gh_command(["pr", "view", str(pr_number), "--json", "author"], token)
    pr_data = json.loads(result)
    author = pr_data.get("author", {}).get("login", "")
    return author == "dependabot[bot]"


def extract_metadata_from_title(pr_number: int) -> Tuple[str, str]:
    """Extract dependency names and update type from PR title."""
    token = get_github_token()
    result = run_gh_command(["pr", "view", str(pr_number), "--json", "title"], token)
    pr_data = json.loads(result)
    title = pr_data.get("title", "")

    # Extract dependency names (format: "Bump X from Y to Z" or "Bump X and Y from ...")
    dep_match = re.match(r"^Bump (.+) from .+$", title)
    if dep_match:
        dependency_names = dep_match.group(1).replace(" and ", ", ")
    else:
        dependency_names = "unknown"

    # Determine update type from version changes
    version_pattern = r"from ([0-9]+)\.([0-9]+)\.([0-9]+) to ([0-9]+)\.([0-9]+)\.([0-9]+)"
    version_match = re.search(version_pattern, title)
    if version_match:
        old_major, old_minor = int(version_match.group(1)), int(version_match.group(2))
        new_major, new_minor = int(version_match.group(4)), int(version_match.group(5))

        if old_major != new_major:
            update_type = "version-update:semver-major"
        elif old_minor != new_minor:
            update_type = "version-update:semver-minor"
        else:
            update_type = "version-update:semver-patch"
    else:
        # Fallback: default to major (requires manual review)
        update_type = "version-update:semver-major"

    return dependency_names, update_type


def check_mergeability(pr_number: int) -> Tuple[bool, str, bool]:
    """Check if PR is mergeable and ready to merge."""
    token = get_github_token()
    result = run_gh_command(
        ["pr", "view", str(pr_number), "--json", "mergeable,mergeableState"],
        token,
    )
    pr_data = json.loads(result)
    mergeable = pr_data.get("mergeable", False)
    mergeable_state = pr_data.get("mergeableState", "unknown")

    ready = mergeable and mergeable_state == "clean"
    return mergeable, mergeable_state, ready


def merge_pr(pr_url: str, dependency_names: str, update_type: str) -> None:
    """Merge a PR using squash merge with auto-merge enabled."""
    token = get_github_token()
    print(f"🚀 Auto-merging {dependency_names} ({update_type})")
    run_gh_command(["pr", "merge", pr_url, "--squash", "--auto"], token)


def comment_on_major_update(pr_url: str, dependency_names: str) -> None:
    """Add a comment to a major version update PR."""
    token = get_github_token()
    body = f"""⚠️ **Major version update** - requires manual review before merging.

This PR updates **{dependency_names}** to a new major version.

Please:
1. Review the changelog for breaking changes
2. Run tests locally: `cargo test`
3. Run performance tests: `hey -n 10000 -c 50 http://127.0.0.1:9999/health`
4. Merge manually if all looks good"""
    print(f"⚠️ Major update detected for {dependency_names} - requires manual review")
    run_gh_command(["pr", "comment", pr_url, "--body", body], token)


def process_dependabot_pr() -> None:
    """Main entry point for processing a Dependabot PR."""
    # Extract PR information
    pr_number, pr_url = extract_pr_info()
    print(f"Extracted PR #{pr_number}: {pr_url}")

    # Check if PR is from Dependabot
    if not is_dependabot_pr(pr_number):
        print("⏭️ PR is not from Dependabot, skipping")
        sys.exit(0)

    print("✅ PR is from Dependabot")

    # Get metadata - use dependabot/fetch-metadata action output if available
    # Otherwise extract from PR title
    event_name = get_event_name()
    if event_name == "pull_request":
        # For pull_request events, we rely on the dependabot/fetch-metadata action
        # This tool is called after that action runs
        dependency_names = os.getenv("DEPENDENCY_NAMES", "").strip()
        update_type = os.getenv("UPDATE_TYPE", "").strip()
        # If metadata not provided, extract from PR title as fallback
        if not dependency_names or not update_type:
            dependency_names, update_type = extract_metadata_from_title(pr_number)
            print(f"Extracted metadata from PR title: {dependency_names} ({update_type})")
        else:
            print(f"Using metadata from dependabot/fetch-metadata: {dependency_names} ({update_type})")
    else:
        # For check_suite/status events, extract from PR title
        dependency_names, update_type = extract_metadata_from_title(pr_number)
        print(f"Extracted metadata: {dependency_names} ({update_type})")

    # Handle major updates
    if update_type == "version-update:semver-major":
        comment_on_major_update(pr_url, dependency_names)
        sys.exit(0)

    # Check mergeability for minor/patch updates
    mergeable, mergeable_state, ready = check_mergeability(pr_number)
    print(f"PR status: mergeable={mergeable}, state={mergeable_state}")

    if not ready:
        print(f"⏳ PR not ready: mergeable={mergeable}, state={mergeable_state} (will retry on next status update)")
        sys.exit(0)

    # Merge the PR
    merge_pr(pr_url, dependency_names, update_type)
