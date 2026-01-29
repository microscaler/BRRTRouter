"""CLI for ports validate: brrtrouter ports validate."""

from __future__ import annotations

import sys
from pathlib import Path

from brrtrouter_tooling.ports import PortRegistry, validate
from brrtrouter_tooling.ports.layout import DEFAULT_LAYOUT


def run_ports_validate_argv() -> None:
    """Parse argv for brrtrouter ports validate and run."""
    args = sys.argv[3:]
    project_root = Path.cwd()
    registry_path: Path | None = None
    json_out = False

    i = 0
    while i < len(args):
        if args[i] == "--project-root" and i + 1 < len(args):
            project_root = Path(args[i + 1]).resolve()
            i += 2
        elif args[i] == "--registry" and i + 1 < len(args):
            registry_path = Path(args[i + 1]).resolve()
            i += 2
        elif args[i] == "--json":
            json_out = True
            i += 1
        else:
            print(f"Error: Unknown argument: {args[i]}", file=sys.stderr)
            print(
                "Usage: brrtrouter ports validate [--project-root <path>] [--registry <path>] [--json]",
                file=sys.stderr,
            )
            sys.exit(1)

    if registry_path is None:
        registry_path = project_root / DEFAULT_LAYOUT["port_registry"]

    registry = PortRegistry(registry_path, project_root)
    exit_code = validate(registry, project_root, json_out=json_out)
    sys.exit(exit_code)
