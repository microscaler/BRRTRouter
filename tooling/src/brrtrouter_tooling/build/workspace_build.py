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
    _run_build,
    should_use_cross,
    should_use_zigbuild,
)


def _ensure_manifest_exists(
    project_root: Path,
    workspace_dir: str,
    gen_if_missing_callback: Callable[[Path], None] | None,
) -> bool:
    """Ensure workspace Cargo.toml exists; optionally run gen_if_missing_callback. Returns False on failure (prints to stderr)."""
    manifest = project_root / workspace_dir / "Cargo.toml"
    if not manifest.exists():
        if gen_if_missing_callback is not None:
            gen_if_missing_callback(project_root)
        if not manifest.exists():
            print(f"❌ {manifest} not found", file=sys.stderr)
            return False
    return True


def _resolve_build_options(arch: str, release: bool) -> tuple[str, bool, bool, list[str]]:
    """Resolve rust_target, use_cross, use_zigbuild, and extra_args (including armv7 jemalloc workaround)."""
    rust_target = ARCH_TARGETS.get(arch, ARCH_TARGETS["amd64"])
    use_cross = should_use_cross()
    use_zigbuild = should_use_zigbuild()
    rel = ["--release"] if release else []
    # jemalloc does not support armv7; disable default features on arm7 to avoid build failures.
    no_jemalloc = rust_target == "armv7-unknown-linux-musleabihf"
    base = ["--no-default-features"] if no_jemalloc else []
    extra_args = base + rel
    return rust_target, use_cross, use_zigbuild, extra_args


def build_workspace_with_options(
    project_root: Path,
    workspace_dir: str = "microservices",
    arch: str = "amd64",
    release: bool = True,
    gen_if_missing_callback: Callable[[Path], None] | None = None,
) -> int:
    """Build workspace at project_root/workspace_dir. arch: amd64|arm64|arm7. Returns 0/1."""
    if not _ensure_manifest_exists(project_root, workspace_dir, gen_if_missing_callback):
        return 1
    rust_target, use_cross, use_zigbuild, extra_args = _resolve_build_options(arch, release)
    ok = _run_build(
        project_root,
        workspace_dir,
        rust_target,
        arch,
        use_zigbuild,
        use_cross,
        extra_args,
        package_name=None,
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
    if not _ensure_manifest_exists(project_root, workspace_dir, gen_if_missing_callback):
        return 1
    rust_target, use_cross, use_zigbuild, extra_args = _resolve_build_options(arch, release)
    ok = _run_build(
        project_root,
        workspace_dir,
        rust_target,
        arch,
        use_zigbuild,
        use_cross,
        extra_args,
        package_name=package_name,
    )
    return 0 if ok else 1
