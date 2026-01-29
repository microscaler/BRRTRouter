"""Extract binaries-*.zip from Multi-Arch job into workspace/target."""

from __future__ import annotations

import sys
import zipfile
from pathlib import Path


def run(
    input_dir: Path,
    project_root: Path,
    workspace_dir: str = "microservices",
    zip_prefix: str = "rerp-binaries-",
) -> int:
    """Extract {zip_prefix}{arch}.zip from input_dir into project_root/{workspace_dir}/target. Returns 0 or 1."""
    if not input_dir.is_dir():
        print(f"❌ Input directory not found: {input_dir}", file=sys.stderr)
        return 1

    dest = project_root / workspace_dir / "target"
    dest.mkdir(parents=True, exist_ok=True)

    zips = [
        input_dir / f"{zip_prefix}amd64.zip",
        input_dir / f"{zip_prefix}arm64.zip",
        input_dir / f"{zip_prefix}arm7.zip",
    ]
    found = [z for z in zips if z.exists()]
    if not found:
        print(
            f"❌ No {zip_prefix}*.zip in {input_dir}. Expected: {zip_prefix}amd64.zip, etc.",
            file=sys.stderr,
        )
        return 1

    for z in found:
        count = 0
        with zipfile.ZipFile(z, "r") as zh:
            for name in zh.namelist():
                if "/" not in name or name.startswith("/") or ".." in name:
                    continue
                zh.extract(name, dest)
                count += 1
                if name.endswith("_impl") and not name.endswith(".d"):
                    (dest / name).chmod(0o755)
        print(f"📦 Extracted {z.name} -> {dest} ({count} entries)")

    print(f"✅ Unpacked into {dest.relative_to(project_root)}")
    return 0
