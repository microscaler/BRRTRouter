"""`hauliage gen` — Regenerate gen crates and impl stubs from OpenAPI specs."""

import sys
from pathlib import Path

from brrtrouter_tooling.workspace.bootstrap.microservice import regenerate_impl_stubs
from brrtrouter_tooling.workspace.discovery import suite_sub_service_names
from brrtrouter_tooling.workspace.gen.regenerate import (
    regenerate_service,
    regenerate_suite_services,
)


def run_gen(args, project_root: Path) -> None:
    if args.gen_cmd == "stubs":
        suite = getattr(args, "suite", None)
        if not suite:
            print("hauliage gen stubs: missing suite name")
            print("  Use: hauliage gen stubs <suite-name> [--service <name>] [--force] [--sync]")
            sys.exit(1)
        service = getattr(args, "service_flag", None) or getattr(args, "service_positional", None)
        force = getattr(args, "force", False)
        sync = getattr(args, "sync", False)
        if service:
            print(f"🔄 Regenerating impl stubs for {service} (suite {suite})...")
        else:
            print(f"🔄 Regenerating impl stubs for suite '{suite}'...")
        rc = regenerate_impl_stubs(project_root, suite, service=service, force=force, sync=sync)
        sys.exit(rc)
    if args.gen_cmd == "suite":
        if not getattr(args, "suite", None):
            print("hauliage gen suite: missing suite name")
            print("  Use: hauliage gen suite <suite-name>")
            sys.exit(1)
        suite = args.suite

        # If --service is specified, regenerate only that service
        if hasattr(args, "service") and args.service:
            print(f"🔄 Regenerating {args.service} service in suite '{suite}'...")
            rc = regenerate_service(project_root, suite, args.service)
            sys.exit(rc)

        # Otherwise regenerate all services in the suite
        services = suite_sub_service_names(project_root, suite)
        if not services:
            print(f"⚠️  No services found for suite: {suite}")
            sys.exit(1)
        print(f"🔄 Regenerating {len(services)} services in suite '{suite}'...")
        rc = regenerate_suite_services(project_root, suite, services)
        sys.exit(rc)
    else:
        print(f"hauliage gen {args.gen_cmd}: unknown subcommand")
        print(
            "  Use: hauliage gen suite <suite-name> or hauliage gen stubs <suite-name> [--force] [--sync]"
        )
        sys.exit(1)
