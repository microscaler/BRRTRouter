"""Build workspace or a single package with optional gen-if-missing callback.

Uses host_aware (ARCH_TARGETS, cargo/cross/zigbuild). Configurable workspace_dir,
package list (caller passes package name), and optional gen_if_missing_callback(project_root).
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path
from typing import Callable

from brrtrouter_tooling.build.host_aware import (
    ARCH_TARGETS,
    _get_cargo_env,
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
        print(f"❌ {manifest} not found", file=sys.stderr)
        return 1
    if gen_if_missing_callback is not None:
        gen_if_missing_callback(project_root)

    rust_target = ARCH_TARGETS.get(arch, ARCH_TARGETS["amd64"])
    use_cross = should_use_cross()
    use_zigbuild = should_use_zigbuild()
    rel = ["--release"] if release else []
    no_jemalloc = rust_target == "armv7-unknown-linux-musleabihf"
    base = ["--no-default-features"] if no_jemalloc else []

    try:
        if use_cross:
            cmd = (
                [
                    "cross",
                    "build",
                    "--manifest-path",
                    str(manifest),
                    "--target",
                    rust_target,
                    "--workspace",
                ]
                + base
                + rel
            )
            subprocess.run(cmd, check=True, cwd=str(project_root))
        elif use_zigbuild:
            cmd = (
                [
                    "cargo",
                    "zigbuild",
                    "--manifest-path",
                    str(manifest),
                    "--target",
                    rust_target,
                    "--workspace",
                ]
                + base
                + rel
            )
            subprocess.run(cmd, check=True, cwd=str(project_root))
        else:
            cmd = (
                [
                    "cargo",
                    "build",
                    "--manifest-path",
                    str(manifest),
                    "--target",
                    rust_target,
                    "--workspace",
                ]
                + base
                + rel
            )
            env = {**os.environ, **_get_cargo_env(rust_target)}
            subprocess.run(cmd, check=True, cwd=str(project_root), env=env)
        return 0
    except subprocess.CalledProcessError:
        return 1


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
        print(f"❌ {manifest} not found", file=sys.stderr)
        return 1
    if gen_if_missing_callback is not None:
        gen_if_missing_callback(project_root)

    rust_target = ARCH_TARGETS.get(arch, ARCH_TARGETS["amd64"])
    use_cross = should_use_cross()
    use_zigbuild = should_use_zigbuild()
    rel = ["--release"] if release else []

    try:
        if use_cross:
            cmd = [
                "cross",
                "build",
                "--manifest-path",
                str(manifest),
                "--target",
                rust_target,
                "-p",
                package_name,
            ] + rel
            subprocess.run(cmd, check=True, cwd=str(project_root))
        elif use_zigbuild:
            cmd = [
                "cargo",
                "zigbuild",
                "--manifest-path",
                str(manifest),
                "--target",
                rust_target,
                "-p",
                package_name,
            ] + rel
            subprocess.run(cmd, check=True, cwd=str(project_root))
        else:
            cmd = [
                "cargo",
                "build",
                "--manifest-path",
                str(manifest),
                "--target",
                rust_target,
                "-p",
                package_name,
            ] + rel
            env = {**os.environ, **_get_cargo_env(rust_target)}
            subprocess.run(cmd, check=True, cwd=str(project_root), env=env)
        return 0
    except subprocess.CalledProcessError:
        return 1
