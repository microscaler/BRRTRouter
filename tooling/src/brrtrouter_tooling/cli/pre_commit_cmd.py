"""CLI for pre-commit: brrtrouter pre-commit workspace-fmt."""

from __future__ import annotations

import sys
from pathlib import Path

from brrtrouter_tooling.cli.parse_common import parse_flags, project_root_resolver
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

    parsed, _ = parse_flags(
        args,
        ("project_root", "--project-root", Path.cwd, project_root_resolver),
        ("workspace_dir", "--workspace-dir", lambda: "microservices", None),
    )
    project_root = parsed["project_root"]
    workspace_dir = parsed["workspace_dir"]

    if sub == "workspace-fmt":
        code = run_workspace_fmt(
            project_root=project_root,
            workspace_dir=workspace_dir,
        )
        sys.exit(code)

    print(f"Error: Unknown pre-commit subcommand: {sub}", file=sys.stderr)
    sys.exit(1)
