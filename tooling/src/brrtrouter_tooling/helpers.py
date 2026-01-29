"""Shared helpers for brrtrouter_tooling (text, spec load/validate, path, version, retry).

Used by bff, bootstrap, openapi, ci, docker, and other modules.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

import yaml

# --- Text ---


def to_pascal_case(name: str) -> str:
    """Convert kebab-case to PascalCase (e.g. my-service -> MyService)."""
    return "".join(word.capitalize() for word in name.split("-"))


def is_snake_case(s: str) -> bool:
    """Match BRRTRouter linter: non-empty, first is lower or '_', all lower/digit/underscore."""
    if not s:
        return False
    if s[0] != "_" and not s[0].islower():
        return False
    return all(c.islower() or c.isdigit() or c == "_" for c in s)


def to_snake_case(s: str) -> str:
    """Port of BRRTRouter src/linter.rs to_snake_case. Converts camelCase, kebab-case, spaces."""
    result: list[str] = []
    for ch in s:
        if ch.isupper():
            if result and result[-1] != "_":
                result.append("_")
            result.append(ch.lower())
        elif ch.islower() or ch.isdigit():
            result.append(ch)
        elif ch in "- ":
            if result and result[-1] != "_":
                result.append("_")
        else:
            result.append(ch)
    return "".join(result)


# --- Spec / file ---


def load_yaml_spec(p: Path) -> dict[str, Any]:
    """Load YAML OpenAPI spec from path."""
    with p.open() as f:
        return yaml.safe_load(f)


def validate_openapi_spec(path: Path) -> None:
    """Basic validation: spec loads and has openapi 3.1.0, paths, info. Raises ValueError if invalid."""
    with path.open() as f:
        spec = yaml.safe_load(f)
    if not spec or spec.get("openapi") != "3.1.0":
        msg = f"Invalid or non-OpenAPI 3.1.0 spec: {path}"
        raise ValueError(msg)
    if "paths" not in spec or "info" not in spec:
        msg = f"Spec missing paths or info: {path}"
        raise ValueError(msg)


def read_file_or_default(path: Path | None, default: str = "") -> str:
    """Return file text if path is a file, else default."""
    if path is not None and path.is_file():
        return path.read_text()
    return default


def extract_readme_overview(readme_path: Path, default: str = "") -> str:
    """First non-empty, non-heading line after \"## Overview\" in README, else default."""
    if not readme_path.exists():
        return default
    content = readme_path.read_text()
    if "## Overview" not in content:
        return default
    section = content.split("## Overview")[1].split("##")[0].strip()
    for line in section.split("\n"):
        line = line.strip()
        if line and not line.startswith("#"):
            return line
    return default


# --- Path ---


def downstream_path(base_path: str, bff_path: str) -> str:
    """Exact path on downstream: base_path + path (normalized)."""
    base = base_path.rstrip("/")
    path = bff_path.strip("/")
    return f"{base}/{path}" if path else base


def find_openapi_files(root: Path) -> list[Path]:
    """Find openapi.yaml and openapi.yml under root (rglob)."""
    out: list[Path] = []
    for name in ("openapi.yaml", "openapi.yml"):
        out.extend(root.rglob(name))
    return sorted(out)


def find_cargo_tomls(
    root: Path,
    *,
    exclude: set[str] | None = None,
) -> list[Path]:
    """All Cargo.toml under root, excluding path segments in exclude (default: target, node_modules, .git)."""
    if exclude is None:
        exclude = {"target", "node_modules", ".git"}
    out: list[Path] = []
    for p in root.rglob("Cargo.toml"):
        try:
            rel = p.relative_to(root)
        except ValueError:
            continue
        if any(part in exclude for part in rel.parts):
            continue
        out.append(p)
    return sorted(out)


# --- Version ---


def compare_versions(v1: str, v2: str) -> int:
    """Compare two version strings (semver). Returns positive if v1 > v2, negative if v1 < v2, zero if equal. Raises ValueError on invalid format."""
    v1 = v1.lstrip("v")
    v2 = v2.lstrip("v")

    def parse_version(v: str) -> tuple[int, int, int, str | None]:
        m = re.match(r"^(\d+)\.(\d+)\.(\d+)(?:-([\w.-]+))?$", v)
        if not m:
            msg = "Invalid version format: " + str(v)
            raise ValueError(msg)
        return (int(m.group(1)), int(m.group(2)), int(m.group(3)), m.group(4))

    major1, minor1, patch1, prerelease1 = parse_version(v1)
    major2, minor2, patch2, prerelease2 = parse_version(v2)

    if major1 != major2:
        return major1 - major2
    if minor1 != minor2:
        return minor1 - minor2
    if patch1 != patch2:
        return patch1 - patch2

    if prerelease1 is None and prerelease2 is not None:
        return 1
    if prerelease1 is not None and prerelease2 is None:
        return -1
    if prerelease1 is None and prerelease2 is None:
        return 0

    rc_match1 = re.match(r"^rc\.(\d+)$", prerelease1)
    rc_match2 = re.match(r"^rc\.(\d+)$", prerelease2)
    if rc_match1 and rc_match2:
        return int(rc_match1.group(1)) - int(rc_match2.group(1))

    if prerelease1 < prerelease2:
        return -1
    if prerelease1 > prerelease2:
        return 1
    return 0


# --- Retry ---


def fibonacci_backoff_sequence(max_total_seconds: int = 300) -> list[int]:
    """Generate Fibonacci backoff sequence (seconds) up to max_total_seconds."""
    sequence: list[int] = []
    total = 0
    a, b = 1, 1
    while total + a <= max_total_seconds:
        sequence.append(a)
        total += a
        a, b = b, a + b
    return sequence


# --- Naming ---


def default_binary_name(system: str, module: str) -> str:
    """Default RERP-style binary name: rerp_{system}_{module}_impl."""
    return f"rerp_{system}_{module.replace('-', '_')}_impl"
