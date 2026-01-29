"""CLI for pre-commit: brrtrouter pre-commit workspace-fmt."""

from __future__ import annotations

import sys
from pathlib import Path

from brrtrouter_tooling.pre_commit import run_workspace_fmt


def run_pre_commit_argv() -> None:
    """Dispatch brrtrouter pre-commit <subcommand> [options]."""
    if len(sys.argv) < 3:
        print(
            "Usage: brrtrouter pre-commit <subcommand> [options]",
            file=sys.stderr,
        )
        print(
            "Subcommands: workspace-fmt",
            file=sys.stderr,
        )
        print(
            "Options: --project-root PATH, --workspace-dir DIR (default: microservices)",
            file=sys.stderr,
        )
        sys.exit(1)

    sub = sys.argv[2].lower()
    args = sys.argv[3:]

    project_root = Path.cwd()
    workspace_dir = "microservices"
    i = 0
    while i < len(args):
        if args[i] == "--project-root" and i + 1 < len(args):
            project_root = Path(args[i + 1]).resolve()
            i += 2
        elif args[i] == "--workspace-dir" and i + 1 < len(args):
            workspace_dir = args[i + 1]
            i += 2
        else:
            i += 1

    if sub == "workspace-fmt":
        code = run_workspace_fmt(
            project_root=project_root,
            workspace_dir=workspace_dir,
        )
        sys.exit(code)

    print(f"Error: Unknown pre-commit subcommand: {sub}", file=sys.stderr)
    sys.exit(1)
