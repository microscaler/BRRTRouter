"""Build workspace or a single package with optional gen-if-missing callback.

Uses host_aware (ARCH_TARGETS, cargo/cross/zigbuild). Configurable workspace_dir,
package list (caller passes package name), and optional gen_if_missing_callback(project_root).

For arm7 (armv7-unknown-linux-musleabihf), --no-default-features is passed because
jemalloc does not support armv7; packages that use jemalloc as a default feature
would otherwise fail to build.
"""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Callable

from brrtrouter_tooling.build.host_aware import (
    ARCH_TARGETS,
    _build_service,
    _build_workspace,
    should_use_cross,
    should_use_zigbuild,
)


def build_workspace_with_options(
    project_root: Path,
    workspace_dir: str = "microservices",
    arch: str = "amd64",
    release: bool = True,
    gen_if_missing_callback: Callable[[Path], None] | None = None,
) -> int:
    """Build workspace at project_root/workspace_dir. arch: amd64|arm64|arm7. Returns 0/1."""
    manifest = project_root / workspace_dir / "Cargo.toml"
    if not manifest.exists():
        if gen_if_missing_callback is not None:
            gen_if_missing_callback(project_root)
        if not manifest.exists():
            print(f"❌ {manifest} not found", file=sys.stderr)
            return 1

    rust_target = ARCH_TARGETS.get(arch, ARCH_TARGETS["amd64"])
    use_cross = should_use_cross()
    use_zigbuild = should_use_zigbuild()
    rel = ["--release"] if release else []
    # jemalloc does not support armv7; disable default features on arm7 to avoid build failures.
    no_jemalloc = rust_target == "armv7-unknown-linux-musleabihf"
    base = ["--no-default-features"] if no_jemalloc else []
    extra_args = base + rel

    ok = _build_workspace(
        project_root,
        workspace_dir,
        rust_target,
        arch,
        use_zigbuild,
        use_cross,
        extra_args,
    )
    return 0 if ok else 1


def build_package_with_options(
    project_root: Path,
    workspace_dir: str = "microservices",
    package_name: str = "",
    arch: str = "amd64",
    release: bool = True,
    gen_if_missing_callback: Callable[[Path], None] | None = None,
) -> int:
    """Build one package (-p package_name) in workspace. Returns 0/1."""
    if not package_name:
        print("❌ package_name required", file=sys.stderr)
        return 1
    manifest = project_root / workspace_dir / "Cargo.toml"
    if not manifest.exists():
        if gen_if_missing_callback is not None:
            gen_if_missing_callback(project_root)
        if not manifest.exists():
            print(f"❌ {manifest} not found", file=sys.stderr)
            return 1

    rust_target = ARCH_TARGETS.get(arch, ARCH_TARGETS["amd64"])
    use_cross = should_use_cross()
    use_zigbuild = should_use_zigbuild()
    rel = ["--release"] if release else []
    # jemalloc does not support armv7; disable default features on arm7 to avoid build failures.
    no_jemalloc = rust_target == "armv7-unknown-linux-musleabihf"
    base = ["--no-default-features"] if no_jemalloc else []
    extra_args = base + rel

    ok = _build_service(
        project_root,
        workspace_dir,
        package_name,
        rust_target,
        arch,
        use_zigbuild,
        use_cross,
        extra_args,
    )
    return 0 if ok else 1
