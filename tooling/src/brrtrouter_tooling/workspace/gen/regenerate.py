"""Regenerate services from OpenAPI specs (flat or nested suite layout)."""

import sys
from pathlib import Path

from brrtrouter_tooling.gen.brrtrouter import call_brrtrouter_generate
from brrtrouter_tooling.workspace.build.constants import get_package_names
from brrtrouter_tooling.workspace.ci.fix_cargo_paths import run as run_fix_cargo_paths
from brrtrouter_tooling.workspace.discovery.suites import (
    resolve_service_microservice_dir,
    resolve_service_openapi_spec_path,
)
from brrtrouter_tooling.workspace.env_paths import discover_brrtrouter_root
from brrtrouter_tooling.workspace.profile import load_profile

if sys.version_info >= (3, 11):
    import tomllib
else:  # pragma: no cover - exercised on 3.10 only
    import tomli as tomllib


def workspace_members(project_root: Path) -> set[Path] | None:
    """Crate directories listed in the workspace manifest's ``[workspace] members`` (globs expanded), resolved.

    The manifest is ``<layout.workspace_dir>/Cargo.toml``. None when there is no manifest or it lists no members:
    callers then fall back to "the gen directory exists".
    """
    manifest = load_profile(project_root).workspace_path / "Cargo.toml"
    if not manifest.is_file():
        return None
    try:
        data = tomllib.loads(manifest.read_text())
    except (OSError, tomllib.TOMLDecodeError):
        return None
    members = (data.get("workspace") or {}).get("members") or []
    if not members:
        return None
    base = manifest.parent
    out: set[Path] = set()
    for m in members:
        if any(c in m for c in "*?["):
            out.update(p.resolve() for p in base.glob(m))
        else:
            out.add((base / m).resolve())
    return out


def is_service_gen_crate(
    project_root: Path, suite: str, service_name: str, members: set[Path] | None = None
) -> bool:
    """True when the service's gen crate is one of the workspace's own crates.

    A spec under ``openapi/{suite}/`` is not always a service of that suite (a BFF merges specs it only proxies),
    and a gen directory that is not a workspace member is a stray (nothing builds it). With no workspace manifest,
    an existing gen directory counts.
    """
    gen = resolve_service_microservice_dir(project_root, suite, service_name) / "gen"
    if members is None:
        members = workspace_members(project_root)
    if members is None:
        return gen.is_dir()
    return gen.resolve() in members


def _fix_cargo_paths_callback(cargo_toml_path: Path, project_root: Path | None) -> None:
    run_fix_cargo_paths(cargo_toml_path, project_root=project_root)


def _gen_package_name(
    project_root: Path, service_name: str, suite: str | None = None
) -> str | None:
    """Gen crate [package].name for a service (e.g. acme_identity_gen).

    Scope discovery to the SUITE: with suite=None a service name that also
    exists in another openapi/ tree (e.g. a draft-tier sketch without a
    bff-suite-config) shadows the real one and picks up the fallback prefix.
    """
    impl_name = get_package_names(project_root, suite=suite).get(service_name)
    return f"{impl_name}_gen" if impl_name else None


def regenerate_service(
    project_root: Path,
    suite: str,
    service_name: str,
    brrtrouter_path: Path | None = None,
    allow_new: bool = False,
) -> int:
    """Regenerate a single service. Returns 0 on success, 1 on error.

    Only a service that already has a gen crate is regenerated. A spec under ``openapi/{suite}/`` is not always a
    service of that suite: a BFF merges specs it only proxies (another suite's service, an external API), and
    generating those writes a stray crate. Creating a new service's first gen crate is explicit: ``allow_new``.
    """
    output_dir = resolve_service_microservice_dir(project_root, suite, service_name) / "gen"
    if not allow_new and not is_service_gen_crate(project_root, suite, service_name):
        why = (
            "it is not a workspace member (a stray crate)"
            if output_dir.is_dir()
            else "its spec may only be proxied by the BFF"
        )
        print(
            f"❌ No gen crate for '{service_name}' in suite '{suite}' at {output_dir}: {why}. "
            "Pass --new to create its first gen crate."
        )
        return 1
    package_name = _gen_package_name(project_root, service_name, suite=suite)

    if brrtrouter_path is None:
        brrtrouter_path = discover_brrtrouter_root(project_root)

    spec_path = resolve_service_openapi_spec_path(project_root, suite, service_name)
    deps_config_path = spec_path.parent / "brrtrouter-dependencies.toml"

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


def existing_gen_services(
    project_root: Path, suite: str, service_names: list[str]
) -> tuple[list[str], list[str]]:
    """Split a suite's spec names into (its own services, specs it only proxies).

    A service is one whose gen crate is a workspace member (``is_service_gen_crate``).
    """
    members = workspace_members(project_root)
    have, proxied = [], []
    for name in service_names:
        own = is_service_gen_crate(project_root, suite, name, members=members)
        (have if own else proxied).append(name)
    return have, proxied


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
