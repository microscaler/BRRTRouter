"""CLI for bootstrap: brrtrouter bootstrap microservice <service>."""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Any

from brrtrouter_tooling.bootstrap import run_bootstrap_microservice
from brrtrouter_tooling.bootstrap.config import DEFAULT_BOOTSTRAP_LAYOUT


def _parse_layout_from_argv(args: list[str]) -> dict[str, Any]:
    """Parse layout overrides from argv (--openapi-dir, --workspace-dir, etc.)."""
    layout: dict[str, Any] = {}
    i = 0
    keys = set(DEFAULT_BOOTSTRAP_LAYOUT)
    while i < len(args):
        if args[i] == "--openapi-dir" and i + 1 < len(args) and "openapi_dir" in keys:
            layout["openapi_dir"] = args[i + 1]
            i += 2
        elif args[i] == "--suite" and i + 1 < len(args) and "suite" in keys:
            layout["suite"] = args[i + 1]
            i += 2
        elif args[i] == "--workspace-dir" and i + 1 < len(args) and "workspace_dir" in keys:
            layout["workspace_dir"] = args[i + 1]
            i += 2
        elif args[i] == "--docker-dir" and i + 1 < len(args) and "docker_dir" in keys:
            layout["docker_dir"] = args[i + 1]
            i += 2
        elif args[i] == "--tiltfile" and i + 1 < len(args) and "tiltfile" in keys:
            layout["tiltfile"] = args[i + 1]
            i += 2
        elif args[i] == "--port-registry" and i + 1 < len(args) and "port_registry" in keys:
            layout["port_registry"] = args[i + 1]
            i += 2
        elif args[i] == "--crate-name-prefix" and i + 1 < len(args) and "crate_name_prefix" in keys:
            layout["crate_name_prefix"] = args[i + 1]
            i += 2
        else:
            i += 1
    return layout


def run_bootstrap_argv() -> None:
    """Dispatch brrtrouter bootstrap microservice <service> [options]."""
    if len(sys.argv) < 4:
        print(
            "Usage: brrtrouter bootstrap microservice <service_name> [options]",
            file=sys.stderr,
        )
        print(
            "Options: --port N, --project-root PATH, --add-dependencies-config",
            file=sys.stderr,
        )
        print(
            "Layout: --openapi-dir, --suite, --workspace-dir, --docker-dir, --tiltfile, --port-registry, --crate-name-prefix",
            file=sys.stderr,
        )
        sys.exit(1)

    if sys.argv[2].lower() != "microservice":
        print(f"Error: Unknown bootstrap subcommand: {sys.argv[2]}", file=sys.stderr)
        sys.exit(1)

    service_name = sys.argv[3]
    args = sys.argv[4:]

    project_root = Path.cwd()
    port = None
    add_dependencies_config = "--add-dependencies-config" in args
    args = [a for a in args if a != "--add-dependencies-config"]

    i = 0
    while i < len(args):
        if args[i] == "--project-root" and i + 1 < len(args):
            project_root = Path(args[i + 1]).resolve()
            i += 2
        elif args[i] == "--port" and i + 1 < len(args):
            try:
                port = int(args[i + 1])
            except ValueError:
                print(f"Error: --port must be an integer: {args[i + 1]}", file=sys.stderr)
                sys.exit(1)
            i += 2
        else:
            i += 1

    layout = _parse_layout_from_argv(args) or None
    code = run_bootstrap_microservice(
        service_name=service_name,
        port=port,
        project_root=project_root,
        add_dependencies_config=add_dependencies_config,
        layout=layout,
    )
    sys.exit(code)
