"""CLI for gen: brrtrouter gen generate | generate-stubs."""

from __future__ import annotations

import sys
from pathlib import Path

from brrtrouter_tooling.gen import (
    call_brrtrouter_generate,
    call_brrtrouter_generate_stubs,
)


def _parse_common_args(args: list[str]) -> tuple[Path, Path | None]:
    """Parse --project-root and --brrtrouter-path from args. Returns (project_root, brrtrouter_path)."""
    project_root = Path.cwd()
    brrtrouter_path = None
    i = 0
    while i < len(args):
        if args[i] == "--project-root" and i + 1 < len(args):
            project_root = Path(args[i + 1]).resolve()
            i += 2
        elif args[i] == "--brrtrouter-path" and i + 1 < len(args):
            brrtrouter_path = Path(args[i + 1]).resolve()
            i += 2
        else:
            i += 1
    return project_root, brrtrouter_path


def run_gen_argv() -> None:
    """Dispatch brrtrouter gen <subcommand>."""
    if len(sys.argv) < 3:
        print(
            "Usage: brrtrouter gen <subcommand> [options]",
            file=sys.stderr,
        )
        print(
            "Subcommands: generate, generate-stubs",
            file=sys.stderr,
        )
        sys.exit(1)

    sub = sys.argv[2].lower()
    args = sys.argv[3:]

    if sub == "generate":
        spec = None
        output = None
        deps_config = None
        project_root, brrtrouter_path = _parse_common_args(args)
        i = 0
        while i < len(args):
            if args[i] == "--spec" and i + 1 < len(args):
                spec = Path(args[i + 1]).resolve()
                i += 2
            elif args[i] == "--output" and i + 1 < len(args):
                output = Path(args[i + 1]).resolve()
                i += 2
            elif args[i] == "--dependencies-config" and i + 1 < len(args):
                deps_config = Path(args[i + 1]).resolve()
                i += 2
            else:
                i += 1
        if not spec or not output:
            print("Error: --spec and --output required", file=sys.stderr)
            sys.exit(1)
        result = call_brrtrouter_generate(
            spec_path=spec,
            output_dir=output,
            project_root=project_root,
            brrtrouter_path=brrtrouter_path,
            deps_config_path=deps_config,
            capture_output=False,
        )
        sys.exit(result.returncode)

    if sub == "generate-stubs":
        spec = None
        output = None
        component_name = None
        force = "--force" in args
        project_root, brrtrouter_path = _parse_common_args(args)
        i = 0
        while i < len(args):
            if args[i] == "--spec" and i + 1 < len(args):
                spec = Path(args[i + 1]).resolve()
                i += 2
            elif args[i] == "--output" and i + 1 < len(args):
                output = Path(args[i + 1]).resolve()
                i += 2
            elif args[i] == "--component-name" and i + 1 < len(args):
                component_name = args[i + 1]
                i += 2
            else:
                i += 1
        if not spec or not output or not component_name:
            print("Error: --spec, --output, and --component-name required", file=sys.stderr)
            sys.exit(1)
        result = call_brrtrouter_generate_stubs(
            spec_path=spec,
            impl_dir=output,
            component_name=component_name,
            project_root=project_root,
            brrtrouter_path=brrtrouter_path,
            force=force,
            capture_output=False,
        )
        sys.exit(result.returncode)

    print(f"Error: Unknown gen subcommand: {sub}", file=sys.stderr)
    sys.exit(1)
