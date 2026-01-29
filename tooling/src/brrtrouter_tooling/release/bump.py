"""Bump version in all Cargo.toml [package] and [workspace.package] sections.

Source of truth: workspace_cargo_toml (e.g. microservices/Cargo.toml [workspace.package].version).
Walks the repo from project_root for every Cargo.toml; excludes paths containing: target, .git,
.venv, venv, env, __pycache__, node_modules, node_packages, build, dist, tmp.
Accepts version = \"v0.1.0\" or \"0.1.0\" when reading; always writes \"X.Y.Z\" (no \"v\") to Cargo.toml.
"""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

from brrtrouter_tooling.helpers import find_cargo_tomls

VERSION_SECTIONS = ("package", "workspace.package")

SKIP_PARTS = frozenset(
    {
        "target",
        ".git",
        ".venv",
        "venv",
        "env",
        "__pycache__",
        "node_modules",
        "node_packages",
        "build",
        "dist",
        "tmp",
    }
)


def _read_current(workspace_toml: Path) -> str:
    """Read version from workspace Cargo.toml [workspace.package].version."""
    text = workspace_toml.read_text()
    in_sec = False
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("["):
            in_sec = s.strip("[]").strip() == "workspace.package"
            continue
        if in_sec:
            m = re.match(r'^\s*version\s*=\s*"v?(\d+\.\d+\.\d+(?:-[\w.-]+)?)"', line)
            if m:
                return m.group(1)
    msg = f"Could not find [workspace.package].version in {workspace_toml}"
    raise SystemExit(msg)


def _next_version(old: str, bump: str) -> str:
    """Compute next version. Bump: patch, minor, major, rc, release (promote). Returns X.Y.Z or X.Y.Z-rc.N."""
    old = old.lstrip("v")
    m = re.match(r"^(\d+)\.(\d+)\.(\d+)(?:-([\w.-]+))?$", old)
    if not m:
        msg = f"Invalid version in workspace Cargo.toml: {old}"
        raise SystemExit(msg)
    x, y, z = int(m.group(1)), int(m.group(2)), int(m.group(3))
    prerel = m.group(4)
    b = (bump or "patch").lower()

    if prerel and b in ("patch", "minor", "major"):
        msg = f"Cannot {b} bump prerelease {old}; use release/promote or rc."
        raise SystemExit(msg)

    if b == "rc":
        if not prerel:
            return f"{x}.{y}.{z}-rc.1"
        m2 = re.match(r"^rc\.(\d+)$", prerel)
        if not m2:
            msg = f"rc bump only supports -rc.N prerelease; found -{prerel}"
            raise SystemExit(msg)
        return f"{x}.{y}.{z}-rc.{int(m2.group(1)) + 1}"

    if b in ("release", "promote"):
        if not prerel:
            msg = f"Already a full release ({old}). Use patch, minor, or major to create a new version."
            raise SystemExit(msg)
        return f"{x}.{y}.{z}"

    if b == "patch":
        z += 1
    elif b == "minor":
        y += 1
        z = 0
    elif b == "major":
        x += 1
        y = z = 0
    else:
        msg = f"Unknown bump: {bump}. Use patch, minor, major, rc, or release."
        raise SystemExit(msg)
    return f"{x}.{y}.{z}"


def _replace_in_file(path: Path, old: str, new: str) -> bool:
    """Replace version in [package] or [workspace.package]. Returns True if changed."""
    text = path.read_text()
    lines = text.splitlines(keepends=True)
    out: list[str] = []
    in_sec = False
    replaced = False
    for line in lines:
        s = line.strip()
        if s.startswith("["):
            in_sec = s.strip("[]").strip() in VERSION_SECTIONS
            out.append(line)
            continue
        if in_sec:
            pat = r'(\s*version\s*=")v?' + re.escape(old) + r'"'
            if re.search(pat, line):
                new_line = re.sub(pat, lambda m: m.group(1) + new + '"', line, count=1)
                out.append(new_line)
                replaced = True
                continue
        out.append(line)
    if replaced:
        path.write_text("".join(out))
    return replaced


def _set_workspace_package_version(path: Path, new: str) -> bool:
    """Set [workspace.package].version to new. Returns True only if changed."""
    text = path.read_text()
    lines = text.splitlines(keepends=True)
    out: list[str] = []
    in_sec = False
    changed = False
    for line in lines:
        s = line.strip()
        if s.startswith("["):
            in_sec = s.strip("[]").strip() == "workspace.package"
            out.append(line)
            continue
        if in_sec:
            m = re.match(r'^(\s*version\s*=\s*")([^"]*)(")', line)
            if m:
                if m.group(2) != new:
                    out.append(m.group(1) + new + m.group(3) + line[m.end() :])
                    changed = True
                else:
                    out.append(line)
                continue
        out.append(line)
    if changed:
        path.write_text("".join(out))
    return changed


def run(
    project_root: Path,
    bump: str,
    workspace_cargo_toml: str | Path = "microservices/Cargo.toml",
) -> int:
    """Bump version: read from workspace Cargo.toml, walk all Cargo.toml, replace. Returns 0 or 1."""
    workspace_toml = project_root / workspace_cargo_toml
    if not workspace_toml.is_file():
        print(f"{workspace_toml} not found", file=sys.stderr)
        return 1

    old = _read_current(workspace_toml)
    new = _next_version(old, bump)

    updated: list[Path] = []
    for p in find_cargo_tomls(project_root, exclude=SKIP_PARTS):
        try:
            if _replace_in_file(p, old, new):
                updated.append(p.relative_to(project_root))
        except (OSError, ValueError) as e:
            print(f"Error updating {p}: {e}", file=sys.stderr)
            return 1

    root_cargo = project_root / "Cargo.toml"
    if root_cargo.is_file():
        try:
            if _set_workspace_package_version(root_cargo, new):
                rel = root_cargo.relative_to(project_root)
                if rel not in updated:
                    updated.append(rel)
        except (OSError, ValueError) as e:
            print(f"Error updating root {root_cargo}: {e}", file=sys.stderr)
            return 1

    if not updated:
        print(
            f"No Cargo.toml had [package]/[workspace.package].version = {old!r}",
            file=sys.stderr,
        )
        return 1

    print(f"Bumped {old} -> {new} ({bump}); updated {len(updated)} file(s)")
    for u in updated:
        print(f"  {u}")

    go = os.environ.get("GITHUB_OUTPUT")
    if go:
        with Path(go).open("a") as f:
            f.write(f"version={new}\n")

    return 0
