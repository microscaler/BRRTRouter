"""Regenerate services from OpenAPI specs: default paths (openapi/{suite}/{service}/openapi.yaml, microservices/{suite}/{service}/gen) with optional fix_cargo_paths callback. Also run_gen_if_missing_for_suite for build gen-if-missing."""

from __future__ import annotations

import sys
from collections.abc import Callable
from pathlib import Path

from brrtrouter_tooling.discovery import bff_service_to_suite
from brrtrouter_tooling.gen.brrtrouter import call_brrtrouter_generate


def regenerate_service(
    project_root: Path,
    suite: str,
    service_name: str,
    brrtrouter_path: Path | None = None,
    fix_cargo_paths_fn: Callable[[Path, Path | None], None] | None = None,
    package_name: str | None = None,
) -> int:
    """Regenerate a single service from its OpenAPI spec.

    Default paths: openapi/{suite}/{service}/openapi.yaml (or openapi/{suite}/openapi_bff.yaml
    for BFF), output microservices/{suite}/{service}/gen. If fix_cargo_paths_fn is provided,
    it is called with (gen_cargo_toml_path, project_root) after generation.
    If package_name is provided, it is passed to brrtrouter-gen for [package].name in gen Cargo.toml.
    Returns 0 on success, 1 on error.
    """
    is_bff = bff_service_to_suite(project_root, service_name) == suite

    if is_bff:
        spec_path = project_root / "openapi" / suite / "openapi_bff.yaml"
        deps_config_path = project_root / "openapi" / suite / "brrtrouter-dependencies.toml"
    else:
        spec_path = project_root / "openapi" / suite / service_name / "openapi.yaml"
        deps_config_path = spec_path.parent / "brrtrouter-dependencies.toml"

    output_dir = project_root / "microservices" / suite / service_name / "gen"

    if not spec_path.exists():
        print(f"❌ OpenAPI spec not found: {spec_path}")
        return 1

    try:
        result = call_brrtrouter_generate(
            spec_path=spec_path,
            output_dir=output_dir,
            project_root=project_root,
            brrtrouter_path=brrtrouter_path,
            deps_config_path=deps_config_path if deps_config_path.exists() else None,
            package_name=package_name,
            capture_output=False,
        )

        if result.returncode != 0:
            print(f"❌ Failed to regenerate {service_name}")
            return 1

        print(f"✅ Regenerated {service_name}")

        gen_cargo = output_dir / "Cargo.toml"
        if gen_cargo.exists() and fix_cargo_paths_fn is not None:
            fix_cargo_paths_fn(gen_cargo, project_root)

        return 0
    except FileNotFoundError as e:
        print(f"❌ {e}")
        return 1


def regenerate_suite_services(
    project_root: Path,
    suite: str,
    service_names: list[str],
    brrtrouter_path: Path | None = None,
    fix_cargo_paths_fn: Callable[[Path, Path | None], None] | None = None,
    package_name_for_service: Callable[[str, str], str | None] | None = None,
) -> int:
    """Regenerate all services in a suite. Returns 0 if all succeed, 1 if any fail.

    If package_name_for_service(suite, service_name) is provided, it is used for each
    service's gen Cargo.toml [package].name.
    """
    failed = []
    for service_name in service_names:
        pkg = package_name_for_service(suite, service_name) if package_name_for_service else None
        rc = regenerate_service(
            project_root,
            suite,
            service_name,
            brrtrouter_path=brrtrouter_path,
            fix_cargo_paths_fn=fix_cargo_paths_fn,
            package_name=pkg,
        )
        if rc != 0:
            failed.append(service_name)

    if failed:
        print(f"\n❌ Failed to regenerate {len(failed)} service(s): {', '.join(failed)}")
        return 1

    print(f"\n✅ Successfully regenerated {len(service_names)} service(s) in suite '{suite}'")
    return 0


def run_gen_if_missing_for_suite(
    project_root: Path,
    suite: str,
    *,
    workspace_dir: str = "microservices",
    get_service_names_fn: Callable[[Path, str], list[str]],
    fix_cargo_paths_fn: Callable[[Path, Path | None], None] | None = None,
    package_name_for_service: Callable[[str, str], str | None] | None = None,
) -> None:
    """Generate gen crates for all services in suite if workspace/suite gen crates are missing.

    Used as gen_if_missing_callback for build_workspace_with_options. Probes
    workspace_dir/suite/<first_service>/gen/Cargo.toml; if missing, generates all services
    in the suite (same path logic as regenerate_service), then calls fix_cargo_paths_fn per gen.
    If package_name_for_service(suite, service_name) is provided, it is used for each
    service's gen Cargo.toml [package].name.
    """
    service_names = get_service_names_fn(project_root, suite)
    if not service_names:
        return
    probe = project_root / workspace_dir / suite / service_names[0] / "gen" / "Cargo.toml"
    if probe.exists():
        return

    print(
        f"📦 {workspace_dir}/{suite} crates missing; running brrtrouter-gen for all {suite} services...",
        file=sys.stderr,
    )
    for name in service_names:
        is_bff = bff_service_to_suite(project_root, name) == suite
        if is_bff:
            spec_path = project_root / "openapi" / suite / "openapi_bff.yaml"
            deps_config_path = project_root / "openapi" / suite / "brrtrouter-dependencies.toml"
        else:
            spec_path = project_root / "openapi" / suite / name / "openapi.yaml"
            deps_config_path = spec_path.parent / "brrtrouter-dependencies.toml"
        if not spec_path.exists():
            continue
        out = project_root / workspace_dir / suite / name / "gen"
        out.mkdir(parents=True, exist_ok=True)
        deps_config = deps_config_path if deps_config_path.exists() else None
        pkg = package_name_for_service(suite, name) if package_name_for_service else None
        result = call_brrtrouter_generate(
            spec_path=spec_path,
            output_dir=out,
            project_root=project_root,
            deps_config_path=deps_config,
            package_name=pkg,
            capture_output=True,
        )
        if result.returncode != 0:
            err = result.stderr or ""
            print(f"⚠️  Failed to generate {name}: {err}", file=sys.stderr)
            continue
        gen_cargo = out / "Cargo.toml"
        if gen_cargo.exists() and fix_cargo_paths_fn is not None:
            fix_cargo_paths_fn(gen_cargo, project_root)
    print("✅ codegen complete")
