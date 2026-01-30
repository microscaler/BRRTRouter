"""Shared CLI argument parsing for common flags (--project-root, --openapi-dir, etc.)."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Any


def parse_flags(
    argv: list[str],
    *specs: tuple[str, str, Any, Callable[[str], Any] | None],
) -> tuple[dict[str, Any], list[str]]:
    """Parse optional --flag value from argv in one pass.

    Each spec is (key, flag_str, default, converter).
    E.g. ("project_root", "--project-root", Path.cwd, lambda s: Path(s).resolve()).
    converter can be None for string values.
    Returns (dict of key -> value, remaining argv).
    """
    result: dict[str, Any] = {}
    for key, _flag, default, _converter in specs:
        result[key] = default() if callable(default) else default

    rest: list[str] = []
    i = 0
    while i < len(argv):
        matched = False
        for key, flag_str, _default, converter in specs:
            if argv[i] == flag_str and i + 1 < len(argv):
                result[key] = converter(argv[i + 1]) if converter else argv[i + 1]
                i += 2
                matched = True
                break
        if not matched:
            rest.append(argv[i])
            i += 1
    return result, rest


def path_resolver(s: str) -> Path:
    """Resolve a path argument to absolute Path (e.g. --project-root, --openapi-dir)."""
    return Path(s).resolve()
