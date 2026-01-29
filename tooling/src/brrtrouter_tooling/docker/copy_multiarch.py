"""Copy component binaries for all or one arch to build_artifacts/{system}_{module}/{arch}."""

from __future__ import annotations

import hashlib
import sys
from collections.abc import Callable
from pathlib import Path

from brrtrouter_tooling.build.host_aware import ARCH_TARGETS
from brrtrouter_tooling.helpers import default_binary_name


def run(
    system: str,
    module: str,
    arch: str,
    project_root: Path,
    workspace_dir: str = "microservices",
    binary_name_fn: Callable[[str, str], str] | None = None,
) -> int:
    """Copy from workspace/target/{triple}/release/ to build_artifacts/{system}_{module}/{arch}. Returns 0 or 1."""
    if arch == "all":
        archs = ["amd64", "arm64", "arm7"]
    elif arch in ARCH_TARGETS:
        archs = [arch]
    else:
        print(
            f"❌ Unknown architecture: {arch}. Use amd64, arm64, arm7, or all.",
            file=sys.stderr,
        )
        return 1

    fn = binary_name_fn or default_binary_name
    root = project_root
    binary_name = fn(system, module)
    microservices_target = root / workspace_dir / "target"
    base_dest = root / "build_artifacts" / f"{system}_{module}"
    any_ok = False

    for a in archs:
        triple = ARCH_TARGETS[a]
        src = microservices_target / triple / "release" / binary_name
        dest_dir = base_dest / a
        dest_bin = dest_dir / binary_name
        hash_path = dest_dir / f"{binary_name}.sha256"

        if not src.exists():
            print(f"❌ Binary not found: {src}", file=sys.stderr)
            print(f"   Build first for {a}", file=sys.stderr)
            continue

        dest_dir.mkdir(parents=True, exist_ok=True)
        dest_bin.write_bytes(src.read_bytes())
        dest_bin.chmod(0o755)
        hash_path.write_text(hashlib.sha256(dest_bin.read_bytes()).hexdigest())
        print(f"✅ {a} binary copied and hash generated: {hash_path.relative_to(root)}")
        any_ok = True

    if not any_ok:
        return 1
    print("🎉 All requested binaries copied!")
    return 0
