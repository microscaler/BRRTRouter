"""CLI for openapi: brrtrouter openapi validate | fix-operation-id-casing | check-decimal-formats | fix-impl-controllers."""

from __future__ import annotations

import sys
from pathlib import Path

from brrtrouter_tooling.cli.parse_common import parse_flags, path_resolver, project_root_resolver
from brrtrouter_tooling.openapi import (
    check_openapi_dir,
    fix_impl_controllers_dir,
    fix_operation_id_run,
    validate_specs,
)


def _parse_openapi_argv() -> tuple[Path, Path]:
    """Parse --project-root and --openapi-dir from sys.argv. Returns (project_root, openapi_dir)."""
    args = sys.argv[3:]
    parsed, _ = parse_flags(
        args,
        ("project_root", "--project-root", Path.cwd, project_root_resolver),
        ("openapi_dir", "--openapi-dir", None, path_resolver),
    )
    project_root = parsed["project_root"]
    openapi_dir = parsed["openapi_dir"] or (project_root / "openapi")
    return project_root, openapi_dir


def run_openapi_validate_argv() -> None:
    """brrtrouter openapi validate [--project-root <path>] [--openapi-dir <path>]."""
    _project_root, openapi_dir = _parse_openapi_argv()
    errors = validate_specs(openapi_dir)
    for path, exc in errors:
        print(f"❌ {path}: {exc}")
    if errors:
        print(f"\n❌ Found {len(errors)} invalid OpenAPI specs")
        sys.exit(1)
    if openapi_dir.exists():
        count = len(list(openapi_dir.rglob("openapi.yaml")))
        if count > 0:
            print(f"\n✅ All {count} OpenAPI specs are valid")
        else:
            print("\n✅ No openapi.yaml found; nothing to validate.")
    else:
        print("\n✅ openapi/ directory not found; nothing to validate.")
    sys.exit(0)


def run_openapi_fix_operation_id_argv() -> None:
    """brrtrouter openapi fix-operation-id-casing [--project-root <path>] [--openapi-dir <path>] [--dry-run] [--verbose]."""
    args = sys.argv[3:]
    project_root, openapi_dir = _parse_openapi_argv()
    dry_run = "--dry-run" in args
    verbose = "--verbose" in args
    total, touched = fix_operation_id_run(
        openapi_dir, dry_run=dry_run, verbose=verbose, rel_to=project_root
    )
    if touched:
        prefix = "[DRY-RUN] " if dry_run else ""
        print(f"{prefix}Updated {touched} file(s), {total} operationId(s) converted to snake_case.")
    else:
        print("No operationId casing changes needed.")
    sys.exit(0)


def run_openapi_check_decimal_formats_argv() -> None:
    """brrtrouter openapi check-decimal-formats [--project-root <path>] [--openapi-dir <path>]."""
    project_root, openapi_dir = _parse_openapi_argv()
    if not openapi_dir.exists():
        print(f"❌ OpenAPI directory not found: {openapi_dir}", file=sys.stderr)
        sys.exit(1)
    files_with_issues = check_openapi_dir(openapi_dir)
    if not files_with_issues:
        print("✅ All OpenAPI specs have format: decimal or format: money for number fields")
        sys.exit(0)
    total_issues = sum(len(issues) for _, issues in files_with_issues)
    print(
        f"❌ Found {total_issues} number fields without format in {len(files_with_issues)} file(s):\n",
        file=sys.stderr,
    )
    for spec_path, issues in files_with_issues:
        try:
            rel_path = spec_path.relative_to(project_root)
        except ValueError:
            rel_path = spec_path
        print(f"  {rel_path}: {len(issues)} issue(s)")
        for issue in issues[:5]:
            print(f"    - {issue}")
        if len(issues) > 5:
            print(f"    ... and {len(issues) - 5} more")
        print()
    sys.exit(1)


def run_openapi_fix_impl_controllers_argv() -> None:
    """brrtrouter openapi fix-impl-controllers [--project-root <path>] [--impl-dir <path>]."""
    args = sys.argv[3:]
    parsed, _ = parse_flags(
        args,
        ("project_root", "--project-root", Path.cwd, project_root_resolver),
        ("impl_dir", "--impl-dir", None, path_resolver),
    )
    project_root = parsed["project_root"]
    impl_dir = parsed["impl_dir"] or (project_root / "microservices" / "accounting")
    if not impl_dir.exists():
        print(f"❌ Impl directory not found: {impl_dir}", file=sys.stderr)
        sys.exit(1)
    files_fixed = fix_impl_controllers_dir(impl_dir)
    if files_fixed:
        total_fixes = sum(f for _, f in files_fixed)
        print(f"✅ Fixed {total_fixes} f64 literals in {len(files_fixed)} file(s):\n")
        for file_path, fixes in files_fixed:
            try:
                rel_path = file_path.relative_to(project_root)
            except ValueError:
                rel_path = file_path
            print(f"  {rel_path}: {fixes} fix(es)")
        sys.exit(0)
    print("✅ No f64 literals found in impl controllers (or already fixed)")
    sys.exit(0)


def run_openapi_argv() -> None:
    """Dispatch brrtrouter openapi <subcommand>."""
    if len(sys.argv) < 3:
        print("Usage: brrtrouter openapi <subcommand> [options]", file=sys.stderr)
        print(
            "Subcommands: validate, fix-operation-id-casing, check-decimal-formats, fix-impl-controllers",
            file=sys.stderr,
        )
        sys.exit(1)
    sub = sys.argv[2].lower()
    if sub == "validate":
        run_openapi_validate_argv()
    elif sub == "fix-operation-id-casing":
        run_openapi_fix_operation_id_argv()
    elif sub == "check-decimal-formats":
        run_openapi_check_decimal_formats_argv()
    elif sub == "fix-impl-controllers":
        run_openapi_fix_impl_controllers_argv()
    else:
        print(f"Error: Unknown openapi subcommand: {sub}", file=sys.stderr)
        sys.exit(1)
