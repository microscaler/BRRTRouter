"""Fix BRRTRouter path dependencies in generated Cargo.toml files."""

from __future__ import annotations

import os
import re
from pathlib import Path


def fix_cargo_toml(
    cargo_toml_path: Path,
    project_root: Path | None = None,
    brrtrouter_path: Path | None = None,
) -> bool:
    """
    Fix brrtrouter/brrtrouter_macros path deps in Cargo.toml to point at brrtrouter_path.

    project_root: repo root; if None, inferred from cargo_toml_path (e.g. 3 levels up from gen/).
    brrtrouter_path: path to BRRTRouter repo; if None, project_root.parent / "BRRTRouter".
    Returns True if content was changed.
    """
    if not cargo_toml_path.exists():
        print(f"Warning: {cargo_toml_path} does not exist, skipping")
        return False

    content = cargo_toml_path.read_text()
    original = content

    cargo_toml_dir = cargo_toml_path.parent.resolve()
    if project_root is not None:
        root = Path(project_root).resolve()
    else:
        if cargo_toml_dir.name == "gen":
            root = cargo_toml_dir.parent.parent.parent.parent
        else:
            root = cargo_toml_dir.parent.parent.parent

    brrt = brrtrouter_path if brrtrouter_path is not None else root.parent / "BRRTRouter"
    brrt = Path(brrt).resolve()
    try:
        rel = Path(os.path.relpath(brrt, cargo_toml_dir)).as_posix()
        rel_macros = Path(os.path.relpath(brrt / "brrtrouter_macros", cargo_toml_dir)).as_posix()
    except ValueError:
        rel = str(brrt)
        rel_macros = str(brrt / "brrtrouter_macros")

    content = re.sub(
        r'brrtrouter = \{ path = "[^"]+" \}',
        f'brrtrouter = {{ path = "{rel}" }}',
        content,
    )
    content = re.sub(
        r'brrtrouter_macros = \{ path = "[^"]+" \}',
        f'brrtrouter_macros = {{ path = "{rel_macros}" }}',
        content,
    )

    if content != original:
        cargo_toml_path.write_text(content)
        print(f"✅ Fixed paths in {cargo_toml_path}")
        return True
    print(f"Info:  No changes needed in {cargo_toml_path}")
    return False


def run(
    cargo_toml_path: Path, project_root: Path | None = None, brrtrouter_path: Path | None = None
) -> int:
    """Run fix for one Cargo.toml. Returns 0."""
    fix_cargo_toml(cargo_toml_path, project_root=project_root, brrtrouter_path=brrtrouter_path)
    return 0
