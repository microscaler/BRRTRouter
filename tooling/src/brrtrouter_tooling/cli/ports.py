"""CLI for ports validate: brrtrouter ports validate."""

from __future__ import annotations

import sys
from pathlib import Path

from brrtrouter_tooling.cli.parse_common import parse_flags, path_resolver
from brrtrouter_tooling.ports import PortRegistry, fix_duplicates, reconcile, validate
from brrtrouter_tooling.ports.layout import DEFAULT_LAYOUT


def run_ports_validate_argv(args: list[str]) -> None:
    """Parse argv for brrtrouter ports validate and run."""
    parsed, rest = parse_flags(
        args,
        ("project_root", "--project-root", Path.cwd, path_resolver),
        ("registry_path", "--registry", None, None),
    )
    for a in rest:
        if a != "--json":
            print(f"Error: Unknown argument: {a}", file=sys.stderr)
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


def run_ports_reconcile_argv(args: list[str]) -> None:
    """Parse argv for brrtrouter ports reconcile and run."""
    parsed, rest = parse_flags(
        args,
        ("project_root", "--project-root", Path.cwd, path_resolver),
        ("registry_path", "--registry", None, None),
    )
    update_configs = "--update-configs" in rest
    for a in rest:
        if a != "--update-configs":
            print(f"Error: Unknown argument: {a}", file=sys.stderr)
            sys.exit(1)
    project_root = parsed["project_root"]
    registry_path = parsed["registry_path"]

    if registry_path is None:
        registry_path = project_root / DEFAULT_LAYOUT["port_registry"]
    else:
        registry_path = Path(registry_path)
        if not registry_path.is_absolute():
            registry_path = project_root / registry_path

    registry = PortRegistry(registry_path, project_root)
    exit_code = reconcile(registry, project_root, update_configs=update_configs)
    sys.exit(exit_code)


def run_ports_fix_duplicates_argv(args: list[str]) -> None:
    """Parse argv for brrtrouter ports fix-duplicates and run."""
    parsed, rest = parse_flags(
        args,
        ("project_root", "--project-root", Path.cwd, path_resolver),
        ("registry_path", "--registry", None, None),
    )
    dry_run = "--dry-run" in rest
    for a in rest:
        if a != "--dry-run":
            print(f"Error: Unknown argument: {a}", file=sys.stderr)
            sys.exit(1)
    project_root = parsed["project_root"]
    registry_path = parsed["registry_path"]

    if registry_path is None:
        registry_path = project_root / DEFAULT_LAYOUT["port_registry"]
    else:
        registry_path = Path(registry_path)
        if not registry_path.is_absolute():
            registry_path = project_root / registry_path

    registry = PortRegistry(registry_path, project_root)
    exit_code = fix_duplicates(registry, project_root, dry_run=dry_run)
    sys.exit(exit_code)


def run_ports_argv() -> None:
    args = sys.argv[2:]
    if len(args) == 0:
        print("Usage: brrtrouter client ports <validate|reconcile|fix-duplicates>", file=sys.stderr)
        sys.exit(1)
    subcommand = args[0]
    if subcommand == "validate":
        run_ports_validate_argv(args[1:])
    elif subcommand == "reconcile":
        run_ports_reconcile_argv(args[1:])
    elif subcommand == "fix-duplicates":
        run_ports_fix_duplicates_argv(args[1:])
    else:
        print(f"Error: Unknown ports subcommand: {subcommand}", file=sys.stderr)
        sys.exit(1)
