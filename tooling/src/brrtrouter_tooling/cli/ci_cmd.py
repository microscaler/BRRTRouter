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
        project_root = Path.cwd()
        workspace_dir_name = "microservices"
        dry_run = "--dry-run" in args
        audit = "--audit" in args
        i = 0
        while i < len(args):
            if args[i] == "--project-root" and i + 1 < len(args):
                project_root = Path(args[i + 1]).resolve()
                i += 2
            elif args[i] == "--workspace-dir" and i + 1 < len(args):
                workspace_dir_name = args[i + 1]
                i += 2
            else:
                i += 1
        run_patch_brrtrouter(
            project_root,
            workspace_dir_name=workspace_dir_name,
            dry_run=dry_run,
            audit=audit,
        )
        sys.exit(0)

    if sub == "fix-cargo-paths":
        cargo_toml = None
        project_root = Path.cwd()
        i = 0
        while i < len(args):
            if args[i] == "--cargo-toml" and i + 1 < len(args):
                cargo_toml = Path(args[i + 1]).resolve()
                i += 2
            elif args[i] == "--project-root" and i + 1 < len(args):
                project_root = Path(args[i + 1]).resolve()
                i += 2
            else:
                i += 1
        if not cargo_toml:
            print("Error: --cargo-toml required", file=sys.stderr)
            sys.exit(1)
        rc = run_fix_cargo_paths(cargo_toml, project_root=project_root)
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
