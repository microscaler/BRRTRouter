"""Fix BRRTRouter path dependencies in generated Cargo.toml files.

Optional gen_crate_config: (name_pattern, version) applied when Cargo.toml is under
project_root/microservices/{suite}/{service}/gen/. name_pattern may use {suite} and {service}
(service is directory name with - replaced by _). Sets [package] name, version, and [lib] block.
"""

from __future__ import annotations

import os
import re
from pathlib import Path


def fix_cargo_toml(
    cargo_toml_path: Path,
    project_root: Path | None = None,
    brrtrouter_path: Path | None = None,
    gen_crate_config: tuple[str, str] | None = None,
) -> bool:
    """
    Fix brrtrouter/brrtrouter_macros path deps in Cargo.toml to point at brrtrouter_path.

    project_root: repo root; if None, inferred from cargo_toml_path (e.g. 3 levels up from gen/).
    brrtrouter_path: path to BRRTRouter repo; if None, project_root.parent / "BRRTRouter".
    gen_crate_config: optional (name_pattern, version) for gen crates under microservices/{suite}/{service}/gen.
      name_pattern may use {suite} and {service} (service in snake_case).
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

    # Optional gen crate name/version/[lib] for .../microservices/{suite}/{service}/gen/Cargo.toml
    if gen_crate_config is not None and cargo_toml_dir.name == "gen":
        try:
            service_dir = cargo_toml_dir.parent
            suite_dir = service_dir.parent
            if suite_dir.name and service_dir.name:
                suite = suite_dir.name
                service_snake = service_dir.name.replace("-", "_")
                name_pattern, version = gen_crate_config
                gen_crate_name = name_pattern.format(suite=suite, service=service_snake)
                if f'name = "{gen_crate_name}"' not in content:
                    content = re.sub(
                        r'name = "[^"]+"', f'name = "{gen_crate_name}"', content, count=1
                    )
                content = re.sub(r'version = "[^"]+"', f'version = "{version}"', content, count=1)
                if "[lib]" not in content:
                    content = re.sub(
                        r"(\[package\][^\[]+)",
                        r'\1\n[lib]\nname = "' + gen_crate_name + '"\npath = "src/lib.rs"\n',
                        content,
                        count=1,
                    )
        except (KeyError, ValueError):
            pass

    if content != original:
        cargo_toml_path.write_text(content)
        print(f"✅ Fixed paths in {cargo_toml_path}")
        return True
    print(f"Info:  No changes needed in {cargo_toml_path}")
    return False


def run(
    cargo_toml_path: Path,
    project_root: Path | None = None,
    brrtrouter_path: Path | None = None,
    gen_crate_config: tuple[str, str] | None = None,
) -> int:
    """Run fix for one Cargo.toml. Returns 0."""
    fix_cargo_toml(
        cargo_toml_path,
        project_root=project_root,
        brrtrouter_path=brrtrouter_path,
        gen_crate_config=gen_crate_config,
    )
    return 0
