"""CLI for gen: brrtrouter gen generate | generate-stubs."""

from __future__ import annotations

import sys
from pathlib import Path

from brrtrouter_tooling.cli.parse_common import parse_flags, path_resolver
from brrtrouter_tooling.gen import (
    call_brrtrouter_generate,
    call_brrtrouter_generate_stubs,
)
from brrtrouter_tooling.helpers import default_gen_package_name


def _parse_common_args(args: list[str]) -> tuple[Path, Path | None, Path | None]:
    """Parse --project-root, --brrtrouter-path, and --openapi-dir from args."""
    parsed, _ = parse_flags(
        args,
        ("project_root", "--project-root", Path.cwd, path_resolver),
        ("brrtrouter_path", "--brrtrouter-path", None, path_resolver),
        ("openapi_dir", "--openapi-dir", None, path_resolver),
    )
    return parsed["project_root"], parsed["brrtrouter_path"], parsed["openapi_dir"]


def _fix_cargo_paths_cb(cargo_toml_path: Path, project_root: Path | None) -> None:
    from brrtrouter_tooling.ci.fix_cargo_paths import run as fix_cargo_paths_run

    fix_cargo_paths_run(cargo_toml_path, project_root=project_root)


def _default_package_name(suite: str, sn: str) -> str:
    """Gen crate package name; ``sn`` is the on-disk service folder (may be kebab-case)."""
    return default_gen_package_name(sn)


def _run_gen_suite_argv(args: list[str]) -> None:
    from brrtrouter_tooling.discovery import suite_sub_service_names
    from brrtrouter_tooling.gen.regenerate import (
        regenerate_service,
        regenerate_suite_services,
    )

    if len(args) == 0 or args[0].startswith("-"):
        print(
            "Usage: brrtrouter client gen suite <suite-name> [--service <name>]",
            file=sys.stderr,
        )
        sys.exit(1)

    suite = args[0]
    args_tail = args[1:]

    service = None
    project_root, brrtrouter_path, openapi_dir = _parse_common_args(args_tail)

    i = 0
    while i < len(args_tail):
        if args_tail[i] == "--service" and i + 1 < len(args_tail):
            service = args_tail[i + 1]
            i += 2
        else:
            i += 1

    if service:
        print(f"🔄 Regenerating {service} service in suite '{suite}'...")
        rc = regenerate_service(
            project_root,
            suite,
            service,
            brrtrouter_path=brrtrouter_path,
            fix_cargo_paths_fn=_fix_cargo_paths_cb,
            package_name=_default_package_name(suite, service),
            openapi_dir=openapi_dir,
        )
        sys.exit(rc)

    services = suite_sub_service_names(project_root, suite)
    if not services:
        print(f"⚠️  No services found for suite: {suite}", file=sys.stderr)
        sys.exit(1)

    print(f"🔄 Regenerating {len(services)} services in suite '{suite}'...")
    rc = regenerate_suite_services(
        project_root,
        suite,
        services,
        brrtrouter_path=brrtrouter_path,
        fix_cargo_paths_fn=_fix_cargo_paths_cb,
        package_name_for_service=_default_package_name,
        openapi_dir=openapi_dir,
    )
    sys.exit(rc)


def _run_gen_stubs_argv(args: list[str]) -> None:
    import re

    from brrtrouter_tooling.discovery import bff_service_to_suite

    if len(args) == 0 or args[0].startswith("-"):
        print(
            "Usage: brrtrouter client gen stubs <suite-name> [<service-name>] [--force] [--sync]",
            file=sys.stderr,
        )
        sys.exit(1)

    suite = args[0]
    args_tail = args[1:]

    # Check if second arg is the service name (positional)
    service = None
    if len(args_tail) > 0 and not args_tail[0].startswith("-"):
        service = args_tail[0]
        args_tail = args_tail[1:]

    force = "--force" in args_tail
    sync = "--sync" in args_tail
    project_root, brrtrouter_path, openapi_dir = _parse_common_args(args_tail)

    # Backup logic in case they used --service
    i = 0
    while i < len(args_tail):
        if args_tail[i] == "--service" and i + 1 < len(args_tail):
            service = args_tail[i + 1]
            i += 2
        else:
            i += 1

    if not service:
        print(
            "Error: missing service name. Use: brrtrouter client gen stubs <suite> <service>",
            file=sys.stderr,
        )
        sys.exit(1)

    print(f"🔄 Regenerating impl stubs for {service} (suite {suite})...")

    is_bff = bff_service_to_suite(project_root, service) == suite
    base_openapi = openapi_dir if openapi_dir is not None else project_root / "openapi"

    if is_bff:
        spec_path = base_openapi / suite / "openapi_bff.yaml"
    else:
        spec_path = base_openapi / suite / service / "openapi.yaml"

    if not spec_path.exists():
        print(f"❌ OpenAPI spec not found: {spec_path}", file=sys.stderr)
        sys.exit(1)

    crate_dir = project_root / "microservices" / suite / service
    impl_dir = crate_dir / "impl"
    gen_cargo = crate_dir / "gen" / "Cargo.toml"

    # read component_name from gen Cargo.toml
    component_name = "unknown_gen"
    if gen_cargo.exists():
        text = gen_cargo.read_text()
        in_package = False
        for line in text.splitlines():
            s = line.strip()
            if s.startswith("["):
                in_package = s.strip("[]").strip() == "package"
                continue
            if in_package:
                m = re.match(r'name\s*=\s*"([^"]+)"', line)
                if m:
                    component_name = m.group(1)
                    break

    result = call_brrtrouter_generate_stubs(
        spec_path=spec_path,
        impl_dir=impl_dir,
        component_name=component_name,
        project_root=project_root,
        brrtrouter_path=brrtrouter_path,
        force=force,
        sync=sync,
        capture_output=False,
    )
    sys.exit(result.returncode)


def run_gen_argv() -> None:
    """Dispatch brrtrouter gen <subcommand>."""
    if len(sys.argv) < 3:
        print(
            "Usage: brrtrouter gen <subcommand> [options]",
            file=sys.stderr,
        )
        print(
            "Subcommands: generate, generate-stubs, suite, stubs",
            file=sys.stderr,
        )
        sys.exit(1)

    sub = sys.argv[2].lower()
    args = sys.argv[3:]

    if sub == "generate":
        spec = None
        output = None
        deps_config = None
        package_name = None
        project_root, brrtrouter_path, _ = _parse_common_args(args)
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
            elif args[i] == "--package-name" and i + 1 < len(args):
                package_name = args[i + 1]
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
            package_name=package_name,
            capture_output=False,
        )
        sys.exit(result.returncode)

    if sub == "generate-stubs":
        spec = None
        output = None
        component_name = None
        force = "--force" in args
        sync = "--sync" in args
        project_root, brrtrouter_path, _ = _parse_common_args(args)
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
            sync=sync,
            capture_output=False,
        )
        sys.exit(result.returncode)

    if sub == "suite":
        _run_gen_suite_argv(args)

    if sub == "stubs":
        _run_gen_stubs_argv(args)

    print(f"Error: Unknown gen subcommand: {sub}", file=sys.stderr)
    sys.exit(1)
