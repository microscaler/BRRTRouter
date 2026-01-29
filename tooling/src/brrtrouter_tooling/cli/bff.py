"""CLI for BFF spec generation: brrtrouter bff generate, brrtrouter bff generate-system."""

from __future__ import annotations

import sys
from pathlib import Path

from brrtrouter_tooling.bff import (
    discover_sub_services,
    generate_bff_spec,
    generate_system_bff_spec,
    list_systems_with_sub_services,
)


def run_bff_generate() -> None:
    """Run brrtrouter bff generate --suite-config <path> [--output <path>] [--validate] [--base-dir <path>]."""
    # argv: [script, bff, generate, --suite-config, path, ...] -> skip first 3
    args = sys.argv[3:]
    suite_config = None
    output = None
    base_dir = None
    validate = False

    i = 0
    while i < len(args):
        if args[i] == "--suite-config" and i + 1 < len(args):
            suite_config = Path(args[i + 1])
            i += 2
        elif args[i] == "--output" and i + 1 < len(args):
            output = Path(args[i + 1])
            i += 2
        elif args[i] == "--base-dir" and i + 1 < len(args):
            base_dir = Path(args[i + 1])
            i += 2
        elif args[i] == "--validate":
            validate = True
            i += 1
        else:
            print(f"Error: Unknown argument: {args[i]}", file=sys.stderr)
            print(
                "Usage: brrtrouter bff generate --suite-config <path> [--output <path>] [--base-dir <path>] [--validate]",
                file=sys.stderr,
            )
            sys.exit(1)

    if not suite_config:
        print("Error: --suite-config <path> is required", file=sys.stderr)
        print(
            "Usage: brrtrouter bff generate --suite-config <path> [--output <path>] [--base-dir <path>] [--validate]",
            file=sys.stderr,
        )
        sys.exit(1)
    if not suite_config.exists():
        print(f"Error: Suite config file not found: {suite_config}", file=sys.stderr)
        sys.exit(1)

    try:
        out_path = generate_bff_spec(
            suite_config_path=suite_config,
            output_path=output,
            base_dir=base_dir,
            validate=validate,
        )
        print(f"Generated BFF spec: {out_path}")
    except (ValueError, OSError, FileNotFoundError) as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)


def run_bff_generate_system_argv() -> None:
    """Parse argv for brrtrouter bff generate-system and run."""
    args = sys.argv[3:]
    openapi_dir: Path = Path.cwd() / "openapi"
    system: str | None = None
    output: str | None = None

    i = 0
    while i < len(args):
        if args[i] == "--openapi-dir" and i + 1 < len(args):
            openapi_dir = Path(args[i + 1]).resolve()
            i += 2
        elif args[i] == "--system" and i + 1 < len(args):
            system = args[i + 1]
            i += 2
        elif args[i] == "--output" and i + 1 < len(args):
            output = args[i + 1]
            i += 2
        else:
            print(f"Error: Unknown argument: {args[i]}", file=sys.stderr)
            print(
                "Usage: brrtrouter bff generate-system [--openapi-dir <path>] [--system <name>] [--output <path>]",
                file=sys.stderr,
            )
            sys.exit(1)

    if not openapi_dir.exists():
        print(f"Error: openapi dir not found: {openapi_dir}", file=sys.stderr)
        sys.exit(1)

    run_bff_generate_system(openapi_dir, system, output)


def run_bff_generate_system(openapi_dir: Path, system: str | None, output: str | None) -> None:
    """Run brrtrouter bff generate-system: directory discovery then generate."""
    if system:
        out_path = Path(output) if output else None
        subs = discover_sub_services(openapi_dir, system)
        if not subs:
            print(f"No sub-services found for {system}", file=sys.stderr)
            sys.exit(0)
        print(f"Generating {system} system BFF OpenAPI specification ({len(subs)} sub-services)...")
        generate_system_bff_spec(openapi_dir, system, output_path=out_path)
        out = Path(output) if output else (openapi_dir / system / "openapi.yaml")
        print(f"Generated {system} BFF spec: {out}")
    else:
        systems = list_systems_with_sub_services(openapi_dir)
        if not systems:
            print("No systems with sub-services found", file=sys.stderr)
            sys.exit(0)
        print(f"Generating system BFF specs for all systems ({len(systems)} with sub-services)...")
        for s in systems:
            generate_system_bff_spec(openapi_dir, s, output_path=None)
            print(f"  {s} -> openapi/{s}/openapi.yaml")
