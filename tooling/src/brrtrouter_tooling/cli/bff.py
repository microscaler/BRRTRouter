"""CLI for BFF spec generation: brrtrouter bff generate."""

from __future__ import annotations

import sys
from pathlib import Path

from brrtrouter_tooling.bff.generate import generate_bff_spec


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
