"""Build microservices workspace (host_aware for jemalloc opt-in; brrtrouter_tooling for single-package)."""

from __future__ import annotations

import sys
from pathlib import Path

from brrtrouter_tooling.build import build_package_with_options
from brrtrouter_tooling.workspace.build.constants import get_package_names
from brrtrouter_tooling.workspace.build.host_aware import run as run_host_aware
from brrtrouter_tooling.workspace.discovery import suite_sub_service_names
from brrtrouter_tooling.workspace.gen.regenerate import regenerate_service
from brrtrouter_tooling.workspace.legacy_env import deprecated_alias
from brrtrouter_tooling.workspace.profile import load_profile


def gen_if_missing(project_root: Path) -> int:
    """Generate the gen crates the workspace lists but that are not on disk (e.g. a fresh clone). Returns 0, or 1 when a
    generation failed (the caller must not build on it).

    Every suite with services is considered, not only those with a BFF (``all_suites``). Only a suite's own services
    get a crate: one the workspace manifest's members name (``service_gen_dir``). A spec the BFF only proxies is never
    given a crate. Without a workspace manifest, every discovered service whose gen crate is missing is generated.
    """
    from brrtrouter_tooling.workspace.discovery.suites import (
        all_suites,
        resolve_service_microservice_dir,
    )
    from brrtrouter_tooling.workspace.gen.regenerate import service_gen_dir, workspace_members

    profile = load_profile(project_root)
    members = workspace_members(project_root)
    missing: list[tuple[str, str, Path]] = []
    for suite in all_suites(project_root):
        for name in suite_sub_service_names(project_root, suite):
            if members is None:
                gen = resolve_service_microservice_dir(project_root, suite, name) / "gen"
            else:
                gen = service_gen_dir(project_root, suite, name, members=members)
                if gen is None:
                    continue  # proxied only: not one of the workspace's crates
            if (gen / "Cargo.toml").exists():
                continue
            missing.append((suite, name, gen))
    if not missing:
        return 0
    print(
        f"📦 {len(missing)} gen crate(s) missing in {profile.workspace_dir}/; running brrtrouter-gen natively...",
        file=sys.stderr,
    )
    failed = [
        f"{suite}/{name}"
        for suite, name, gen in missing
        if regenerate_service(project_root, suite, name, allow_new=True, output_dir=gen) != 0
    ]
    if failed:
        print(f"❌ codegen failed for {', '.join(failed)}; not building", file=sys.stderr)
        return 1
    print("✅ codegen complete")
    return 0


run_hauliage_gen_if_missing = deprecated_alias(gen_if_missing, "run_hauliage_gen_if_missing")  # legacy-name


def build_microservices_workspace(project_root: Path, arch: str, release: bool) -> int:
    """Build microservices/ workspace. arch: amd64|arm64|arm7. Uses jemalloc for amd64/arm64 (CI opt-in). Returns 0/1."""
    if gen_if_missing(project_root) != 0:
        return 1
    return run_host_aware(
        target="workspace",
        arch=arch,
        extra_args=None,
        project_root=project_root,
        release=release,
    )


def build_microservice(project_root: Path, name: str, release: bool) -> int:
    """Build one microservice. name e.g. identity. Returns 0/1.

    Package names are resolved SUITE-scoped: a service name duplicated in a
    non-suite openapi tree (e.g. a draft-tier sketch) must not shadow the real
    crate name with the fallback prefix.
    """
    from brrtrouter_tooling.workspace.discovery.suites import service_to_suite

    suite = service_to_suite(project_root, name)
    package_names = get_package_names(project_root, suite=suite)
    pkg = package_names.get(name)
    if not pkg:
        print(
            f"❌ unknown service: {name}. Valid: {', '.join(package_names)}",
            file=sys.stderr,
        )
        return 1
    host_arch = run_host_aware.__globals__.get('detect_host_architecture', lambda: "amd64")()

    return build_package_with_options(
        project_root,
        workspace_dir=load_profile(project_root).workspace_dir,
        package_name=pkg,
        arch=host_arch,
        release=release,
        gen_if_missing_callback=gen_if_missing,
    )
