"""Validate version to prevent downgrades."""

import os
import sys

from brrtrouter_tooling.helpers import compare_versions


def validate_version(current: str, latest: str | None, allow_same: bool = False) -> int:
    """Validate current version is greater than latest (or equal if allow_same). Raises SystemExit if invalid."""
    if latest is None:
        return 0

    try:
        cmp_val = compare_versions(current, latest)
    except ValueError as e:
        msg = "Version comparison error: " + str(e)
        raise SystemExit(msg) from e

    if cmp_val > 0:
        return 0

    if cmp_val == 0:
        if allow_same:
            return 0
        print(
            f"Version {current} is not greater than latest release {latest}. Use --allow-same to allow same version.",
            file=sys.stderr,
        )
        raise SystemExit(1)

    print(
        f"Version downgrade detected: current={current}, latest={latest}. Cannot release a version lower than the latest.",
        file=sys.stderr,
    )
    raise SystemExit(1)


def run_validate_version_cli(
    current: str | None,
    latest: str | None = None,
    allow_same: bool = False,
) -> int:
    """CLI helper for validate-version. Fetches latest from GitHub if not provided."""
    from brrtrouter_tooling.ci.get_latest_tag import get_latest_tag

    if not current:
        print("Error: --current required", file=sys.stderr)
        return 1

    if not latest:
        repo = os.environ.get("GITHUB_REPOSITORY", "")
        token = os.environ.get("GITHUB_TOKEN", "")

        if repo and token:
            latest = get_latest_tag(repo, token)
        else:
            print(
                "Error: --latest required or set GITHUB_REPOSITORY and GITHUB_TOKEN",
                file=sys.stderr,
            )
            return 1

    try:
        validate_version(current, latest, allow_same=allow_same)
        return 0
    except SystemExit as e:
        code = getattr(e, "code", None)
        if code is None and e.args and isinstance(e.args[0], int):
            code = e.args[0]
        return code if code is not None else 1


def run() -> int:
    """CLI entry point for validate-version (standalone)."""
    import argparse

    parser = argparse.ArgumentParser(description="Validate version to prevent downgrades")
    parser.add_argument("--current", required=True, help="Current version to validate")
    parser.add_argument("--latest", help="Latest version from GitHub")
    parser.add_argument("--allow-same", action="store_true", help="Allow same version")

    args = parser.parse_args()
    return run_validate_version_cli(
        current=args.current,
        latest=args.latest,
        allow_same=args.allow_same,
    )
