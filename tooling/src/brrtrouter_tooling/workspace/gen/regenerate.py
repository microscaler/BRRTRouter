"""Regenerate services from OpenAPI specs (flattened layout)."""

from pathlib import Path

from brrtrouter_tooling.discovery import bff_service_to_suite
from brrtrouter_tooling.gen.brrtrouter import call_brrtrouter_generate
from brrtrouter_tooling.workspace.build.constants import get_package_names
from brrtrouter_tooling.workspace.ci.fix_cargo_paths import run as run_fix_cargo_paths
from brrtrouter_tooling.workspace.env_paths import discover_brrtrouter_root


def _fix_cargo_paths_callback(cargo_toml_path: Path, project_root: Path | None) -> None:
    run_fix_cargo_paths(cargo_toml_path, project_root=project_root)


def _gen_package_name(project_root: Path, service_name: str) -> str | None:
    """Gen crate [package].name for a service (e.g. hauliage_identity_gen)."""
    impl_name = get_package_names(project_root).get(service_name)
    return f"{impl_name}_gen" if impl_name else None


def regenerate_service(
    project_root: Path,
    suite: str,
    service_name: str,
    brrtrouter_path: Path | None = None,
) -> int:
    """Regenerate a single service. Returns 0 on success, 1 on error."""
    package_name = _gen_package_name(project_root, service_name)

    if brrtrouter_path is None:
        brrtrouter_path = discover_brrtrouter_root(project_root)

    is_bff = bff_service_to_suite(project_root, service_name) == suite

    if is_bff:
        spec_path = project_root / "openapi" / "openapi_bff.yaml"
        deps_config_path = project_root / "openapi" / "brrtrouter-dependencies.toml"
    else:
        spec_path = project_root / "openapi" / service_name / "openapi.yaml"
        deps_config_path = spec_path.parent / "brrtrouter-dependencies.toml"

    output_dir = project_root / "microservices" / service_name / "gen"

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
        if gen_cargo.exists():
            _fix_cargo_paths_callback(gen_cargo, project_root)

        return 0
    except FileNotFoundError as e:
        print(f"❌ {e}")
        return 1


def regenerate_suite_services(
    project_root: Path,
    suite: str,
    service_names: list[str],
    brrtrouter_path: Path | None = None,
) -> int:
    """Regenerate all services in a suite. Returns 0 if all succeed, 1 if any fail."""
    failed = []
    for service_name in service_names:
        rc = regenerate_service(
            project_root,
            suite,
            service_name,
            brrtrouter_path=brrtrouter_path,
        )
        if rc != 0:
            failed.append(service_name)

    if failed:
        print(f"\n❌ Failed to regenerate {len(failed)} service(s): {', '.join(failed)}")
        return 1

    print(f"\n✅ Successfully regenerated {len(service_names)} service(s) in suite '{suite}'")
    return 0
