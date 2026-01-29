"""CLI for ports validate: brrtrouter ports validate."""

from __future__ import annotations

import sys
from pathlib import Path

from brrtrouter_tooling.cli.parse_common import parse_flags, path_resolver
from brrtrouter_tooling.ports import PortRegistry, validate
from brrtrouter_tooling.ports.layout import DEFAULT_LAYOUT


def run_ports_validate_argv() -> None:
    """Parse argv for brrtrouter ports validate and run."""
    args = sys.argv[3:]
    parsed, rest = parse_flags(
        args,
        ("project_root", "--project-root", Path.cwd, path_resolver),
        ("registry_path", "--registry", None, None),
    )
    for a in rest:
        if a != "--json":
            print(f"Error: Unknown argument: {a}", file=sys.stderr)
            print(
                "Usage: brrtrouter ports validate [--project-root <path>] [--registry <path>] [--json]",
                file=sys.stderr,
            )
            sys.exit(1)
    project_root = parsed["project_root"]
    registry_path = parsed["registry_path"]
    json_out = "--json" in rest

    if registry_path is None:
        registry_path = project_root / DEFAULT_LAYOUT["port_registry"]
    else:
        registry_path = Path(registry_path)
        if not registry_path.is_absolute():
            registry_path = project_root / registry_path

    registry = PortRegistry(registry_path, project_root)
    exit_code = validate(registry, project_root, json_out=json_out)
    sys.exit(exit_code)
