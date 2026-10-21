"""`<cli> gen` — Regenerate gen crates and impl stubs from OpenAPI specs."""

import sys
from pathlib import Path

from brrtrouter_tooling.workspace.bootstrap.microservice import regenerate_impl_stubs
from brrtrouter_tooling.workspace.discovery import suite_sub_service_names
from brrtrouter_tooling.workspace.gen.regenerate import (
    existing_gen_services,
    regenerate_service,
    regenerate_suite_services,
)
from brrtrouter_tooling.workspace.profile import cli_name


def run_gen(args, project_root: Path) -> None:
    if args.gen_cmd == "stubs":
        suite = getattr(args, "suite", None)
        if not suite:
            print(f"{cli_name()} gen stubs: missing suite name")
            print(
                f"  Use: {cli_name()} gen stubs <suite-name> [--service <name>] [--force] [--sync]"
            )
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
            print(f"{cli_name()} gen suite: missing suite name")
            print(f"  Use: {cli_name()} gen suite <suite-name>")
            sys.exit(1)
        suite = args.suite

        # If --service is specified, regenerate only that service
        if hasattr(args, "service") and args.service:
            print(f"🔄 Regenerating {args.service} service in suite '{suite}'...")
            rc = regenerate_service(
                project_root, suite, args.service, allow_new=getattr(args, "new", False)
            )
            sys.exit(rc)

        # Otherwise regenerate all services in the suite
        # Only the suite's own services (those with a gen crate); a spec the BFF only proxies is skipped.
        services, proxied = existing_gen_services(
            project_root, suite, suite_sub_service_names(project_root, suite)
        )
        if proxied:
            print(
                f"Skipping specs with no gen crate in '{suite}' (proxied only): {', '.join(proxied)}"
            )
        if not services:
            print(f"⚠️  No services found for suite: {suite}")
            sys.exit(1)
        print(f"🔄 Regenerating {len(services)} services in suite '{suite}'...")
        rc = regenerate_suite_services(project_root, suite, services)
        sys.exit(rc)
    else:
        print(f"{cli_name()} gen {args.gen_cmd}: unknown subcommand")
        print(
            f"  Use: {cli_name()} gen suite <suite-name> or {cli_name()} gen stubs <suite-name> [--force] [--sync]"
        )
        sys.exit(1)
