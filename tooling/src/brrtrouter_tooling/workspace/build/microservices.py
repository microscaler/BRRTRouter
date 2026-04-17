"""Build microservices workspace (host_aware for jemalloc opt-in; brrtrouter_tooling for single-package)."""

from __future__ import annotations

import sys
from pathlib import Path

from brrtrouter_tooling.build import build_package_with_options
from brrtrouter_tooling.workspace.build.constants import get_package_names
from brrtrouter_tooling.workspace.build.host_aware import run as run_host_aware
from brrtrouter_tooling.workspace.discovery import suite_sub_service_names
from brrtrouter_tooling.workspace.gen.regenerate import regenerate_service


def run_hauliage_gen_if_missing(project_root: Path) -> None:
    """Generate gen crates for hauliage natively if missing."""
    service_names = list(suite_sub_service_names(project_root, "hauliage"))
    if not service_names:
        return

    workspace_dir = "microservices"
    # In flat structure, probe microservices/{service}/gen/Cargo.toml
    probe = project_root / workspace_dir / service_names[0] / "gen" / "Cargo.toml"
    if probe.exists():
        return

    print(
        f"📦 {workspace_dir} crates missing; running brrtrouter-gen natively for all services...",
        file=sys.stderr,
    )
    for name in service_names:
        regenerate_service(project_root, "hauliage", name)
    print("✅ codegen complete")


def build_microservices_workspace(project_root: Path, arch: str, release: bool) -> int:
    """Build microservices/ workspace. arch: amd64|arm64|arm7. Uses jemalloc for amd64/arm64 (CI opt-in). Returns 0/1."""
    run_hauliage_gen_if_missing(project_root)
    return run_host_aware(
        target="workspace",
        arch=arch,
        extra_args=None,
        project_root=project_root,
        release=release,
    )


def build_microservice(project_root: Path, name: str, release: bool) -> int:
    """Build one hauliage microservice. name e.g. identity. Returns 0/1."""
    package_names = get_package_names(project_root)
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
        workspace_dir="microservices",
        package_name=pkg,
        arch=host_arch,
        release=release,
        gen_if_missing_callback=run_hauliage_gen_if_missing,
    )
