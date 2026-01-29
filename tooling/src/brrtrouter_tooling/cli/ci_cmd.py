"""CLI for ci: brrtrouter ci patch-brrtrouter | fix-cargo-paths | is-tag | get-latest-tag | validate-version."""

from __future__ import annotations

import sys
from pathlib import Path

from brrtrouter_tooling.ci import (
    run_fix_cargo_paths,
    run_get_latest_tag,
    run_is_tag,
    run_patch_brrtrouter,
    run_validate_version_cli,
)
from brrtrouter_tooling.cli.parse_common import parse_flags, path_resolver, project_root_resolver


def run_ci_argv() -> None:
    """Dispatch brrtrouter ci <subcommand>."""
    if len(sys.argv) < 3:
        print(
            "Usage: brrtrouter ci <subcommand> [options]",
            file=sys.stderr,
        )
        print(
            "Subcommands: patch-brrtrouter, fix-cargo-paths, is-tag, get-latest-tag, validate-version",
            file=sys.stderr,
        )
        sys.exit(1)

    sub = sys.argv[2].lower()
    args = sys.argv[3:]

    if sub == "patch-brrtrouter":
        parsed, _ = parse_flags(
            args,
            ("project_root", "--project-root", Path.cwd, project_root_resolver),
            ("workspace_dir", "--workspace-dir", lambda: "microservices", None),
        )
        rc = run_patch_brrtrouter(
            parsed["project_root"],
            workspace_dir_name=parsed["workspace_dir"],
            dry_run="--dry-run" in args,
            audit="--audit" in args,
        )
        sys.exit(rc)

    if sub == "fix-cargo-paths":
        parsed, _ = parse_flags(
            args,
            ("project_root", "--project-root", Path.cwd, project_root_resolver),
            ("cargo_toml", "--cargo-toml", None, path_resolver),
        )
        if not parsed["cargo_toml"]:
            print("Error: --cargo-toml required", file=sys.stderr)
            sys.exit(1)
        rc = run_fix_cargo_paths(parsed["cargo_toml"], project_root=parsed["project_root"])
        sys.exit(rc)

    if sub == "is-tag":
        rc = run_is_tag()
        sys.exit(rc)

    if sub == "get-latest-tag":
        rc = run_get_latest_tag()
        sys.exit(rc)

    if sub == "validate-version":
        current = None
        latest = None
        allow_same = False
        i = 0
        while i < len(args):
            if args[i] == "--current" and i + 1 < len(args):
                current = args[i + 1]
                i += 2
            elif args[i] == "--latest" and i + 1 < len(args):
                latest = args[i + 1]
                i += 2
            elif args[i] == "--allow-same":
                allow_same = True
                i += 1
            else:
                i += 1
        rc = run_validate_version_cli(current=current, latest=latest, allow_same=allow_same)
        sys.exit(rc)

    print(f"Error: Unknown ci subcommand: {sub}", file=sys.stderr)
    sys.exit(1)
